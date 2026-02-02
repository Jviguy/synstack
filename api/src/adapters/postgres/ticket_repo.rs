//! PostgreSQL adapter for TicketRepository

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::domain::entities::{
    AgentId, NewTicket, ProjectId, Ticket, TicketId, TicketPriority, TicketStatus,
};
use crate::domain::ports::TicketRepository;
use crate::entity::tickets;
use crate::error::DomainError;

/// PostgreSQL implementation of TicketRepository
pub struct PostgresTicketRepository {
    db: DatabaseConnection,
}

impl PostgresTicketRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TicketRepository for PostgresTicketRepository {
    async fn find_by_id(&self, id: &TicketId) -> Result<Option<Ticket>, DomainError> {
        let result = tickets::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_project(&self, project_id: &ProjectId) -> Result<Vec<Ticket>, DomainError> {
        let results = tickets::Entity::find()
            .filter(tickets::Column::ProjectId.eq(project_id.0))
            .order_by_desc(tickets::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_open_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<Ticket>, DomainError> {
        let results = tickets::Entity::find()
            .filter(tickets::Column::ProjectId.eq(project_id.0))
            .filter(tickets::Column::Status.eq("open"))
            .order_by_desc(tickets::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Ticket>, DomainError> {
        let results = tickets::Entity::find()
            .filter(tickets::Column::AssignedTo.eq(agent_id.0))
            .order_by_desc(tickets::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_open_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Ticket>, DomainError> {
        let results = tickets::Entity::find()
            .filter(tickets::Column::AssignedTo.eq(agent_id.0))
            .filter(tickets::Column::Status.ne("closed"))
            .order_by_desc(tickets::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn create(&self, ticket: &NewTicket) -> Result<Ticket, DomainError> {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        let model = tickets::ActiveModel {
            id: Set(id),
            project_id: Set(ticket.project_id.0),
            title: Set(ticket.title.clone()),
            body: Set(ticket.body.clone()),
            gitea_issue_number: Set(ticket.gitea_issue_number),
            gitea_issue_url: Set(ticket.gitea_issue_url.clone()),
            status: Set(Some("open".to_string())),
            priority: Set(Some(ticket.priority.to_string())),
            assigned_to: Set(None),
            created_by: Set(ticket.created_by.map(|a| a.0)),
            created_at: Set(Some(now)),
            closed_at: Set(None),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn assign(&self, id: &TicketId, agent_id: &AgentId) -> Result<(), DomainError> {
        tickets::ActiveModel {
            id: Set(id.0),
            assigned_to: Set(Some(agent_id.0)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn unassign(&self, id: &TicketId) -> Result<(), DomainError> {
        tickets::ActiveModel {
            id: Set(id.0),
            assigned_to: Set(None),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn update_status(&self, id: &TicketId, status: TicketStatus) -> Result<(), DomainError> {
        tickets::ActiveModel {
            id: Set(id.0),
            status: Set(Some(status.to_string())),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn close(&self, id: &TicketId) -> Result<(), DomainError> {
        let now = Utc::now().fixed_offset();

        tickets::ActiveModel {
            id: Set(id.0),
            status: Set(Some("closed".to_string())),
            closed_at: Set(Some(now)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn count_open_by_project(&self, project_id: &ProjectId) -> Result<i64, DomainError> {
        let count = tickets::Entity::find()
            .filter(tickets::Column::ProjectId.eq(project_id.0))
            .filter(tickets::Column::Status.eq("open"))
            .count(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count as i64)
    }
}

/// Convert SeaORM model to domain entity
impl From<tickets::Model> for Ticket {
    fn from(model: tickets::Model) -> Self {
        Ticket {
            id: TicketId(model.id),
            project_id: ProjectId(model.project_id),
            title: model.title,
            body: model.body,
            gitea_issue_number: model.gitea_issue_number,
            gitea_issue_url: model.gitea_issue_url,
            status: model
                .status
                .and_then(|s| s.parse().ok())
                .unwrap_or(TicketStatus::Open),
            priority: model
                .priority
                .and_then(|p| p.parse().ok())
                .unwrap_or(TicketPriority::Medium),
            assigned_to: model.assigned_to.map(AgentId),
            created_by: model.created_by.map(AgentId),
            created_at: model
                .created_at
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            closed_at: model.closed_at.map(|dt| dt.with_timezone(&Utc)),
        }
    }
}
