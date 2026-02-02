//! PostgreSQL adapter for EngagementRepository
//!
//! NOTE: This file requires running `make db-migrate && make entities` to generate
//! the SeaORM entity files before it will compile.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use crate::domain::entities::{
    AgentId, Engagement, EngagementCounts, EngagementId, EngagementType, NewEngagement, TargetType,
};
use crate::domain::ports::EngagementRepository;
use crate::entity::{engagement_counts, engagements};
use crate::error::DomainError;

/// PostgreSQL implementation of EngagementRepository
pub struct PostgresEngagementRepository {
    db: DatabaseConnection,
}

impl PostgresEngagementRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EngagementRepository for PostgresEngagementRepository {
    async fn find_by_id(&self, id: &EngagementId) -> Result<Option<Engagement>, DomainError> {
        let result = engagements::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_target(
        &self,
        target_type: &str,
        target_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Engagement>, DomainError> {
        let results = engagements::Entity::find()
            .filter(engagements::Column::TargetType.eq(target_type))
            .filter(engagements::Column::TargetId.eq(target_id))
            .order_by_desc(engagements::Column::CreatedAt)
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_agent(
        &self,
        agent_id: &AgentId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Engagement>, DomainError> {
        let results = engagements::Entity::find()
            .filter(engagements::Column::AgentId.eq(agent_id.0))
            .order_by_desc(engagements::Column::CreatedAt)
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn get_counts(
        &self,
        target_type: &str,
        target_id: Uuid,
    ) -> Result<EngagementCounts, DomainError> {
        let result = engagement_counts::Entity::find()
            .filter(engagement_counts::Column::TargetType.eq(target_type))
            .filter(engagement_counts::Column::TargetId.eq(target_id))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()).unwrap_or_default())
    }

    async fn create(&self, engagement: &NewEngagement) -> Result<Engagement, DomainError> {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        let model = engagements::ActiveModel {
            id: Set(id),
            agent_id: Set(engagement.agent_id.0),
            target_type: Set(engagement.target_type.to_string()),
            target_id: Set(engagement.target_id),
            engagement_type: Set(engagement.engagement_type.to_string()),
            reaction: Set(engagement.reaction.map(|r| r.to_string())),
            body: Set(engagement.body.clone()),
            gitea_synced: Set(false),
            gitea_id: Set(None),
            created_at: Set(now),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn mark_synced(&self, id: &EngagementId, gitea_id: i64) -> Result<(), DomainError> {
        engagements::ActiveModel {
            id: Set(id.0),
            gitea_synced: Set(true),
            gitea_id: Set(Some(gitea_id)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn has_reaction(
        &self,
        agent_id: &AgentId,
        target_type: &str,
        target_id: Uuid,
        reaction: &str,
    ) -> Result<bool, DomainError> {
        let count = engagements::Entity::find()
            .filter(engagements::Column::AgentId.eq(agent_id.0))
            .filter(engagements::Column::TargetType.eq(target_type))
            .filter(engagements::Column::TargetId.eq(target_id))
            .filter(engagements::Column::Reaction.eq(reaction))
            .count(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count > 0)
    }
}

/// Convert SeaORM model to domain entity
impl From<engagements::Model> for Engagement {
    fn from(model: engagements::Model) -> Self {
        Engagement {
            id: EngagementId(model.id),
            agent_id: AgentId(model.agent_id),
            target_type: model.target_type.parse().unwrap_or(TargetType::Pr),
            target_id: model.target_id,
            engagement_type: model
                .engagement_type
                .parse()
                .unwrap_or(EngagementType::Reaction),
            reaction: model.reaction.and_then(|r| r.parse().ok()),
            body: model.body,
            gitea_synced: model.gitea_synced,
            gitea_id: model.gitea_id,
            created_at: model.created_at.with_timezone(&Utc),
        }
    }
}

/// Convert engagement_counts model to domain entity
impl From<engagement_counts::Model> for EngagementCounts {
    fn from(model: engagement_counts::Model) -> Self {
        EngagementCounts {
            laugh_count: model.laugh_count,
            fire_count: model.fire_count,
            skull_count: model.skull_count,
            comment_count: model.comment_count,
            total_score: model.total_score,
        }
    }
}
