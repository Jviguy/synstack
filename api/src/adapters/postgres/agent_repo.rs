//! PostgreSQL adapter for AgentRepository

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::domain::entities::{Agent, AgentId, ClaimAgent, NewAgent, Tier};
use crate::domain::ports::AgentRepository;
use crate::entity::agents;
use crate::error::DomainError;

/// PostgreSQL implementation of AgentRepository
pub struct PostgresAgentRepository {
    db: DatabaseConnection,
}

impl PostgresAgentRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AgentRepository for PostgresAgentRepository {
    async fn find_by_id(&self, id: &AgentId) -> Result<Option<Agent>, DomainError> {
        let result = agents::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_api_key_hash(&self, hash: &str) -> Result<Option<Agent>, DomainError> {
        let result = agents::Entity::find()
            .filter(agents::Column::ApiKeyHash.eq(hash))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Agent>, DomainError> {
        let result = agents::Entity::find()
            .filter(agents::Column::Name.eq(name))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_gitea_username(&self, username: &str) -> Result<Option<Agent>, DomainError> {
        let result = agents::Entity::find()
            .filter(agents::Column::GiteaUsername.eq(username))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn create(&self, agent: &NewAgent) -> Result<Agent, DomainError> {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        let model = agents::ActiveModel {
            id: Set(id),
            name: Set(agent.name.clone()),
            api_key_hash: Set(agent.api_key_hash.clone()),
            gitea_username: Set(agent.gitea_username.clone()),
            gitea_token_encrypted: Set(agent.gitea_token_encrypted.clone()),
            elo: Set(Some(1000)),
            tier: Set(Some("bronze".to_string())),
            created_at: Set(Some(now)),
            last_seen_at: Set(None),
            claim_code: Set(Some(agent.claim_code.clone())),
            claimed_at: Set(None),
            github_id: Set(None),
            github_username: Set(None),
            github_avatar_url: Set(None),
        };

        let result = model
            .insert(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_last_seen(&self, id: &AgentId) -> Result<(), DomainError> {
        let now = Utc::now().fixed_offset();

        agents::ActiveModel {
            id: Set(id.0),
            last_seen_at: Set(Some(now)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn update_elo(&self, id: &AgentId, elo: i32) -> Result<(), DomainError> {
        let tier = Tier::from_elo(elo).to_string();

        agents::ActiveModel {
            id: Set(id.0),
            elo: Set(Some(elo)),
            tier: Set(Some(tier)),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_gitea_token_encrypted(
        &self,
        id: &AgentId,
    ) -> Result<Option<Vec<u8>>, DomainError> {
        let result = agents::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.gitea_token_encrypted))
    }

    async fn find_top_by_elo(&self, limit: i64) -> Result<Vec<Agent>, DomainError> {
        let results = agents::Entity::find()
            .order_by_desc(agents::Column::Elo)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_claim_code(&self, code: &str) -> Result<Option<Agent>, DomainError> {
        let result = agents::Entity::find()
            .filter(agents::Column::ClaimCode.eq(code))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_github_id(&self, github_id: i64) -> Result<Option<Agent>, DomainError> {
        let result = agents::Entity::find()
            .filter(agents::Column::GithubId.eq(github_id))
            .one(&self.db)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn claim(&self, id: &AgentId, claim: &ClaimAgent) -> Result<(), DomainError> {
        let now = Utc::now().fixed_offset();

        agents::ActiveModel {
            id: Set(id.0),
            claim_code: Set(None), // Clear claim code after claiming
            claimed_at: Set(Some(now)),
            github_id: Set(Some(claim.github_id)),
            github_username: Set(Some(claim.github_username.clone())),
            github_avatar_url: Set(claim.github_avatar_url.clone()),
            ..Default::default()
        }
        .update(&self.db)
        .await
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }
}

/// Convert SeaORM model to domain entity
impl From<agents::Model> for Agent {
    fn from(model: agents::Model) -> Self {
        Agent {
            id: AgentId(model.id),
            name: model.name,
            api_key_hash: model.api_key_hash,
            gitea_username: model.gitea_username,
            elo: model.elo.unwrap_or(1000),
            tier: model
                .tier
                .and_then(|s| s.parse().ok())
                .unwrap_or(Tier::Bronze),
            created_at: model
                .created_at
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            last_seen_at: model.last_seen_at.map(|dt| dt.with_timezone(&Utc)),
            claim_code: model.claim_code,
            claimed_at: model.claimed_at.map(|dt| dt.with_timezone(&Utc)),
            github_id: model.github_id,
            github_username: model.github_username,
            github_avatar_url: model.github_avatar_url,
        }
    }
}
