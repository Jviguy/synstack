//! API key authentication middleware

use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};

use crate::app::hash_api_key;
use crate::error::AppError;
use crate::AppState;

/// Extract the API key from the Authorization header
fn extract_api_key(request: &Request<Body>) -> Option<&str> {
    request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
}

/// Authentication middleware
///
/// Validates the API key and injects the Agent into request extensions.
/// Routes that require authentication should use this middleware.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Extract API key
    let api_key = extract_api_key(&request).ok_or(AppError::Unauthorized)?;

    // Hash the API key
    let key_hash = hash_api_key(api_key);

    // Look up the agent
    let agent = state
        .agent_service
        .find_by_api_key(&key_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    // Update last seen (fire and forget, log errors)
    let agent_id = agent.id;
    let agent_service = state.agent_service.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_service.touch(&agent_id).await {
            tracing::warn!(error = %e, agent_id = %agent_id.0, "Failed to update last_seen");
        }
    });

    // Inject agent into request extensions
    request.extensions_mut().insert(agent);

    // Continue to the handler
    Ok(next.run(request).await)
}

/// Optional authentication middleware
///
/// Like auth_middleware but doesn't fail if no auth is provided.
/// The agent will be None in extensions if not authenticated.
#[allow(dead_code)]
pub async fn optional_auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    if let Some(api_key) = extract_api_key(&request) {
        let key_hash = hash_api_key(api_key);

        if let Ok(Some(agent)) = state.agent_service.find_by_api_key(&key_hash).await {
            // Update last seen (fire and forget, log errors)
            let agent_id = agent.id;
            let agent_service = state.agent_service.clone();
            tokio::spawn(async move {
                if let Err(e) = agent_service.touch(&agent_id).await {
                    tracing::warn!(error = %e, agent_id = %agent_id.0, "Failed to update last_seen");
                }
            });

            request.extensions_mut().insert(agent);
        }
    }

    next.run(request).await
}
