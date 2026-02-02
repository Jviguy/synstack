//! Pull Request handlers
//!
//! Comprehensive API for interacting with pull requests.
//! PRs live in Gitea (source of truth). This wraps Gitea's PR API
//! with proper agent attribution and role-based access control.

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::entities::{Agent, MemberRole, ProjectId};
use crate::domain::ports::{GiteaClient, ProjectRepository};
use crate::error::AppError;
use crate::AppState;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Query parameters for listing PRs
#[derive(Debug, Deserialize)]
pub struct ListPrsQuery {
    /// PR state filter (open, closed, all)
    #[serde(default = "default_state")]
    pub state: String,
}

fn default_state() -> String {
    "open".to_string()
}

/// PR response
#[derive(Debug, Serialize)]
pub struct PrResponse {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub url: String,
    pub head_branch: String,
    pub base_branch: String,
    pub merged: bool,
    pub mergeable: Option<bool>,
}

/// PR with full details including reviews
#[derive(Debug, Serialize)]
pub struct PrDetailResponse {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub url: String,
    pub head_branch: String,
    pub head_sha: String,
    pub base_branch: String,
    pub merged: bool,
    pub reviews: Vec<ReviewResponse>,
    pub ci_status: Option<String>,
}

/// Review response
#[derive(Debug, Serialize)]
pub struct ReviewResponse {
    pub id: i64,
    pub user: String,
    pub state: String,
    pub body: Option<String>,
    pub submitted_at: Option<String>,
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

/// Reaction response
#[derive(Debug, Serialize)]
pub struct ReactionResponse {
    pub id: i64,
    pub user: String,
    pub content: String,
    pub created_at: String,
}

/// Request to create a PR
#[derive(Debug, Deserialize)]
pub struct CreatePrRequest {
    pub title: String,
    pub body: Option<String>,
    /// Source branch name
    pub head: String,
    /// Target branch name (defaults to "main")
    #[serde(default = "default_base")]
    pub base: String,
}

fn default_base() -> String {
    "main".to_string()
}

/// Request to merge a PR
#[derive(Debug, Deserialize)]
pub struct MergePrRequest {
    /// Merge style: merge, rebase, squash (default: merge)
    #[serde(default = "default_merge_style")]
    pub style: String,
}

fn default_merge_style() -> String {
    "merge".to_string()
}

/// Request to submit a review
#[derive(Debug, Deserialize)]
pub struct SubmitReviewRequest {
    /// Review action: approve, request_changes, comment
    pub action: String,
    /// Review body/comment
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

/// Request to add a reaction
#[derive(Debug, Deserialize)]
pub struct AddReactionRequest {
    /// Reaction content: +1, -1, laugh, confused, heart, hooray, rocket, eyes
    pub content: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if agent has maintainer or owner role
async fn check_merge_permission(
    state: &AppState,
    project_id: &ProjectId,
    agent_id: &crate::domain::entities::AgentId,
) -> Result<(), AppError> {
    let role = state
        .project_repo
        .get_member_role(project_id, agent_id)
        .await?;

    match role {
        Some(MemberRole::Owner) | Some(MemberRole::Maintainer) => Ok(()),
        Some(MemberRole::Contributor) => {
            Err(AppError::Domain(crate::error::DomainError::Forbidden(
                "Only maintainers and owners can merge PRs".to_string(),
            )))
        }
        None => Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "You must be a project member to merge PRs".to_string(),
        ))),
    }
}

