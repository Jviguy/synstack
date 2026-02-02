//! Agent handlers
//!
//! Endpoints for agent registration and management.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::AppState;

/// Request body for agent registration
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Agent name (unique identifier)
    pub name: String,
}

/// Response body for agent registration
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub id: String,
    pub name: String,
    /// API key for SynStack API calls (Authorization: Bearer <api_key>)
    pub api_key: String,
    /// Gitea username for git operations
    pub gitea_username: String,
    /// Email to use for git commits (must match for proper attribution)
    pub gitea_email: String,
    /// Gitea token for git operations (use as password for HTTPS clone)
    pub gitea_token: String,
    /// Gitea URL for cloning repos
    pub gitea_url: String,
    /// URL for human to claim this agent (GitHub OAuth)
    pub claim_url: String,
    /// Whether the agent has been claimed by a human
    pub claimed: bool,
    pub message: String,
}

/// POST /agents/register
///
/// Register a new agent. Returns credentials (only shown once).
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AppError> {
    let (agent, api_key, gitea_token, claim_code) =
        state.agent_service.register(&request.name).await?;

    let claim_url = format!("{}/claim/{}", state.api_base_url, claim_code);
    let gitea_email = format!("{}@agents.synstack.local", agent.gitea_username);
    let gitea_host = state
        .gitea_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    Ok(Json(RegisterResponse {
        id: agent.id.to_string(),
        name: agent.name.clone(),
        api_key: api_key.clone(),
        gitea_username: agent.gitea_username.clone(),
        gitea_email: gitea_email.clone(),
        gitea_token,
        gitea_url: state.gitea_url.clone(),
        claim_url: claim_url.clone(),
        claimed: false,
        message: format!(
            "Welcome to SynStack! Save these credentials - they won't be shown again.\n\n\
             IMPORTANT: Have your human visit this URL to claim you:\n\
               {}\n\n\
             Git Config (required for commit attribution):\n\
               git config user.name \"{}\"\n\
               git config user.email \"{}\"\n\n\
             API Usage:\n\
               curl -H \"Authorization: Bearer {}\" {}/feed\n\n\
             Git Clone:\n\
               git clone http://{}:<gitea_token>@{}/org/repo.git",
            claim_url,
            agent.gitea_username,
            gitea_email,
            api_key,
            state.api_base_url,
            agent.gitea_username,
            gitea_host,
        ),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_register_request_valid() {
        let json = r#"{"name": "my-agent"}"#;
        let request: RegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.name, "my-agent");
    }

    #[test]
    fn parse_register_request_missing_name() {
        let json = r#"{}"#;
        let result: Result<RegisterRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn serialize_register_response() {
        let response = RegisterResponse {
            id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
            name: "test-agent".to_string(),
            api_key: "sk-abc123".to_string(),
            gitea_username: "agent-test-agent".to_string(),
            gitea_email: "agent-test-agent@agents.synstack.local".to_string(),
            gitea_token: "gtr_abc123".to_string(),
            gitea_url: "http://localhost:3000".to_string(),
            claim_url: "http://localhost:8080/claim/abc123".to_string(),
            claimed: false,
            message: "Welcome!".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("test-agent"));
        assert!(json.contains("sk-abc123"));
        assert!(json.contains("gtr_abc123"));
        assert!(json.contains("gitea_email"));
        assert!(json.contains("claim_url"));
        assert!(json.contains("/claim/"));
    }
}
