//! PostgreSQL adapter for ProjectRepository

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use crate::domain::entities::{
    AgentId, BuildStatus, MemberRole, NewProject, Project, ProjectId, ProjectMember, ProjectStatus,
};
use crate::domain::ports::ProjectRepository;
use crate::entity::{project_members, projects};
use crate::error::DomainError;

/// PostgreSQL implementation of ProjectRepository
pub struct PostgresProjectRepository {
    db: DatabaseConnection,
}

impl PostgresProjectRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ProjectRepository for PostgresProjectRepository {
    async fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, DomainError> {
        let result = projects::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Project>, DomainError> {
        let result = projects::Entity::find()
            .filter(projects::Column::Name.eq(name))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_active(&self, limit: i64, offset: i64) -> Result<Vec<Project>, DomainError> {
        let results = projects::Entity::find()
            .filter(projects::Column::Status.eq("active"))
            .order_by_desc(projects::Column::CreatedAt)
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_all(&self, limit: i64, offset: i64) -> Result<Vec<Project>, DomainError> {
        let results = projects::Entity::find()
            .order_by_desc(projects::Column::CreatedAt)
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn create(&self, project: &NewProject) -> Result<Project, DomainError> {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        let model = projects::ActiveModel {
            id: Set(id),
            name: Set(project.name.clone()),
            description: Set(project.description.clone()),
            gitea_org: Set(project.gitea_org.clone()),
            gitea_repo: Set(project.gitea_repo.clone()),
            language: Set(project.language.clone()),
            status: Set(Some("active".to_string())),
            contributor_count: Set(Some(0)),
            open_ticket_count: Set(Some(0)),
            build_status: Set(Some("unknown".to_string())),
            created_by: Set(project.created_by.map(|id| id.0)),
            created_at: Set(Some(now)),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_status(
        &self,
        id: &ProjectId,
        status: ProjectStatus,
    ) -> Result<(), DomainError> {
        projects::ActiveModel {
            id: Set(id.0),
            status: Set(Some(status.to_string())),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn update_stats(
        &self,
        id: &ProjectId,
        contributor_count: i32,
        open_ticket_count: i32,
    ) -> Result<(), DomainError> {
        projects::ActiveModel {
            id: Set(id.0),
            contributor_count: Set(Some(contributor_count)),
            open_ticket_count: Set(Some(open_ticket_count)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn adjust_ticket_count(&self, id: &ProjectId, delta: i32) -> Result<(), DomainError> {
        // Use raw SQL for atomic increment
        let stmt = sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "UPDATE projects SET open_ticket_count = COALESCE(open_ticket_count, 0) + $1 WHERE id = $2",
            [delta.into(), id.0.into()],
        );

        self.db
            .execute(stmt)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_members(&self, id: &ProjectId) -> Result<Vec<ProjectMember>, DomainError> {
        let results = project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(id.0))
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn add_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
        role: MemberRole,
    ) -> Result<ProjectMember, DomainError> {
        let now = Utc::now().fixed_offset();

        let model = project_members::ActiveModel {
            project_id: Set(project_id.0),
            agent_id: Set(agent_id.0),
            role: Set(Some(role.to_string())),
            joined_at: Set(Some(now)),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn is_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<bool, DomainError> {
        let result = project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id.0))
            .filter(project_members::Column::AgentId.eq(agent_id.0))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.is_some())
    }

    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Project>, DomainError> {
        use sea_orm::JoinType;
        use sea_orm::QuerySelect;
        use sea_orm::RelationTrait;

        let results = projects::Entity::find()
            .join(
                JoinType::InnerJoin,
                projects::Relation::ProjectMembers.def(),
            )
            .filter(project_members::Column::AgentId.eq(agent_id.0))
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn get_member_role(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<Option<MemberRole>, DomainError> {
        let result = project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id.0))
            .filter(project_members::Column::AgentId.eq(agent_id.0))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.and_then(|m| m.role.and_then(|r| r.parse().ok())))
    }

    async fn update_member_role(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
        role: MemberRole,
    ) -> Result<(), DomainError> {
        use sea_orm::IntoActiveModel;

        let member = project_members::Entity::find()
            .filter(project_members::Column::ProjectId.eq(project_id.0))
            .filter(project_members::Column::AgentId.eq(agent_id.0))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?
            .ok_or_else(|| {
                DomainError::NotFound(format!("Member not found in project {}", project_id))
            })?;

        let mut active_model = member.into_active_model();
        active_model.role = Set(Some(role.to_string()));

        active_model
            .update(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn remove_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<(), DomainError> {
        let result = project_members::Entity::delete_many()
            .filter(project_members::Column::ProjectId.eq(project_id.0))
            .filter(project_members::Column::AgentId.eq(agent_id.0))
            .exec(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        if result.rows_affected == 0 {
            Err(DomainError::NotFound(format!(
                "Member not found in project {}",
                project_id
            )))
        } else {
            Ok(())
        }
    }
}

/// Convert SeaORM model to domain entity
impl From<projects::Model> for Project {
    fn from(model: projects::Model) -> Self {
        Project {
            id: ProjectId(model.id),
            name: model.name,
            description: model.description,
            gitea_org: model.gitea_org,
            gitea_repo: model.gitea_repo,
            language: model.language,
            status: model
                .status
                .and_then(|s| s.parse().ok())
                .unwrap_or(ProjectStatus::Active),
            contributor_count: model.contributor_count.unwrap_or(0),
            open_ticket_count: model.open_ticket_count.unwrap_or(0),
            build_status: model
                .build_status
                .and_then(|s| s.parse().ok())
                .unwrap_or(BuildStatus::Unknown),
            created_by: model.created_by.map(AgentId),
            created_at: model
                .created_at
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        }
    }
}

/// Convert SeaORM model to domain entity
impl From<project_members::Model> for ProjectMember {
    fn from(model: project_members::Model) -> Self {
        ProjectMember {
            project_id: ProjectId(model.project_id),
            agent_id: AgentId(model.agent_id),
            role: model
                .role
                .and_then(|r| r.parse().ok())
                .unwrap_or(MemberRole::Contributor),
            joined_at: model
                .joined_at
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        }
    }
}
