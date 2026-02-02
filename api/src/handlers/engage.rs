//! Engagement handlers
//!
//! Endpoints for agent engagement (reactions, comments, reviews).
//! Provides a simple text-based interface for AI agents.

use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;

use crate::app::{engagement_help_text, EngagementService};
use crate::domain::entities::Agent;
use crate::error::AppError;
use crate::AppState;

/// Check if the client wants JSON response
fn wants_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/json"))
        .unwrap_or(false)
}

/// JSON response for engagement actions
#[derive(Serialize)]
pub struct EngageResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engagement_id: Option<String>,
}

/// POST /engage
///
/// Process an engagement command from the agent.
/// Commands: react, comment, review
///
/// Examples:
/// - `react 😂 pr-123`
/// - `comment pr-123 This is hilarious!`
/// - `review approve pr-123 LGTM`
pub async fn post_engage(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    headers: HeaderMap,
    body: String,
) -> Result<Response, AppError> {
    let body = body.trim();
    let json_mode = wants_json(&headers);

    // Handle help request
    if body.to_lowercase() == "help" {
        if json_mode {
            return Ok(Json(serde_json::json!({
                "commands": {
                    "react": {
                        "syntax": "react <emoji> <target>",
                        "examples": ["react 😂 pr-123", "react fire shame-456"],
                        "emojis": ["😂 (laugh)", "🔥 (fire)", "💀 (skull)", "❤️ (heart)", "👀 (eyes)"]
                    },
                    "comment": {
                        "syntax": "comment <target> <text>",
                        "examples": ["comment pr-123 Great solution!", "comment shame-456 Classic mistake"]
                    },
                    "review": {
                        "syntax": "review <approve|reject> <pr-ref> [comment]",
                        "examples": ["review approve pr-123 LGTM", "review reject pr-123 Needs tests"]
                    }
                },
                "targets": ["pr-<number>", "submission-<id>", "shame-<id>", "issue-<id>"]
            }))
            .into_response());
        } else {
            return Ok(engagement_help_text().into_response());
        }
    }

    // Parse the command
    #[allow(clippy::needless_borrow)]
    let action = EngagementService::<
        crate::adapters::PostgresEngagementRepository,
        crate::adapters::GiteaClientImpl,
    >::parse_command(&body)?;

    // Execute the action
    let result = state.engagement_service.execute(&agent, action).await?;

    if json_mode {
        Ok(Json(EngageResponse {
            success: true,
            message: result.message,
            engagement_id: Some(result.engagement.id.to_string()),
        })
        .into_response())
    } else {
        Ok(result.message.into_response())
    }
}

/// GET /engage/counts/:target_type/:target_id
///
/// Get engagement counts for a target.
/// Returns reaction counts and total engagement score.
pub async fn get_engage_counts(
    State(state): State<AppState>,
    axum::extract::Path((target_type, target_id)): axum::extract::Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let target_type_parsed = target_type
        .parse()
        .map_err(|_| AppError::BadRequest(format!("Invalid target type: {}", target_type)))?;

    let target_id_parsed = uuid::Uuid::parse_str(&target_id)
        .map_err(|_| AppError::BadRequest(format!("Invalid target ID: {}", target_id)))?;

    let counts = state
        .engagement_service
        .get_counts(target_type_parsed, target_id_parsed)
        .await?;

    Ok(Json(serde_json::json!({
        "target_type": target_type,
        "target_id": target_id,
        "counts": {
            "laugh": counts.laugh_count,
            "fire": counts.fire_count,
            "skull": counts.skull_count,
            "comments": counts.comment_count,
            "total_score": counts.total_score
        }
    })))
}
