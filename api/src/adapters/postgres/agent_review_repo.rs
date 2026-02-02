//! PostgreSQL adapter for AgentReviewRepository

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::domain::entities::{
    AgentId, AgentReview, AgentReviewId, NewAgentReview, ProjectId, ReviewVerdict,
};
use crate::domain::ports::AgentReviewRepository;
use crate::entity::agent_reviews;
use crate::error::DomainError;

/// PostgreSQL implementation of AgentReviewRepository
pub struct PostgresAgentReviewRepository {
    db: DatabaseConnection,
}

impl PostgresAgentReviewRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AgentReviewRepository for PostgresAgentReviewRepository {
    async fn find_by_id(&self, id: &AgentReviewId) -> Result<Option<AgentReview>, DomainError> {
        let result = agent_reviews::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_pr(
        &self,
        project_id: &ProjectId,
        pr_id: i64,
    ) -> Result<Vec<AgentReview>, DomainError> {
        let results = agent_reviews::Entity::find()
            .filter(agent_reviews::Column::ProjectId.eq(project_id.0))
            .filter(agent_reviews::Column::PrId.eq(pr_id))
            .order_by_desc(agent_reviews::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_reviewer(&self, agent_id: &AgentId) -> Result<Vec<AgentReview>, DomainError> {
        let results = agent_reviews::Entity::find()
            .filter(agent_reviews::Column::ReviewerAgentId.eq(agent_id.0))
            .order_by_desc(agent_reviews::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_reviewed(&self, agent_id: &AgentId) -> Result<Vec<AgentReview>, DomainError> {
        let results = agent_reviews::Entity::find()
            .filter(agent_reviews::Column::ReviewedAgentId.eq(agent_id.0))
            .order_by_desc(agent_reviews::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn count_by_reviewer_since(
        &self,
        agent_id: &AgentId,
        since: DateTime<Utc>,
    ) -> Result<i64, DomainError> {
        let count = agent_reviews::Entity::find()
            .filter(agent_reviews::Column::ReviewerAgentId.eq(agent_id.0))
            .filter(agent_reviews::Column::CreatedAt.gte(since.fixed_offset()))
            .count(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count as i64)
    }

    async fn exists_for_pr_and_reviewer(
        &self,
        project_id: &ProjectId,
        pr_id: i64,
        reviewer_agent_id: &AgentId,
    ) -> Result<bool, DomainError> {
        let result = agent_reviews::Entity::find()
            .filter(agent_reviews::Column::ProjectId.eq(project_id.0))
            .filter(agent_reviews::Column::PrId.eq(pr_id))
            .filter(agent_reviews::Column::ReviewerAgentId.eq(reviewer_agent_id.0))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.is_some())
    }

    async fn create(&self, review: &NewAgentReview) -> Result<AgentReview, DomainError> {
        // Validate: no self-reviews
        if review.is_self_review() {
            return Err(DomainError::Validation(
                "Cannot review your own PR".to_string(),
            ));
        }

        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        let model = agent_reviews::ActiveModel {
            id: Set(id),
            pr_id: Set(review.pr_id),
            project_id: Set(review.project_id.0),
            reviewer_agent_id: Set(review.reviewer_agent_id.0),
            reviewed_agent_id: Set(review.reviewed_agent_id.0),
            verdict: Set(review.verdict.to_string()),
            reviewer_elo_at_time: Set(review.reviewer_elo_at_time),
            created_at: Set(now),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }
}

/// Convert SeaORM model to domain entity
impl From<agent_reviews::Model> for AgentReview {
    fn from(model: agent_reviews::Model) -> Self {
        AgentReview {
            id: AgentReviewId(model.id),
            pr_id: model.pr_id,
            project_id: ProjectId(model.project_id),
            reviewer_agent_id: AgentId(model.reviewer_agent_id),
            reviewed_agent_id: AgentId(model.reviewed_agent_id),
            verdict: model
                .verdict
                .parse()
                .unwrap_or(ReviewVerdict::ChangesRequested),
            reviewer_elo_at_time: model.reviewer_elo_at_time,
            created_at: model.created_at.with_timezone(&Utc),
        }
    }
}
