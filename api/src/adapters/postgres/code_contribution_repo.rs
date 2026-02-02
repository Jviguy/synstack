//! PostgreSQL adapter for CodeContributionRepository

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::domain::entities::{
    AgentId, CodeContribution, CodeContributionId, ContributionStatus, NewCodeContribution,
    ProjectId,
};
use crate::domain::ports::CodeContributionRepository;
use crate::entity::code_contributions;
use crate::error::DomainError;

/// PostgreSQL implementation of CodeContributionRepository
pub struct PostgresCodeContributionRepository {
    db: DatabaseConnection,
}

impl PostgresCodeContributionRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CodeContributionRepository for PostgresCodeContributionRepository {
    async fn find_by_id(
        &self,
        id: &CodeContributionId,
    ) -> Result<Option<CodeContribution>, DomainError> {
        let result = code_contributions::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_commit_sha(&self, sha: &str) -> Result<Option<CodeContribution>, DomainError> {
        let result = code_contributions::Entity::find()
            .filter(code_contributions::Column::CommitSha.eq(sha))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_pr(
        &self,
        project_id: &ProjectId,
        pr_number: i64,
    ) -> Result<Option<CodeContribution>, DomainError> {
        let result = code_contributions::Entity::find()
            .filter(code_contributions::Column::ProjectId.eq(project_id.0))
            .filter(code_contributions::Column::PrNumber.eq(pr_number))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_agent(
        &self,
        agent_id: &AgentId,
    ) -> Result<Vec<CodeContribution>, DomainError> {
        let results = code_contributions::Entity::find()
            .filter(code_contributions::Column::AgentId.eq(agent_id.0))
            .order_by_desc(code_contributions::Column::MergedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<CodeContribution>, DomainError> {
        let results = code_contributions::Entity::find()
            .filter(code_contributions::Column::ProjectId.eq(project_id.0))
            .order_by_desc(code_contributions::Column::MergedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_eligible_for_longevity_bonus(
        &self,
        threshold: DateTime<Utc>,
    ) -> Result<Vec<CodeContribution>, DomainError> {
        let results = code_contributions::Entity::find()
            .filter(code_contributions::Column::Status.eq("healthy"))
            .filter(code_contributions::Column::LongevityBonusPaid.eq(false))
            .filter(code_contributions::Column::MergedAt.lte(threshold.fixed_offset()))
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn create(
        &self,
        contribution: &NewCodeContribution,
    ) -> Result<CodeContribution, DomainError> {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        let model = code_contributions::ActiveModel {
            id: Set(id),
            agent_id: Set(contribution.agent_id.0),
            project_id: Set(contribution.project_id.0),
            pr_number: Set(contribution.pr_number),
            commit_sha: Set(contribution.commit_sha.clone()),
            status: Set("healthy".to_string()),
            bug_count: Set(0),
            longevity_bonus_paid: Set(false),
            dependent_prs_count: Set(0),
            merged_at: Set(contribution.merged_at.fixed_offset()),
            reverted_at: Set(None),
            replaced_at: Set(None),
            created_at: Set(now),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_status(
        &self,
        id: &CodeContributionId,
        status: ContributionStatus,
        timestamp: DateTime<Utc>,
    ) -> Result<(), DomainError> {
        let ts = timestamp.fixed_offset();

        let mut model = code_contributions::ActiveModel {
            id: Set(id.0),
            status: Set(status.to_string()),
            ..Default::default()
        };

        // Set the appropriate timestamp based on status
        match status {
            ContributionStatus::Reverted => {
                model.reverted_at = Set(Some(ts));
            }
            ContributionStatus::Replaced => {
                model.replaced_at = Set(Some(ts));
            }
            ContributionStatus::Healthy => {
                // Clear the timestamps if somehow going back to healthy
                model.reverted_at = Set(None);
                model.replaced_at = Set(None);
            }
        }

        model
            .update(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn mark_longevity_bonus_paid(&self, id: &CodeContributionId) -> Result<(), DomainError> {
        code_contributions::ActiveModel {
            id: Set(id.0),
            longevity_bonus_paid: Set(true),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn increment_bug_count(&self, id: &CodeContributionId) -> Result<(), DomainError> {
        // First fetch current value, then increment
        let contribution = self.find_by_id(id).await?;
        let contribution =
            contribution.ok_or_else(|| DomainError::NotFound("Contribution not found".into()))?;

        code_contributions::ActiveModel {
            id: Set(id.0),
            bug_count: Set(contribution.bug_count + 1),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn increment_dependent_prs(&self, id: &CodeContributionId) -> Result<(), DomainError> {
        // First fetch current value, then increment
        let contribution = self.find_by_id(id).await?;
        let contribution =
            contribution.ok_or_else(|| DomainError::NotFound("Contribution not found".into()))?;

        code_contributions::ActiveModel {
            id: Set(id.0),
            dependent_prs_count: Set(contribution.dependent_prs_count + 1),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }
}

/// Convert SeaORM model to domain entity
impl From<code_contributions::Model> for CodeContribution {
    fn from(model: code_contributions::Model) -> Self {
        CodeContribution {
            id: CodeContributionId(model.id),
            agent_id: AgentId(model.agent_id),
            project_id: ProjectId(model.project_id),
            pr_number: model.pr_number,
            commit_sha: model.commit_sha,
            status: model.status.parse().unwrap_or(ContributionStatus::Healthy),
            bug_count: model.bug_count,
            longevity_bonus_paid: model.longevity_bonus_paid,
            dependent_prs_count: model.dependent_prs_count,
            merged_at: model.merged_at.with_timezone(&Utc),
            reverted_at: model.reverted_at.map(|dt| dt.with_timezone(&Utc)),
            replaced_at: model.replaced_at.map(|dt| dt.with_timezone(&Utc)),
            created_at: model.created_at.with_timezone(&Utc),
        }
    }
}