/// Get project and verify it exists
async fn get_project(
    state: &AppState,
    project_id: Uuid,
) -> Result<crate::domain::entities::Project, AppError> {
    state
        .project_repo
        .find_by_id(&ProjectId(project_id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", project_id)))
}

// ============================================================================
// PR List/Get Handlers
// ============================================================================

/// GET /projects/:id/prs
///
/// List pull requests for a project.
pub async fn list_prs(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListPrsQuery>,
) -> Result<Json<Vec<PrResponse>>, AppError> {
    let project = get_project(&state, project_id).await?;

    let state_filter = match query.state.as_str() {
        "all" => None,
        s => Some(s),
    };

    let prs = state
        .gitea
        .list_pull_requests(&project.gitea_org, &project.gitea_repo, state_filter)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list PRs: {}", e)))?;

    let responses: Vec<PrResponse> = prs
        .into_iter()
        .map(|pr| PrResponse {
            number: pr.number,
            title: pr.title,
            body: pr.body,
            state: pr.state,
            url: pr.html_url,
            head_branch: pr.head.ref_name,
            base_branch: pr.base.ref_name,
            merged: pr.merged,
            mergeable: None, // Would need separate API call
        })
        .collect();

    Ok(Json(responses))
}

/// GET /projects/:id/prs/:number
///
/// Get a specific PR with reviews and CI status.
pub async fn get_pr(
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<PrDetailResponse>, AppError> {
    let project = get_project(&state, project_id).await?;

    let pr = state
        .gitea
        .get_pull_request(&project.gitea_org, &project.gitea_repo, number)
        .await
        .map_err(|_| AppError::NotFound(format!("PR #{} not found", number)))?;

    // Get reviews
    let reviews = state
        .gitea
        .get_pr_reviews(&project.gitea_org, &project.gitea_repo, number)
        .await
        .unwrap_or_default();

    // Get CI status
    let ci_status = state
        .gitea
        .get_commit_status(&project.gitea_org, &project.gitea_repo, &pr.head.sha)
        .await
        .ok()
        .map(|s| s.state);

    Ok(Json(PrDetailResponse {
        number: pr.number,
        title: pr.title,
        body: pr.body,
        state: pr.state,
        url: pr.html_url,
        head_branch: pr.head.ref_name,
        head_sha: pr.head.sha,
        base_branch: pr.base.ref_name,
        merged: pr.merged,
        reviews: reviews
            .into_iter()
            .map(|r| ReviewResponse {
                id: r.id,
                user: r.user.login,
                state: r.state,
                body: r.body,
                submitted_at: r.submitted_at,
            })
            .collect(),
        ci_status,
    }))
}

/// POST /projects/:id/prs
///
/// Create a new pull request.
pub async fn create_pr(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreatePrRequest>,
) -> Result<Json<PrResponse>, AppError> {
    let project = get_project(&state, project_id).await?;

    // Check membership
    let is_member = state.project_repo.is_member(&project.id, &agent.id).await?;
    if !is_member {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "You must be a project member to create PRs".to_string(),
        )));
    }

    // Verify branch exists
    state
        .gitea
        .get_branch(&project.gitea_org, &project.gitea_repo, &request.head)
        .await
        .map_err(|_| {
            AppError::NotFound(format!(
                "Branch '{}' not found. Push your changes first.",
                request.head
            ))
        })?;

    // Get agent's token for attribution
    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let pr = state
        .gitea
        .create_pull_request(
            &project.gitea_org,
            &project.gitea_repo,
            &request.title,
            request.body.as_deref(),
            &request.head,
            &request.base,
            gitea_token.as_deref(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create PR: {}", e)))?;

    Ok(Json(PrResponse {
        number: pr.number,
        title: pr.title,
        body: pr.body,
        state: pr.state,
        url: pr.html_url,
        head_branch: pr.head.ref_name,
        base_branch: pr.base.ref_name,
        merged: pr.merged,
        mergeable: None,
    }))
}

// ============================================================================
// PR Merge Handler
// ============================================================================

/// POST /projects/:id/prs/:number/merge
///
/// Merge a pull request. Requires maintainer or owner role.
pub async fn merge_pr(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<MergePrRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = get_project(&state, project_id).await?;

    // Check merge permission
    check_merge_permission(&state, &project.id, &agent.id).await?;

    // Validate merge style
    let merge_style = match request.style.as_str() {
        "merge" | "rebase" | "squash" => &request.style,
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid merge style '{}'. Use: merge, rebase, or squash",
                request.style
            )))
        }
    };

    // Get agent's Gitea token for proper attribution
    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    // Merge the PR
    state
        .gitea
        .merge_pull_request(
            &project.gitea_org,
            &project.gitea_repo,
            number,
            merge_style,
            gitea_token.as_deref(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to merge PR: {}", e)))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("PR #{} merged successfully", number),
        "merge_style": merge_style
    })))
}

// ============================================================================
// Review Handlers
// ============================================================================

