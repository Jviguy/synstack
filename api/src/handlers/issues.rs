//! Issue handlers
//!
//! Comprehensive API for interacting with issues.
//! Issues live in Gitea (source of truth).

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::entities::{Agent, IssueId, NewIssue, ProjectId};
use crate::domain::ports::{IssueRepository, ProjectRepository};
use crate::error::AppError;
use crate::AppState;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Query parameters for listing issues
#[derive(Debug, Deserialize)]
pub struct ListIssuesQuery {
    /// Issue state filter (open, closed, all)
    #[serde(default = "default_state")]
    pub state: String,
}

fn default_state() -> String {
    "open".to_string()
}

/// Issue response
#[derive(Debug, Serialize)]
pub struct IssueResponse {
    pub project_id: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub url: String,
    pub labels: Vec<LabelResponse>,
    pub assignees: Vec<String>,
}

/// Label response
#[derive(Debug, Serialize)]
pub struct LabelResponse {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

/// Comment response
#[derive(Debug, Serialize)]
pub struct CommentResponse {
    pub id: i64,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Request to create a new issue
#[derive(Debug, Deserialize)]
pub struct CreateIssueRequest {
    pub title: String,
    pub body: String,
}

/// Request to update an issue
#[derive(Debug, Deserialize)]
pub struct UpdateIssueRequest {
    pub title: Option<String>,
    pub body: Option<String>,
}

/// Request to add a comment
#[derive(Debug, Deserialize)]
pub struct AddCommentRequest {
    pub body: String,
}

/// Request to edit a comment
#[derive(Debug, Deserialize)]
pub struct EditCommentRequest {
    pub body: String,
}

/// Request to add labels
#[derive(Debug, Deserialize)]
pub struct AddLabelsRequest {
    pub labels: Vec<String>,
}

/// Request to assign users
#[derive(Debug, Deserialize)]
pub struct AssignRequest {
    pub assignees: Vec<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /projects/:id/issues
///
/// List issues for a project.
pub async fn list_issues(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListIssuesQuery>,
) -> Result<Json<Vec<IssueResponse>>, AppError> {
    let project_id = ProjectId(project_id);

    let issues = state
        .issue_repo
        .list(&project_id, Some(&query.state))
        .await?;

    let responses: Vec<IssueResponse> = issues
        .into_iter()
        .map(|i| IssueResponse {
            project_id: i.id.project_id.0.to_string(),
            number: i.id.number,
            title: i.title,
            body: i.body,
            state: i.state.to_string(),
            url: i.url,
            labels: i
                .labels
                .into_iter()
                .map(|l| LabelResponse {
                    name: l.name,
                    color: l.color,
                    description: l.description,
                })
                .collect(),
            assignees: i.assignees,
        })
        .collect();

    Ok(Json(responses))
}

/// GET /projects/:id/issues/:number
///
/// Get a specific issue.
pub async fn get_issue(
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let issue = state
        .issue_repo
        .get(&issue_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Issue {} not found", number)))?;

    Ok(Json(IssueResponse {
        project_id: issue.id.project_id.0.to_string(),
        number: issue.id.number,
        title: issue.title,
        body: issue.body,
        state: issue.state.to_string(),
        url: issue.url,
        labels: issue
            .labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
        assignees: issue.assignees,
    }))
}

/// POST /projects/:id/issues
///
/// Create a new issue in Gitea.
/// Requires authentication - the agent must be a member of the project.
pub async fn create_issue(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Json<IssueResponse>, AppError> {
    let project_id = ProjectId(project_id);

    // Check if agent is a member of the project
    let is_member = state.project_repo.is_member(&project_id, &agent.id).await?;
    if !is_member {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "You must be a member of the project to create issues".to_string(),
        )));
    }

    // Get agent's Gitea token for proper attribution
    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let new_issue = NewIssue {
        title: request.title,
        body: request.body,
    };

    let issue = state
        .issue_repo
        .create(&project_id, &new_issue, gitea_token.as_deref())
        .await?;

    // Increment open ticket count
    state.project_repo.adjust_ticket_count(&project_id, 1).await?;

