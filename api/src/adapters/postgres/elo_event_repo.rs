//! PostgreSQL adapter for EloEventRepository

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::domain::entities::{AgentId, EloEvent, EloEventId, EloEventType, NewEloEvent};
use crate::domain::ports::EloEventRepository;
use crate::entity::elo_events;
use crate::error::DomainError;

/// PostgreSQL implementation of EloEventRepository
pub struct PostgresEloEventRepository {
    db: DatabaseConnection,
}

impl PostgresEloEventRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EloEventRepository for PostgresEloEventRepository {
    async fn find_by_id(&self, id: &EloEventId) -> Result<Option<EloEvent>, DomainError> {
        let result = elo_events::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<EloEvent>, DomainError> {
        let results = elo_events::Entity::find()
            .filter(elo_events::Column::AgentId.eq(agent_id.0))
            .order_by_desc(elo_events::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_agent_paginated(
        &self,
        agent_id: &AgentId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EloEvent>, DomainError> {
        let results = elo_events::Entity::find()
            .filter(elo_events::Column::AgentId.eq(agent_id.0))
            .order_by_desc(elo_events::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_reference(&self, reference_id: Uuid) -> Result<Vec<EloEvent>, DomainError> {
        let results = elo_events::Entity::find()
            .filter(elo_events::Column::ReferenceId.eq(reference_id))
            .order_by_desc(elo_events::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn create(&self, event: &NewEloEvent) -> Result<EloEvent, DomainError> {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        let model = elo_events::ActiveModel {
            id: Set(id),
            agent_id: Set(event.agent_id.0),
            event_type: Set(event.event_type.to_string()),
            delta: Set(event.delta),
            old_elo: Set(event.old_elo),
            new_elo: Set(event.new_elo),
            reference_id: Set(event.reference_id),
            details: Set(event.details.clone()),
            created_at: Set(now),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn sum_delta_by_agent(&self, agent_id: &AgentId) -> Result<i64, DomainError> {
        use sea_orm::sea_query::Expr;

        // Use raw SQL for sum since SeaORM's sum returns Option<Decimal>
        let result: Option<i64> = elo_events::Entity::find()
            .filter(elo_events::Column::AgentId.eq(agent_id.0))
            .select_only()
            .column_as(Expr::col(elo_events::Column::Delta).sum(), "sum")
            .into_tuple()
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.unwrap_or(0))
    }
}

/// Convert SeaORM model to domain entity
impl From<elo_events::Model> for EloEvent {
    fn from(model: elo_events::Model) -> Self {
        EloEvent {
            id: EloEventId(model.id),
            agent_id: AgentId(model.agent_id),
            event_type: model.event_type.parse().unwrap_or(EloEventType::PrMerged),
            delta: model.delta,
            old_elo: model.old_elo,
            new_elo: model.new_elo,
            reference_id: model.reference_id,
            details: model.details,
            created_at: model.created_at.with_timezone(&Utc),
        }
    }
}
