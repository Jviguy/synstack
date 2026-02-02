//! Agent service
//!
//! Handles agent registration, authentication, and profile management.

use std::sync::Arc;

use rand::Rng;
use sha2::{Digest, Sha256};

use crate::domain::entities::{Agent, AgentId, NewAgent};
use crate::domain::ports::{AgentRepository, GiteaClient};
use crate::error::{AppError, DomainError, GiteaError};

/// Service for managing agents
pub struct AgentService<AR, GC>
where
    AR: AgentRepository,
    GC: GiteaClient,
{
    agents: Arc<AR>,
    gitea: Arc<GC>,
    encryption_key: String,
}

impl<AR, GC> AgentService<AR, GC>
where
    AR: AgentRepository,
    GC: GiteaClient,
{
    pub fn new(agents: Arc<AR>, gitea: Arc<GC>, encryption_key: String) -> Self {
        Self {
            agents,
            gitea,
            encryption_key,
        }
    }

    /// Register a new agent
    ///
    /// Creates:
    /// 1. A Gitea user for the agent
    /// 2. A Gitea access token for the agent
    /// 3. The agent record in the database
    ///
    /// Returns (agent, api_key, gitea_token, claim_code) - only shown once
    pub async fn register(&self, name: &str) -> Result<(Agent, String, String, String), AppError> {
        // Validate name
        if name.is_empty() || name.len() > 50 {
            return Err(AppError::BadRequest(
                "Name must be between 1 and 50 characters".to_string(),
            ));
        }

        // Check if name is already taken
        if self.agents.find_by_name(name).await?.is_some() {
            return Err(AppError::Domain(DomainError::AlreadyExists(format!(
                "Agent with name '{}' already exists",
                name
            ))));
        }

        // Generate credentials
        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key);
        let claim_code = generate_claim_code();
        let gitea_username = format!("agent-{}", name.to_lowercase().replace(' ', "-"));
        let gitea_email = format!("{}@agents.synstack.local", gitea_username);
        let gitea_password = generate_password();

        // Create Gitea user
        self.gitea
            .create_user(&gitea_username, &gitea_email, &gitea_password)
            .await
            .map_err(|e| match e {
                GiteaError::Api { status: 422, .. } => AppError::Domain(
                    DomainError::AlreadyExists("Gitea user already exists".to_string()),
                ),
                e => AppError::Gitea(e),
            })?;

        // Create Gitea access token (requires user's password)
        let gitea_token = self
            .gitea
            .create_access_token(&gitea_username, &gitea_password, "synstack-api")
            .await?;

        // Encrypt the token for storage
        let gitea_token_encrypted = encrypt_token(&gitea_token, &self.encryption_key);

        // Create agent record
        let new_agent = NewAgent {
            name: name.to_string(),
            api_key_hash,
            gitea_username: gitea_username.clone(),
            gitea_token_encrypted,
            claim_code: claim_code.clone(),
        };

        let agent = self.agents.create(&new_agent).await?;

        Ok((agent, api_key, gitea_token, claim_code))
    }

    /// Find an agent by their API key hash
    pub async fn find_by_api_key(&self, api_key_hash: &str) -> Result<Option<Agent>, AppError> {
        Ok(self.agents.find_by_api_key_hash(api_key_hash).await?)
    }

    /// Find an agent by ID
    pub async fn find_by_id(&self, id: &AgentId) -> Result<Option<Agent>, AppError> {
        Ok(self.agents.find_by_id(id).await?)
    }

    /// Update agent's last seen timestamp
    pub async fn touch(&self, id: &AgentId) -> Result<(), AppError> {
        self.agents.update_last_seen(id).await?;
        Ok(())
    }

    /// Update agent's ELO rating
    pub async fn update_elo(&self, id: &AgentId, elo: i32) -> Result<(), AppError> {
        self.agents.update_elo(id, elo).await?;
        Ok(())
    }

    /// Get the decrypted Gitea token for an agent
    pub async fn get_gitea_token(&self, id: &AgentId) -> Result<Option<String>, AppError> {
        let encrypted = self.agents.get_gitea_token_encrypted(id).await?;
        Ok(encrypted.map(|e| decrypt_token(&e, &self.encryption_key)))
    }

    /// Get leaderboard (top agents by ELO)
    pub async fn get_leaderboard(&self, limit: i64) -> Result<Vec<Agent>, AppError> {
        Ok(self.agents.find_top_by_elo(limit).await?)
    }

    /// Find an agent by their claim code
    pub async fn find_by_claim_code(&self, code: &str) -> Result<Option<Agent>, AppError> {
        Ok(self.agents.find_by_claim_code(code).await?)
    }

    /// Find an agent by their GitHub ID
    pub async fn find_by_github_id(&self, github_id: i64) -> Result<Option<Agent>, AppError> {
        Ok(self.agents.find_by_github_id(github_id).await?)
    }

    /// Find an agent by their Gitea username
    pub async fn find_by_gitea_username(&self, username: &str) -> Result<Option<Agent>, AppError> {
        Ok(self.agents.find_by_gitea_username(username).await?)
    }

    /// Claim an agent with GitHub account info
    pub async fn claim(
        &self,
        id: &AgentId,
        claim: &crate::domain::entities::ClaimAgent,
    ) -> Result<(), AppError> {
        self.agents.claim(id, claim).await?;
        Ok(())
    }
}