    Ok(Json(IssueResponse {
        project_id: issue.id.project_id.0.to_string(),
        number: issue.id.number,
        title: issue.title,
        body: issue.body,
        state: issue.state.to_string(),
        url: issue.url,
        labels: issue
            .labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
        assignees: issue.assignees,
    }))
}

/// PATCH /projects/:id/issues/:number
///
/// Update an issue (title/body).
pub async fn update_issue(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<UpdateIssueRequest>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let issue = state
        .issue_repo
        .update(
            &issue_id,
            request.title.as_deref(),
            request.body.as_deref(),
            gitea_token.as_deref(),
        )
        .await?;

    Ok(Json(IssueResponse {
        project_id: issue.id.project_id.0.to_string(),
        number: issue.id.number,
        title: issue.title,
        body: issue.body,
        state: issue.state.to_string(),
        url: issue.url,
        labels: issue
            .labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
        assignees: issue.assignees,
    }))
}

/// POST /projects/:id/issues/:number/close
///
/// Close an issue.
pub async fn close_issue(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let issue = state
        .issue_repo
        .close(&issue_id, gitea_token.as_deref())
        .await?;

    // Decrement open ticket count
    state
        .project_repo
        .adjust_ticket_count(&ProjectId(project_id), -1)
        .await?;

    Ok(Json(IssueResponse {
        project_id: issue.id.project_id.0.to_string(),
        number: issue.id.number,
        title: issue.title,
        body: issue.body,
        state: issue.state.to_string(),
        url: issue.url,
        labels: issue
            .labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
        assignees: issue.assignees,
    }))
}

/// POST /projects/:id/issues/:number/reopen
///
/// Reopen an issue.
pub async fn reopen_issue(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<IssueResponse>, AppError> {
    let project_id_typed = ProjectId(project_id);
    let issue_id = IssueId::new(project_id_typed, number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let issue = state
        .issue_repo
        .reopen(&issue_id, gitea_token.as_deref())
        .await?;

    // Increment open ticket count (issue reopened)
    state
        .project_repo
        .adjust_ticket_count(&project_id_typed, 1)
        .await?;

    Ok(Json(IssueResponse {
        project_id: issue.id.project_id.0.to_string(),
        number: issue.id.number,
        title: issue.title,
        body: issue.body,
        state: issue.state.to_string(),
        url: issue.url,
        labels: issue
            .labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
        assignees: issue.assignees,
    }))
}

// ============================================================================
// Comment Handlers
// ============================================================================

/// GET /projects/:id/issues/:number/comments
///
/// List comments on an issue.
pub async fn list_comments(
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<Vec<CommentResponse>>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let comments = state.issue_repo.list_comments(&issue_id).await?;

    Ok(Json(
        comments
            .into_iter()
            .map(|c| CommentResponse {
                id: c.id,
                body: c.body,
                author: c.author,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect(),
    ))
}

/// POST /projects/:id/issues/:number/comments
///
/// Add a comment to an issue.
pub async fn add_comment(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<AddCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let comment = state
        .issue_repo
        .add_comment(&issue_id, &request.body, gitea_token.as_deref())
        .await?;

    Ok(Json(CommentResponse {
        id: comment.id,
        body: comment.body,
        author: comment.author,
        created_at: comment.created_at,
        updated_at: comment.updated_at,
    }))
}

/// PATCH /projects/:id/issues/:number/comments/:comment_id
///
/// Edit a comment.
pub async fn edit_comment(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number, comment_id)): Path<(Uuid, i64, i64)>,
    Json(request): Json<EditCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let comment = state
        .issue_repo
        .edit_comment(&issue_id, comment_id, &request.body, gitea_token.as_deref())
        .await?;

    Ok(Json(CommentResponse {
        id: comment.id,
        body: comment.body,
        author: comment.author,
        created_at: comment.created_at,
        updated_at: comment.updated_at,
    }))
}

/// DELETE /projects/:id/issues/:number/comments/:comment_id
///
/// Delete a comment.
pub async fn delete_comment(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number, comment_id)): Path<(Uuid, i64, i64)>,
) -> Result<(), AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    state
        .issue_repo
        .delete_comment(&issue_id, comment_id, gitea_token.as_deref())
        .await?;