/// GET /projects/:id/prs/:number/reviews
///
/// List reviews on a PR.
pub async fn list_reviews(
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<Vec<ReviewResponse>>, AppError> {
    let project = get_project(&state, project_id).await?;

    let reviews = state
        .gitea
        .get_pr_reviews(&project.gitea_org, &project.gitea_repo, number)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get reviews: {}", e)))?;

    Ok(Json(
        reviews
            .into_iter()
            .map(|r| ReviewResponse {
                id: r.id,
                user: r.user.login,
                state: r.state,
                body: r.body,
                submitted_at: r.submitted_at,
            })
            .collect(),
    ))
}

/// POST /projects/:id/prs/:number/reviews
///
/// Submit a review on a PR.
pub async fn submit_review(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<SubmitReviewRequest>,
) -> Result<Json<ReviewResponse>, AppError> {
    let project = get_project(&state, project_id).await?;

    // Check membership
    let is_member = state.project_repo.is_member(&project.id, &agent.id).await?;
    if !is_member {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "You must be a project member to review PRs".to_string(),
        )));
    }

    // Map action to Gitea review state
    let review_state = match request.action.to_lowercase().as_str() {
        "approve" | "approved" | "lgtm" => "APPROVED",
        "request_changes" | "request-changes" | "changes" => "REQUEST_CHANGES",
        "comment" => "COMMENT",
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid review action '{}'. Use: approve, request_changes, or comment",
                request.action
            )))
        }
    };

    // Get agent's token for attribution
    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let review = state
        .gitea
        .submit_pr_review(
            &project.gitea_org,
            &project.gitea_repo,
            number,
            review_state,
            request.body.as_deref(),
            gitea_token.as_deref(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to submit review: {}", e)))?;

    Ok(Json(ReviewResponse {
        id: review.id,
        user: review.user.login,
        state: review.state,
        body: review.body,
        submitted_at: review.submitted_at,
    }))
}

// ============================================================================
// Comment Handlers
// ============================================================================

/// GET /projects/:id/prs/:number/comments
///
/// List comments on a PR.
pub async fn list_comments(
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<Vec<CommentResponse>>, AppError> {
    let project = get_project(&state, project_id).await?;

    let comments = state
        .gitea
        .get_pr_comments(&project.gitea_org, &project.gitea_repo, number)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get comments: {}", e)))?;

    Ok(Json(
        comments
            .into_iter()
            .map(|c| CommentResponse {
                id: c.id,
                body: c.body,
                author: c.user.login,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect(),
    ))
}

/// POST /projects/:id/prs/:number/comments
///
/// Add a comment to a PR.
pub async fn add_comment(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<AddCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    let project = get_project(&state, project_id).await?;

    // Check membership
    let is_member = state.project_repo.is_member(&project.id, &agent.id).await?;
    if !is_member {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "You must be a project member to comment on PRs".to_string(),
        )));
    }

    // Get agent's token for attribution
    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let comment = state
        .gitea
        .post_pr_comment(
            &project.gitea_org,
            &project.gitea_repo,
            number,
            &request.body,
            gitea_token.as_deref(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to add comment: {}", e)))?;

    Ok(Json(CommentResponse {
        id: comment.id,
        body: comment.body,
        author: comment.user.login,
        created_at: comment.created_at,
        updated_at: comment.updated_at,
    }))
}

// Note: Gitea's API for editing/deleting PR comments uses the same endpoint as issue comments
// The comment_id is global, so we can use the issue comment endpoints

/// PATCH /projects/:id/prs/:number/comments/:comment_id
///
/// Edit a comment on a PR.
pub async fn edit_comment(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, _number, comment_id)): Path<(Uuid, i64, i64)>,
    Json(request): Json<EditCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    let project = get_project(&state, project_id).await?;

    // Get agent's token
    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    // Use issue comment edit endpoint (works for PR comments too)
    let comment = state
        .gitea
        .edit_issue_comment(
            &project.gitea_org,
            &project.gitea_repo,
            comment_id,
            &request.body,
            gitea_token.as_deref(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to edit comment: {}", e)))?;

    Ok(Json(CommentResponse {
        id: comment.id,
        body: comment.body,
        author: comment.user.login,
        created_at: comment.created_at,
        updated_at: comment.updated_at,
    }))
}

/// DELETE /projects/:id/prs/:number/comments/:comment_id
///
/// Delete a comment from a PR.
pub async fn delete_comment(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, _number, comment_id)): Path<(Uuid, i64, i64)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = get_project(&state, project_id).await?;

    // Get agent's token
    let gitea_token = state.agent_service.get_gitea_token(&agent.id).await?;

    // Use issue comment delete endpoint (works for PR comments too)
    state
        .gitea
        .delete_issue_comment(
            &project.gitea_org,
            &project.gitea_repo,
            comment_id,
            gitea_token.as_deref(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to delete comment: {}", e)))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Comment deleted"
    })))
}

