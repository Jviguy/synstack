//! Claim handlers
//!
//! API endpoints for claiming agents via GitHub OAuth.
//! The frontend handles the OAuth redirect flow, this API does the token exchange.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::domain::entities::ClaimAgent;
use crate::error::AppError;
use crate::AppState;

/// Response for starting a claim - returns OAuth URL for frontend to redirect to
#[derive(Debug, Serialize)]
pub struct StartClaimResponse {
    pub agent_name: String,
    pub oauth_url: String,
    /// The state parameter to verify in callback
    pub state: String,
}

/// Request body for completing OAuth callback
#[derive(Debug, Deserialize)]
pub struct CompleteClaimRequest {
    /// The code from GitHub OAuth callback
    pub code: String,
    /// The state parameter (claim_code) to verify
    pub state: String,
}

/// Response for completed claim
#[derive(Debug, Serialize)]
pub struct CompleteClaimResponse {
    pub success: bool,
    pub agent_name: String,
    pub github_username: String,
    pub github_avatar_url: Option<String>,
}

/// GitHub OAuth token response
#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: Option<String>,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// GitHub user info from API
#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
    avatar_url: Option<String>,
}

/// GET /claim/:code
///
/// Get the GitHub OAuth URL to start the claim flow.
/// Frontend should redirect the user to the returned oauth_url.
pub async fn start_claim(
    State(state): State<AppState>,
    Path(claim_code): Path<String>,
) -> Result<Json<StartClaimResponse>, AppError> {
    // Check if GitHub OAuth is configured
    let client_id = state
        .config
        .github_client_id
        .as_ref()
        .ok_or_else(|| AppError::Internal("GitHub OAuth not configured".to_string()))?;

    // Verify the claim code exists
    let agent = state
        .agent_service
        .find_by_claim_code(&claim_code)
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid or expired claim code".to_string()))?;

    // Check if already claimed
    if agent.claimed_at.is_some() {
        return Err(AppError::BadRequest(format!(
            "Agent '{}' has already been claimed",
            agent.name
        )));
    }

    // Build GitHub OAuth URL
    // Frontend should redirect user here, GitHub will redirect back to frontend
    // with ?code=XXX&state=YYY, then frontend calls POST /claim/callback
    let redirect_uri = format!("{}/claim/callback", state.api_base_url);
    let oauth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&state={}&scope=read:user",
        client_id,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&claim_code)
    );

    Ok(Json(StartClaimResponse {
        agent_name: agent.name,
        oauth_url,
        state: claim_code,
    }))
}

/// POST /claim/callback
///
/// Complete the OAuth flow. Frontend calls this with the code and state
/// received from GitHub's redirect.
pub async fn complete_claim(
    State(state): State<AppState>,
    Json(request): Json<CompleteClaimRequest>,
) -> Result<Json<CompleteClaimResponse>, AppError> {
    let (client_id, client_secret) = match (
        state.config.github_client_id.as_ref(),
        state.config.github_client_secret.as_ref(),
    ) {
        (Some(id), Some(secret)) => (id, secret),
        _ => {
            return Err(AppError::Internal(
                "GitHub OAuth not configured".to_string(),
            ))
        }
    };

    // The state contains the claim code
    let claim_code = &request.state;

    // Verify the claim code and get the agent
    let agent = state
        .agent_service
        .find_by_claim_code(claim_code)
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid or expired claim code".to_string()))?;

    // Check if already claimed
    if agent.claimed_at.is_some() {
        return Err(AppError::BadRequest(format!(
            "Agent '{}' has already been claimed",
            agent.name
        )));
    }

    // Exchange the code for an access token
    let http = reqwest::Client::new();
    let token_response = http
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", &request.code),
        ])
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub API error: {}", e)))?;

    if !token_response.status().is_success() {
        return Err(AppError::Internal(
            "Failed to exchange code for token".to_string(),
        ));
    }

    let token_data: GitHubTokenResponse = token_response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse token response: {}", e)))?;

    // Check for OAuth error response
    if let Some(error) = token_data.error {
        let description = token_data.error_description.unwrap_or_default();
        tracing::warn!("GitHub OAuth error: {} - {}", error, description);
        return Err(AppError::BadRequest(format!(
            "GitHub OAuth error: {}",
            description
        )));
    }

    let access_token = token_data
        .access_token
        .ok_or_else(|| AppError::Internal("GitHub returned no access token".to_string()))?;

    // Get user info from GitHub
    let user_response = http
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "SynStack-API")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub API error: {}", e)))?;

    if !user_response.status().is_success() {
        return Err(AppError::Internal(
            "Failed to get user info from GitHub".to_string(),
        ));
    }

    let github_user: GitHubUser = user_response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse user response: {}", e)))?;

    // Check if this GitHub account already claimed an agent
    if let Some(existing_agent) = state
        .agent_service
        .find_by_github_id(github_user.id)
        .await?
    {
        return Err(AppError::BadRequest(format!(
            "GitHub account @{} has already claimed agent '{}'",
            github_user.login, existing_agent.name
        )));
    }

    // Claim the agent
    let claim_data = ClaimAgent {
        github_id: github_user.id,
        github_username: github_user.login.clone(),
        github_avatar_url: github_user.avatar_url.clone(),
    };

    state.agent_service.claim(&agent.id, &claim_data).await?;

    tracing::info!(
        agent_name = %agent.name,
        github_user = %github_user.login,
        "Agent claimed successfully"
    );

    Ok(Json(CompleteClaimResponse {
        success: true,
        agent_name: agent.name,
        github_username: github_user.login,
        github_avatar_url: github_user.avatar_url,
    }))
}

/// GET /claim/:code/status
///
/// Check claim status for an agent.
pub async fn claim_status(
    State(state): State<AppState>,
    Path(claim_code): Path<String>,
) -> Result<Json<ClaimStatusResponse>, AppError> {
    let agent = state.agent_service.find_by_claim_code(&claim_code).await?;

    match agent {
        Some(a) => Ok(Json(ClaimStatusResponse {
            agent_name: a.name,
            claimed: a.claimed_at.is_some(),
            github_username: a.github_username,
        })),
        None => Err(AppError::NotFound("Invalid claim code".to_string())),
    }
}

#[derive(Debug, Serialize)]
pub struct ClaimStatusResponse {
    pub agent_name: String,
    pub claimed: bool,
    pub github_username: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_complete_claim_request() {
        let json = r#"{"code": "abc123", "state": "claim-code-xyz"}"#;
        let request: CompleteClaimRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.code, "abc123");
        assert_eq!(request.state, "claim-code-xyz");
    }

    #[test]
    fn serialize_start_claim_response() {
        let response = StartClaimResponse {
            agent_name: "test-agent".to_string(),
            oauth_url: "https://github.com/login/oauth/authorize?...".to_string(),
            state: "claim-code-123".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("test-agent"));
        assert!(json.contains("oauth_url"));
    }

    #[test]
    fn serialize_complete_claim_response() {
        let response = CompleteClaimResponse {
            success: true,
            agent_name: "test-agent".to_string(),
            github_username: "octocat".to_string(),
            github_avatar_url: Some("https://github.com/avatar.png".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("test-agent"));
        assert!(json.contains("octocat"));
    }

    #[test]
    fn serialize_claim_status_response() {
        let response = ClaimStatusResponse {
            agent_name: "my-agent".to_string(),
            claimed: true,
            github_username: Some("octocat".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("my-agent"));
        assert!(json.contains("octocat"));
    }
}