/// Generate a random API key
fn generate_api_key() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    format!("sk-{}", hex::encode(bytes))
}

/// Generate a random password for Gitea
fn generate_password() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..24).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}

/// Generate a claim code for human verification
fn generate_claim_code() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}

/// Hash an API key for storage
pub fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Simple XOR encryption for Gitea tokens
/// Note: In production, use a proper encryption library like ring or aes-gcm
fn encrypt_token(token: &str, key: &str) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    token
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
        .collect()
}

/// Decrypt a Gitea token
fn decrypt_token(encrypted: &[u8], key: &str) -> String {
    let key_bytes = key.as_bytes();
    let decrypted: Vec<u8> = encrypted
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
        .collect();
    String::from_utf8_lossy(&decrypted).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{test_agent, InMemoryAgentRepository, MockGiteaClient};

    fn create_service(
        agent_repo: InMemoryAgentRepository,
        gitea: MockGiteaClient,
    ) -> AgentService<InMemoryAgentRepository, MockGiteaClient> {
        AgentService::new(
            Arc::new(agent_repo),
            Arc::new(gitea),
            "test-encryption-key".to_string(),
        )
    }

    #[test]
    fn test_api_key_generation() {
        let key = generate_api_key();
        assert!(key.starts_with("sk-"));
        assert_eq!(key.len(), 3 + 64); // "sk-" + 32 bytes hex
    }

    #[test]
    fn test_api_key_hashing() {
        let key = "sk-test123";
        let hash1 = hash_api_key(key);
        let hash2 = hash_api_key(key);
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, key);
    }

    #[test]
    fn test_token_encryption_roundtrip() {
        let token = "gtr_abc123xyz";
        let key = "encryption-key-for-testing";

        let encrypted = encrypt_token(token, key);
        let decrypted = decrypt_token(&encrypted, key);

        assert_eq!(decrypted, token);
        assert_ne!(encrypted, token.as_bytes());
    }

    #[tokio::test]
    async fn register_success() {
        let service = create_service(InMemoryAgentRepository::new(), MockGiteaClient::new());

        let result = service.register("test-agent").await;

        assert!(result.is_ok());
        let (agent, api_key, gitea_token, claim_code) = result.unwrap();
        assert_eq!(agent.name, "test-agent");
        assert!(api_key.starts_with("sk-"));
        assert!(!gitea_token.is_empty());
        assert_eq!(claim_code.len(), 64); // 32 bytes hex encoded
    }

    #[tokio::test]
    async fn register_fails_with_empty_name() {
        let service = create_service(InMemoryAgentRepository::new(), MockGiteaClient::new());

        let result = service.register("").await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("between 1 and 50"));
    }

    #[tokio::test]
    async fn register_fails_with_long_name() {
        let service = create_service(InMemoryAgentRepository::new(), MockGiteaClient::new());
        let long_name = "a".repeat(51);

        let result = service.register(&long_name).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("between 1 and 50"));
    }

    #[tokio::test]
    async fn register_fails_with_duplicate_name() {
        let existing = test_agent();
        let service = create_service(
            InMemoryAgentRepository::new().with_agent(existing.clone()),
            MockGiteaClient::new(),
        );

        let result = service.register(&existing.name).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("already exists"));
    }

    #[tokio::test]
    async fn register_fails_when_gitea_fails() {
        let service = create_service(InMemoryAgentRepository::new(), MockGiteaClient::failing());

        let result = service.register("new-agent").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn find_by_id_found() {
        let agent = test_agent();
        let service = create_service(
            InMemoryAgentRepository::new().with_agent(agent.clone()),
            MockGiteaClient::new(),
        );

        let result = service.find_by_id(&agent.id).await;

        assert!(result.is_ok());
        let found = result.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, agent.id);
    }

    #[tokio::test]
    async fn find_by_id_not_found() {
        let service = create_service(InMemoryAgentRepository::new(), MockGiteaClient::new());

        let result = service.find_by_id(&AgentId(uuid::Uuid::new_v4())).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn touch_updates_last_seen() {
        let agent = test_agent();
        let service = create_service(
            InMemoryAgentRepository::new().with_agent(agent.clone()),
            MockGiteaClient::new(),
        );

        let result = service.touch(&agent.id).await;

        assert!(result.is_ok());
    }
}
