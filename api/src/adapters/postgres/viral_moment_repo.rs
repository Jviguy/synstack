//! PostgreSQL adapter for ViralMomentRepository
//!
//! NOTE: This file requires running `make db-migrate && make entities` to generate
//! the SeaORM entity files before it will compile.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, SimpleExpr},
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use crate::domain::entities::{
    AgentId, MomentType, NewViralMoment, ReferenceType, ViralMoment, ViralMomentId,
};
use crate::domain::ports::ViralMomentRepository;
use crate::entity::viral_moments;
use crate::error::DomainError;

/// PostgreSQL implementation of ViralMomentRepository
pub struct PostgresViralMomentRepository {
    db: DatabaseConnection,
}

impl PostgresViralMomentRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ViralMomentRepository for PostgresViralMomentRepository {
    async fn find_by_id(&self, id: &ViralMomentId) -> Result<Option<ViralMoment>, DomainError> {
        let result = viral_moments::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_type(
        &self,
        moment_type: MomentType,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, DomainError> {
        let results = viral_moments::Entity::find()
            .filter(viral_moments::Column::MomentType.eq(moment_type.to_string()))
            .filter(viral_moments::Column::Hidden.eq(false))
            .order_by_desc(viral_moments::Column::Score)
            .order_by_desc(viral_moments::Column::CreatedAt)
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_top(&self, limit: i64) -> Result<Vec<ViralMoment>, DomainError> {
        let results = viral_moments::Entity::find()
            .filter(viral_moments::Column::Hidden.eq(false))
            .order_by_desc(viral_moments::Column::Score)
            .order_by_desc(viral_moments::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_promoted(&self, limit: i64) -> Result<Vec<ViralMoment>, DomainError> {
        let results = viral_moments::Entity::find()
            .filter(viral_moments::Column::Hidden.eq(false))
            .filter(viral_moments::Column::Promoted.eq(true))
            .order_by_desc(viral_moments::Column::Score)
            .order_by_desc(viral_moments::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn exists_for_reference(
        &self,
        reference_type: &str,
        reference_id: Uuid,
    ) -> Result<bool, DomainError> {
        let count = viral_moments::Entity::find()
            .filter(viral_moments::Column::ReferenceType.eq(reference_type))
            .filter(viral_moments::Column::ReferenceId.eq(reference_id))
            .count(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count > 0)
    }

    async fn create(&self, moment: &NewViralMoment) -> Result<ViralMoment, DomainError> {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        // Convert AgentIds to UUIDs for the array
        let agent_ids: Vec<Uuid> = moment.agent_ids.iter().map(|a| a.0).collect();

        let model = viral_moments::ActiveModel {
            id: Set(id),
            moment_type: Set(moment.moment_type.to_string()),
            title: Set(moment.title.clone()),
            subtitle: Set(moment.subtitle.clone()),
            score: Set(moment.score),
            agent_ids: Set(agent_ids),
            reference_type: Set(moment.reference_type.to_string()),
            reference_id: Set(moment.reference_id),
            snapshot: Set(moment.snapshot.clone()),
            promoted: Set(false),
            hidden: Set(false),
            llm_classified: Set(false),
            llm_classification: Set(None),
            created_at: Set(now),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_score(&self, id: &ViralMomentId, score: i32) -> Result<(), DomainError> {
        viral_moments::ActiveModel {
            id: Set(id.0),
            score: Set(score),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn set_promoted(&self, id: &ViralMomentId, promoted: bool) -> Result<(), DomainError> {
        viral_moments::ActiveModel {
            id: Set(id.0),
            promoted: Set(promoted),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn set_hidden(&self, id: &ViralMomentId, hidden: bool) -> Result<(), DomainError> {
        viral_moments::ActiveModel {
            id: Set(id.0),
            hidden: Set(hidden),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn update_llm_classification(
        &self,
        id: &ViralMomentId,
        classification: serde_json::Value,
    ) -> Result<(), DomainError> {
        viral_moments::ActiveModel {
            id: Set(id.0),
            llm_classified: Set(true),
            llm_classification: Set(Some(classification)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn find_by_agent(
        &self,
        agent_id: &AgentId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, DomainError> {
        // PostgreSQL array contains operator: agent_ids @> ARRAY[uuid]::uuid[]
        // Use raw expression for array containment check
        let array_contains: SimpleExpr =
            Expr::cust_with_values("agent_ids @> ARRAY[$1]::uuid[]", [agent_id.0]);

        let results = viral_moments::Entity::find()
            .filter(viral_moments::Column::Hidden.eq(false))
            .filter(array_contains)
            .order_by_desc(viral_moments::Column::CreatedAt)
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }
}

/// Convert SeaORM model to domain entity
impl From<viral_moments::Model> for ViralMoment {
    fn from(model: viral_moments::Model) -> Self {
        ViralMoment {
            id: ViralMomentId(model.id),
            moment_type: model.moment_type.parse().unwrap_or(MomentType::HallOfShame),
            title: model.title,
            subtitle: model.subtitle,
            score: model.score,
            agent_ids: model.agent_ids.into_iter().map(AgentId).collect(),
            reference_type: model
                .reference_type
                .parse()
                .unwrap_or(ReferenceType::PullRequest),
            reference_id: model.reference_id,
            snapshot: model.snapshot,
            promoted: model.promoted,
            hidden: model.hidden,
            llm_classified: model.llm_classified,
            llm_classification: model
                .llm_classification
                .and_then(|v| serde_json::from_value(v).ok()),
            created_at: model.created_at.with_timezone(&Utc),
        }
    }
}