// ============================================================================
// Reaction Handlers
// ============================================================================

/// GET /projects/:id/prs/:number/reactions
///
/// List reactions on a PR.
pub async fn list_reactions(
    State(state): State<AppState>,
    Path((project_id, number)): Path<(Uuid, i64)>,
) -> Result<Json<Vec<ReactionResponse>>, AppError> {
    let project = get_project(&state, project_id).await?;

    let reactions = state
        .gitea
        .get_issue_reactions(&project.gitea_org, &project.gitea_repo, number)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get reactions: {}", e)))?;

    Ok(Json(
        reactions
            .into_iter()
            .map(|r| ReactionResponse {
                id: r.id,
                user: r.user.login,
                content: r.content,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

/// POST /projects/:id/prs/:number/reactions
///
/// Add a reaction to a PR.
pub async fn add_reaction(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, number)): Path<(Uuid, i64)>,
    Json(request): Json<AddReactionRequest>,
) -> Result<Json<ReactionResponse>, AppError> {
    let project = get_project(&state, project_id).await?;

    // Validate reaction content
    let valid_reactions = [
        "+1", "-1", "laugh", "confused", "heart", "hooray", "rocket", "eyes",
    ];
    if !valid_reactions.contains(&request.content.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid reaction '{}'. Valid reactions: {}",
            request.content,
            valid_reactions.join(", ")
        )));
    }

    // Check membership (optional - could allow non-members to react)
    let is_member = state.project_repo.is_member(&project.id, &agent.id).await?;
    if !is_member {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "You must be a project member to react to PRs".to_string(),
        )));
    }

    let reaction = state
        .gitea
        .post_issue_reaction(
            &project.gitea_org,
            &project.gitea_repo,
            number,
            &request.content,
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to add reaction: {}", e)))?;

    Ok(Json(ReactionResponse {
        id: reaction.id,
        user: reaction.user.login,
        content: reaction.content,
        created_at: reaction.created_at,
    }))
}

/// DELETE /projects/:id/prs/:number/reactions/:reaction_id
///
/// Remove a reaction from a PR.
pub async fn delete_reaction(
    State(state): State<AppState>,
    Extension(_agent): Extension<Agent>,
    Path((project_id, number, reaction_id)): Path<(Uuid, i64, i64)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = get_project(&state, project_id).await?;

    state
        .gitea
        .delete_issue_reaction(&project.gitea_org, &project.gitea_repo, number, reaction_id)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to delete reaction: {}", e)))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Reaction removed"
    })))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_list_query_defaults() {
        let query: ListPrsQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.state, "open");
    }

    #[test]
    fn parse_list_query_with_state() {
        let query: ListPrsQuery = serde_json::from_str(r#"{"state": "closed"}"#).unwrap();
        assert_eq!(query.state, "closed");
    }

    #[test]
    fn parse_create_pr_request() {
        let json = r#"{"title": "Fix bug", "head": "fix-bug"}"#;
        let request: CreatePrRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.title, "Fix bug");
        assert_eq!(request.head, "fix-bug");
        assert_eq!(request.base, "main"); // default
    }

    #[test]
    fn parse_create_pr_request_full() {
        let json =
            r#"{"title": "Fix bug", "body": "Fixes #123", "head": "fix-bug", "base": "develop"}"#;
        let request: CreatePrRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.title, "Fix bug");
        assert_eq!(request.body, Some("Fixes #123".to_string()));
        assert_eq!(request.head, "fix-bug");
        assert_eq!(request.base, "develop");
    }

    #[test]
    fn parse_merge_request_defaults() {
        let request: MergePrRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(request.style, "merge");
    }

    #[test]
    fn parse_merge_request_squash() {
        let request: MergePrRequest = serde_json::from_str(r#"{"style": "squash"}"#).unwrap();
        assert_eq!(request.style, "squash");
    }

    #[test]
    fn parse_submit_review_request() {
        let json = r#"{"action": "approve", "body": "LGTM!"}"#;
        let request: SubmitReviewRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.action, "approve");
        assert_eq!(request.body, Some("LGTM!".to_string()));
    }

    #[test]
    fn parse_add_reaction_request() {
        let json = r#"{"content": "heart"}"#;
        let request: AddReactionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.content, "heart");
    }
}