    Ok(())
}

// ============================================================================
// Label Handlers
// ============================================================================

/// GET /projects/:id/issues/:number/labels
///
/// List labels on an issue.
pub async fn list_labels(
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<Vec<LabelResponse>>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let labels = state.issue_repo.list_labels(&issue_id).await?;

    Ok(Json(
        labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
    ))
}

/// POST /projects/:id/issues/:number/labels
///
/// Add labels to an issue.
pub async fn add_labels(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<AddLabelsRequest>,
) -> Result<Json<Vec<LabelResponse>>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let labels = state
        .issue_repo
        .add_labels(&issue_id, request.labels, gitea_token.as_deref())
        .await?;

    Ok(Json(
        labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
    ))
}

/// DELETE /projects/:id/issues/:number/labels/:label
///
/// Remove a label from an issue.
pub async fn remove_label(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number, label)): Path<(Uuid, i64, String)>,
) -> Result<(), AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    state
        .issue_repo
        .remove_label(&issue_id, &label, gitea_token.as_deref())
        .await?;

    Ok(())
}

/// GET /projects/:project_id/labels
///
/// List available labels for a project.
pub async fn list_available_labels(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<LabelResponse>>, AppError> {
    let labels = state
        .issue_repo
        .list_available_labels(&ProjectId(project_id))
        .await?;

    Ok(Json(
        labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
    ))
}

// ============================================================================
// Assignee Handlers
// ============================================================================

/// POST /projects/:id/issues/:number/assignees
///
/// Assign users to an issue.
pub async fn assign_issue(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<AssignRequest>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let issue = state
        .issue_repo
        .assign(&issue_id, request.assignees, gitea_token.as_deref())
        .await?;

    Ok(Json(IssueResponse {
        project_id: issue.id.project_id.0.to_string(),
        number: issue.id.number,
        title: issue.title,
        body: issue.body,
        state: issue.state.to_string(),
        url: issue.url,
        labels: issue
            .labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
        assignees: issue.assignees,
    }))
}

/// DELETE /projects/:id/issues/:number/assignees/:assignee
///
/// Remove an assignee from an issue.
pub async fn unassign_issue(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number, assignee)): Path<(Uuid, i64, String)>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = IssueId::new(ProjectId(project_id), number);

    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let issue = state
        .issue_repo
        .unassign(&issue_id, &assignee, gitea_token.as_deref())
        .await?;

    Ok(Json(IssueResponse {
        project_id: issue.id.project_id.0.to_string(),
        number: issue.id.number,
        title: issue.title,
        body: issue.body,
        state: issue.state.to_string(),
        url: issue.url,
        labels: issue
            .labels
            .into_iter()
            .map(|l| LabelResponse {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect(),
        assignees: issue.assignees,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_list_query_defaults() {
        let query: ListIssuesQuery = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(query.state, "open");
    }

    #[test]
    fn parse_list_query_with_state() {
        let query: ListIssuesQuery = serde_json::from_str(r#"{"state": "closed"}"#).unwrap();
        assert_eq!(query.state, "closed");
    }

    #[test]
    fn parse_create_issue_request() {
        let json = r#"{"title": "Bug fix", "body": "Fix the bug"}"#;
        let request: CreateIssueRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.title, "Bug fix");
        assert_eq!(request.body, "Fix the bug");
    }

    #[test]
    fn parse_add_comment_request() {
        let json = r#"{"body": "This is a comment"}"#;
        let request: AddCommentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.body, "This is a comment");
    }

    #[test]
    fn parse_add_labels_request() {
        let json = r#"{"labels": ["bug", "help wanted"]}"#;
        let request: AddLabelsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.labels, vec!["bug", "help wanted"]);
    }

    #[test]
    fn parse_assign_request() {
        let json = r#"{"assignees": ["agent-1", "agent-2"]}"#;
        let request: AssignRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.assignees, vec!["agent-1", "agent-2"]);
    }

    #[test]
    fn serialize_issue_response() {
        let response = IssueResponse {
            project_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            number: 1,
            title: "Test Issue".to_string(),
            body: Some("Description".to_string()),
            state: "open".to_string(),
            url: "https://gitea.example.com/org/repo/issues/1".to_string(),
            labels: vec![LabelResponse {
                name: "bug".to_string(),
                color: "ff0000".to_string(),
                description: Some("A bug".to_string()),
            }],
            assignees: vec!["agent-1".to_string()],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Test Issue"));
        assert!(json.contains("\"number\":1"));
        assert!(json.contains("bug"));
        assert!(json.contains("agent-1"));
    }
}
