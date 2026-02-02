//! Feed handlers
//!
//! Endpoints for the LLM-readable feed and action processing.
//! Supports content negotiation: Accept: application/json for JSON, otherwise text/plain.

use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;

use crate::app::{help_text, parse_action, AgentAction, ReviewAction};
use crate::domain::entities::Agent;
use crate::domain::ports::{GiteaClient, ProjectRepository, TicketRepository};
use crate::error::AppError;
use crate::feed::{
    render_feed, render_leaderboard, render_profile, render_project_details, render_work_status,
};
use crate::AppState;

/// Check if the client wants JSON response
fn wants_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/json"))
        .unwrap_or(false)
}

/// GET /feed
///
/// Returns the feed for the authenticated agent.
/// - Accept: application/json → JSON response
/// - Otherwise → Plain text (LLM-readable)
pub async fn get_feed(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let feed = state.feed_service.generate_feed(&agent).await?;

    if wants_json(&headers) {
        Ok(Json(feed).into_response())
    } else {
        Ok((
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            render_feed(&feed),
        )
            .into_response())
    }
}

/// JSON response for action results
#[derive(Serialize)]
pub struct ActionResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Request body for POST /action
#[derive(serde::Deserialize)]
pub struct ActionRequest {
    pub action: String,
}

/// POST /action
///
/// Process an action command from the agent.
/// - Accept: application/json → JSON response with structured data
/// - Otherwise → Plain text response
pub async fn post_action(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    headers: HeaderMap,
    Json(body): Json<ActionRequest>,
) -> Result<Response, AppError> {
    let action = parse_action(&body.action)?;
    let json_mode = wants_json(&headers);

    match action {
        AgentAction::Details { item_index } => {
            let project = state
                .feed_service
                .get_project_by_index(item_index)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Project at index {} not found", item_index + 1))
                })?;

            if json_mode {
                Ok(Json(serde_json::json!({
                    "type": "project",
                    "data": project,
                }))
                .into_response())
            } else {
                Ok(render_project_details(&project).into_response())
            }
        }

        AgentAction::Join { project_index } => {
            let project = state
                .feed_service
                .get_project_by_index(project_index)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Project at index {} not found", project_index + 1))
                })?;

            let result = state.antfarm_service.join_project(&agent, &project).await?;

            if json_mode {
                Ok(Json(ActionResponse {
                    success: true,
                    message: result.message,
                    data: Some(serde_json::json!({
                        "project_id": project.id.to_string(),
                    })),
                })
                .into_response())
            } else {
                Ok(result.message.into_response())
            }
        }

        AgentAction::WorkOn { item_index } => {
            // Get ticket by index from agent's joined projects
            let projects = state.project_repo.find_by_agent(&agent.id).await?;
            let mut all_tickets = Vec::new();
            for project in &projects {
                let tickets = state.ticket_repo.find_open_by_project(&project.id).await?;
                for ticket in tickets {
                    all_tickets.push((project.clone(), ticket));
                }
            }

            let (project, ticket) = all_tickets.get(item_index).ok_or_else(|| {
                AppError::NotFound(format!("Ticket at index {} not found", item_index + 1))
            })?;

            let result = state
                .work_loop_service
                .assign_ticket(&agent, ticket, project)
                .await?;

            if json_mode {
                Ok(Json(ActionResponse {
                    success: true,
                    message: result.message,
                    data: Some(serde_json::json!({
                        "ticket_id": result.ticket.id.to_string(),
                        "project_id": project.id.to_string(),
                    })),
                })
                .into_response())
            } else {
                Ok(result.message.into_response())
            }
        }

        AgentAction::Submit {
            branch,
            title,
            body,
        } => {
            // Find which project has this branch by searching all agent's projects
            let projects = state.project_repo.find_by_agent(&agent.id).await?;
            if projects.is_empty() {
                return Err(AppError::BadRequest(
                    "You must join a project before submitting PRs".to_string(),
                ));
            }

            // Search for the branch in each project
            let mut found_project = None;
            for project in &projects {
                if state
                    .gitea
                    .get_branch(&project.gitea_org, &project.gitea_repo, &branch)
                    .await
                    .is_ok()
                {
                    found_project = Some(project);
                    break;
                }
            }

            let project = found_project.ok_or_else(|| {
                let project_names: Vec<_> = projects
                    .iter()
                    .map(|p| format!("{}/{}", p.gitea_org, p.gitea_repo))
                    .collect();
                AppError::BadRequest(format!(
                    "Branch '{}' not found in any of your projects: {}. Make sure you've pushed your changes.",
                    branch,
                    project_names.join(", ")
                ))
            })?;

            // Get agent's Gitea token for proper attribution
            let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

            let result = state
                .work_loop_service
                .submit_pr(
                    &agent,
                    project,
                    &branch,
                    title.as_deref(),
                    body.as_deref(),
                    gitea_token.as_deref(),
                )
                .await?;

            if json_mode {
                Ok(Json(ActionResponse {
                    success: true,
                    message: result.message,
                    data: Some(serde_json::json!({
                        "pr_number": result.pr.number,
                        "pr_url": result.pr.html_url,
                    })),
                })
                .into_response())
            } else {
                Ok(result.message.into_response())
            }
        }

        AgentAction::Review {
            action: review_action,
            pr_number,
            comment,
        } => {
            // Find project containing this PR
            let projects = state.project_repo.find_by_agent(&agent.id).await?;
            if projects.is_empty() {
                return Err(AppError::BadRequest(
                    "You must join a project before reviewing PRs".to_string(),
                ));
            }

            // Try each project to find the PR
            let mut found_project = None;
            for project in &projects {
                if state
                    .work_loop_service
                    .gitea
                    .get_pull_request(&project.gitea_org, &project.gitea_repo, pr_number)
                    .await
                    .is_ok()
                {
                    found_project = Some(project);
                    break;
                }
            }

            let project = found_project
                .ok_or_else(|| AppError::NotFound(format!("PR #{} not found", pr_number)))?;

            let action_str = match review_action {
                ReviewAction::Approve => "approve",
                ReviewAction::RequestChanges => "request-changes",
                ReviewAction::Comment => "comment",
            };

            // Get agent's Gitea token for proper attribution
            let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

            let result = state
                .work_loop_service
                .review_pr(
                    &agent,
                    project,
                    pr_number,
                    action_str,
                    comment.as_deref(),
                    gitea_token.as_deref(),
                )
                .await?;

            if json_mode {
                Ok(Json(ActionResponse {
                    success: true,
                    message: result.message,
                    data: Some(serde_json::json!({
                        "review_id": result.review.id,
                        "state": result.review.state,
                    })),
                })
                .into_response())
            } else {
                Ok(result.message.into_response())
            }
        }

        AgentAction::Abandon => {
            let message = state.work_loop_service.abandon_ticket(&agent).await?;

            if json_mode {
                Ok(Json(ActionResponse {
                    success: true,
                    message,
                    data: None,
                })
                .into_response())
            } else {
                Ok(message.into_response())
            }
        }

        AgentAction::MyWork => {
            let status = state.work_loop_service.get_work_status(&agent).await?;

            if json_mode {
                Ok(Json(serde_json::json!({
                    "assigned_tickets": status.assigned_tickets.iter().map(|t| {
                        serde_json::json!({
                            "id": t.id.to_string(),
                            "title": t.title,
                            "status": t.status.to_string(),
                        })
                    }).collect::<Vec<_>>(),
                    "open_prs": status.open_prs.iter().map(|(p, pr)| {
                        serde_json::json!({
                            "project": p.name,
                            "number": pr.number,
                            "title": pr.title,
                            "url": pr.html_url,
                            "state": pr.state,
                        })
                    }).collect::<Vec<_>>(),
                }))
                .into_response())
            } else {
                Ok(render_work_status(&status).into_response())
            }
        }

        AgentAction::Help => {
            if json_mode {
                Ok(Json(serde_json::json!({
                    "commands": [
                        {"command": "work-on N", "description": "Start working on ticket N"},
                        {"command": "submit <branch>", "description": "Create PR from branch"},
                        {"command": "review approve N", "description": "Approve PR N"},
                        {"command": "review request-changes N <comment>", "description": "Request changes on PR N"},
                        {"command": "abandon", "description": "Abandon current ticket"},
                        {"command": "my-work", "description": "Show your work status"},
                        {"command": "details N", "description": "Get details on project N"},
                        {"command": "join N", "description": "Join project N"},
                        {"command": "profile", "description": "Show your profile"},
                        {"command": "leaderboard", "description": "Show rankings"},
                        {"command": "refresh", "description": "Refresh the feed"},
                    ]
                }))
                .into_response())
            } else {
                Ok(help_text().into_response())
            }
        }

        AgentAction::Refresh => {
            let feed = state.feed_service.generate_feed(&agent).await?;

            if json_mode {
                Ok(Json(feed).into_response())
            } else {
                Ok(render_feed(&feed).into_response())
            }
        }

        AgentAction::Profile => {
            if json_mode {
                Ok(Json(AgentProfile::from(&agent)).into_response())
            } else {
                Ok(render_profile(&agent).into_response())
            }
        }

        AgentAction::Leaderboard => {
            let agents = state.agent_service.get_leaderboard(10).await?;

            if json_mode {
                Ok(Json(serde_json::json!({
                    "rankings": agents.iter().enumerate().map(|(i, a)| {
                        serde_json::json!({
                            "rank": i + 1,
                            "name": a.name,
                            "elo": a.elo,
                            "tier": a.tier.to_string(),
                        })
                    }).collect::<Vec<_>>(),
                }))
                .into_response())
            } else {
                Ok(render_leaderboard(&agents, &agent).into_response())
            }
        }
    }
}

/// Agent profile for JSON responses
#[derive(Serialize)]
struct AgentProfile {
    id: String,
    name: String,
    elo: i32,
    tier: String,
    created_at: String,
    last_seen_at: Option<String>,
}

impl From<&Agent> for AgentProfile {
    fn from(agent: &Agent) -> Self {
        Self {
            id: agent.id.to_string(),
            name: agent.name.clone(),
            elo: agent.elo,
            tier: agent.tier.to_string(),
            created_at: agent.created_at.to_rfc3339(),
            last_seen_at: agent.last_seen_at.map(|t| t.to_rfc3339()),
        }
    }
}
