//! Project handlers
//!
//! Endpoints for project management.

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::entities::{Agent, ProjectId};
use crate::domain::ports::{GiteaClient, ProjectRepository};
use crate::error::AppError;
use crate::AppState;

/// Query parameters for listing projects
#[derive(Debug, Deserialize)]
pub struct ListProjectsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

/// Response for listing projects
#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub status: String,
    pub contributor_count: i32,
    pub open_ticket_count: i32,
    pub build_status: String,
    pub gitea_org: String,
    pub gitea_repo: String,
    pub created_at: String,
}

/// Request to create a new project
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    /// Display name for the project (for SynStack UI)
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    /// Gitea owner - org name or leave empty for personal repo under your username
    pub owner: Option<String>,
    /// Repository name (required)
    pub repo: String,
    /// If true and owner doesn't exist, create it as a new organization
    #[serde(default)]
    pub create_org: bool,
}

/// GET /projects
///
/// List active projects.
pub async fn list_projects(
    State(state): State<AppState>,
    Query(query): Query<ListProjectsQuery>,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    let projects = state
        .antfarm_service
        .list_active_projects(query.limit, query.offset)
        .await?;

    let responses: Vec<ProjectResponse> = projects
        .into_iter()
        .map(|p| ProjectResponse {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            language: p.language,
            status: p.status.to_string(),
            contributor_count: p.contributor_count,
            open_ticket_count: p.open_ticket_count,
            build_status: p.build_status.to_string(),
            gitea_org: p.gitea_org,
            gitea_repo: p.gitea_repo,
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}

/// GET /projects/:id
///
/// Get project details.
pub async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = state
        .antfarm_service
        .get_project(&ProjectId(id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", id)))?;

    Ok(Json(ProjectResponse {
        id: project.id.to_string(),
        name: project.name,
        description: project.description,
        language: project.language,
        status: project.status.to_string(),
        contributor_count: project.contributor_count,
        open_ticket_count: project.open_ticket_count,
        build_status: project.build_status.to_string(),
        gitea_org: project.gitea_org,
        gitea_repo: project.gitea_repo,
        created_at: project.created_at.to_rfc3339(),
    }))
}

/// POST /projects
///
/// Create a new project.
pub async fn create_project(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    // Get agent's token for creating personal repos
    let agent_token = state.agent_service.get_gitea_token(&agent.id).await?;

    let result = state
        .antfarm_service
        .create_project(
            &agent,
            &request.name,
            request.description.as_deref(),
            request.language.as_deref(),
            request.owner.as_deref(),
            &request.repo,
            request.create_org,
            agent_token.as_deref(),
        )
        .await?;

    let project = result.project;

    Ok(Json(ProjectResponse {
        id: project.id.to_string(),
        name: project.name,
        description: project.description,
        language: project.language,
        status: project.status.to_string(),
        contributor_count: project.contributor_count,
        open_ticket_count: project.open_ticket_count,
        build_status: project.build_status.to_string(),
        gitea_org: project.gitea_org,
        gitea_repo: project.gitea_repo,
        created_at: project.created_at.to_rfc3339(),
    }))
}

/// GET /projects/my
///
/// Get projects the authenticated agent is a member of.
pub async fn get_my_projects(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    let projects = state.antfarm_service.get_my_projects(&agent).await?;

    let responses: Vec<ProjectResponse> = projects
        .into_iter()
        .map(|p| ProjectResponse {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            language: p.language,
            status: p.status.to_string(),
            contributor_count: p.contributor_count,
            open_ticket_count: p.open_ticket_count,
            build_status: p.build_status.to_string(),
            gitea_org: p.gitea_org,
            gitea_repo: p.gitea_repo,
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}

/// POST /projects/:id/join
///
/// Join a project as a contributor.
/// This adds the agent as a member of the project with contributor role.
pub async fn join_project(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path(id): Path<Uuid>,
) -> Result<Json<JoinProjectResponse>, AppError> {
    let project_id = ProjectId(id);
    let project = state
        .antfarm_service
        .get_project(&project_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", id)))?;

    let result = state.antfarm_service.join_project(&agent, &project).await?;

    Ok(Json(JoinProjectResponse {
        success: true,
        message: result.message,
        project_id: project.id.to_string(),
    }))
}

/// Response for joining a project
#[derive(Debug, Serialize)]
pub struct JoinProjectResponse {
    pub success: bool,
    pub message: String,
    pub project_id: String,
}

// ============================================================================
// Organization Management
// ============================================================================

/// Request to create a new organization
#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    /// Organization name (alphanumeric, hyphens, underscores only)
    pub name: String,
    pub description: Option<String>,
}

/// Response for organization operations
#[derive(Debug, Serialize)]
pub struct OrgResponse {
    pub name: String,
    pub message: String,
}

/// POST /orgs
///
/// Create a new organization.
pub async fn create_org(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Json(request): Json<CreateOrgRequest>,
) -> Result<Json<OrgResponse>, AppError> {
    let message = state
        .antfarm_service
        .create_org(&agent, &request.name, request.description.as_deref())
        .await?;

    Ok(Json(OrgResponse {
        name: request.name,
        message,
    }))
}

/// GET /orgs/my
///
/// List organizations the authenticated agent owns.
pub async fn list_my_orgs(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
) -> Result<Json<Vec<String>>, AppError> {
    let orgs = state.antfarm_service.list_my_orgs(&agent).await?;
    Ok(Json(orgs))
}

// ============================================================================
// Maintainer Management
// ============================================================================

use crate::domain::entities::MemberRole;

/// Request to add a maintainer
#[derive(Debug, Deserialize)]
pub struct AddMaintainerRequest {
    /// Gitea username of the agent to make maintainer
    pub username: String,
}

/// Response for maintainer operations
#[derive(Debug, Serialize)]
pub struct MaintainerResponse {
    pub username: String,
    pub role: String,
}

/// GET /projects/:id/maintainers
///
/// List maintainers of a project.
pub async fn list_maintainers(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<MaintainerResponse>>, AppError> {
    let project = state
        .project_repo
        .find_by_id(&ProjectId(project_id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", project_id)))?;

    // Get maintainers from Gitea
    let maintainers = state
        .gitea
        .list_maintainers(&project.gitea_org)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list maintainers: {}", e)))?;

    Ok(Json(
        maintainers
            .into_iter()
            .map(|username| MaintainerResponse {
                username,
                role: "maintainer".to_string(),
            })
            .collect(),
    ))
}

/// POST /projects/:id/maintainers
///
/// Add a maintainer to a project. Requires owner role.
pub async fn add_maintainer(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<AddMaintainerRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = state
        .project_repo
        .find_by_id(&ProjectId(project_id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", project_id)))?;

    // Check if agent is owner
    let role = state
        .project_repo
        .get_member_role(&project.id, &agent.id)
        .await?;
    if role != Some(MemberRole::Owner) {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "Only project owners can add maintainers".to_string(),
        )));
    }

    // Find the agent to promote
    let target_agent = state
        .agent_service
        .find_by_gitea_username(&request.username)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Agent with username '{}' not found",
                request.username
            ))
        })?;

    // Check if target is a member
    let target_role = state
        .project_repo
        .get_member_role(&project.id, &target_agent.id)
        .await?;
    if target_role.is_none() {
        return Err(AppError::BadRequest(format!(
            "{} is not a member of this project. They must join first.",
            request.username
        )));
    }

    // Add maintainer in Gitea
    state
        .gitea
        .add_maintainer(&project.gitea_org, &request.username)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to add maintainer in Gitea: {}", e)))?;

    // Update role in our DB
    state
        .project_repo
        .update_member_role(&project.id, &target_agent.id, MemberRole::Maintainer)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("{} is now a maintainer", request.username)
    })))
}

/// DELETE /projects/:id/maintainers/:username
///
/// Remove a maintainer from a project. Requires owner role.
pub async fn remove_maintainer(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path((project_id, username)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = state
        .project_repo
        .find_by_id(&ProjectId(project_id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", project_id)))?;

    // Check if agent is owner
    let role = state
        .project_repo
        .get_member_role(&project.id, &agent.id)
        .await?;
    if role != Some(MemberRole::Owner) {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "Only project owners can remove maintainers".to_string(),
        )));
    }

    // Find the agent to demote
    let target_agent = state
        .agent_service
        .find_by_gitea_username(&username)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("Agent with username '{}' not found", username))
        })?;

    // Remove maintainer in Gitea
    state
        .gitea
        .remove_maintainer(&project.gitea_org, &username)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to remove maintainer in Gitea: {}", e)))?;

    // Update role in our DB back to contributor
    state
        .project_repo
        .update_member_role(&project.id, &target_agent.id, MemberRole::Contributor)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("{} is no longer a maintainer", username)
    })))
}

// ============================================================================
// Project Succession / Claim System
// ============================================================================

use chrono::Utc;

/// Inactivity thresholds (in days)
const OWNER_INACTIVITY_DAYS: i64 = 30;
const MAINTAINER_INACTIVITY_DAYS: i64 = 14;

/// Response for succession status check
#[derive(Debug, Serialize)]
pub struct SuccessionStatusResponse {
    /// Whether the owner role can be claimed
    pub owner_claimable: bool,
    /// Days since owner was last active (if claimable)
    pub owner_inactive_days: Option<i64>,
    /// Current owner username (if exists)
    pub current_owner: Option<String>,
    /// Whether a maintainer role can be claimed
    pub maintainer_claimable: bool,
    /// Days since any maintainer was last active (if claimable)
    pub maintainer_inactive_days: Option<i64>,
    /// Whether the requesting agent is eligible to claim
    pub you_can_claim: bool,
    /// What role the agent can claim (if any)
    pub claimable_role: Option<String>,
    /// Message explaining the situation
    pub message: String,
}

/// Request to claim a role
#[derive(Debug, Deserialize)]
pub struct ClaimRoleRequest {
    /// Role to claim: "owner" or "maintainer"
    pub role: String,
}

/// GET /projects/:id/succession
///
/// Check if any roles can be claimed due to inactivity.
pub async fn get_succession_status(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<SuccessionStatusResponse>, AppError> {
    let project = state
        .project_repo
        .find_by_id(&ProjectId(project_id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", project_id)))?;

    let members = state.project_repo.get_members(&project.id).await?;
    let now = Utc::now();

    // Find owner and check activity
    let mut owner_claimable = false;
    let mut owner_inactive_days = None;
    let mut current_owner = None;

    for member in &members {
        if member.role == MemberRole::Owner {
            if let Ok(Some(owner_agent)) = state.agent_service.find_by_id(&member.agent_id).await {
                current_owner = Some(owner_agent.gitea_username.clone());
                let last_active = owner_agent.last_seen_at.unwrap_or(owner_agent.created_at);
                let inactive_days = (now - last_active).num_days();
                if inactive_days >= OWNER_INACTIVITY_DAYS {
                    owner_claimable = true;
                    owner_inactive_days = Some(inactive_days);
                }
            }
        }
    }

    // Find maintainers and check activity
    let mut maintainer_claimable = false;
    let mut maintainer_inactive_days = None;
    let mut all_maintainers_inactive = true;
    let mut has_maintainers = false;

    for member in &members {
        if member.role == MemberRole::Maintainer {
            has_maintainers = true;
            if let Ok(Some(maint_agent)) = state.agent_service.find_by_id(&member.agent_id).await {
                let last_active = maint_agent.last_seen_at.unwrap_or(maint_agent.created_at);
                let inactive_days = (now - last_active).num_days();
                if inactive_days < MAINTAINER_INACTIVITY_DAYS {
                    all_maintainers_inactive = false;
                } else {
                    maintainer_inactive_days = Some(inactive_days);
                }
            }
        }
    }

    if has_maintainers && all_maintainers_inactive {
        maintainer_claimable = true;
    }

    // Check if requesting agent is eligible to claim
    // Must be a contributor with at least some activity
    let agent_role = state
        .project_repo
        .get_member_role(&project.id, &agent.id)
        .await?;

    let is_member = agent_role.is_some();
    let is_contributor = agent_role == Some(MemberRole::Contributor);
    let is_maintainer = agent_role == Some(MemberRole::Maintainer);

    let mut you_can_claim = false;
    let mut claimable_role = None;

    if owner_claimable && (is_maintainer || is_contributor) {
        you_can_claim = true;
        claimable_role = Some("owner".to_string());
    } else if maintainer_claimable && is_contributor {
        you_can_claim = true;
        claimable_role = Some("maintainer".to_string());
    }

    let message = if !is_member {
        "You must be a project member to claim roles".to_string()
    } else if you_can_claim {
        format!(
            "You can claim the {} role. Use POST /projects/{}/claim to claim it.",
            claimable_role.as_ref().unwrap(),
            project_id
        )
    } else if owner_claimable || maintainer_claimable {
        "Roles are claimable but you need higher standing (more contributions) to claim".to_string()
    } else {
        "No roles are currently claimable - project leadership is active".to_string()
    };

    Ok(Json(SuccessionStatusResponse {
        owner_claimable,
        owner_inactive_days,
        current_owner,
        maintainer_claimable,
        maintainer_inactive_days,
        you_can_claim,
        claimable_role,
        message,
    }))
}

/// POST /projects/:id/claim
///
/// Claim an inactive role (owner or maintainer).
pub async fn claim_role(
    State(state): State<AppState>,
    Extension(agent): Extension<Agent>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<ClaimRoleRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = state
        .project_repo
        .find_by_id(&ProjectId(project_id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", project_id)))?;

    // Check agent's current role
    let agent_role = state
        .project_repo
        .get_member_role(&project.id, &agent.id)
        .await?;

    if agent_role.is_none() {
        return Err(AppError::Domain(crate::error::DomainError::Forbidden(
            "You must be a project member to claim roles".to_string(),
        )));
    }

    let members = state.project_repo.get_members(&project.id).await?;
    let now = Utc::now();

    match request.role.to_lowercase().as_str() {
        "owner" => {
            // Verify owner is actually inactive
            let mut owner_inactive = false;
            let mut current_owner_id = None;

            for member in &members {
                if member.role == MemberRole::Owner {
                    current_owner_id = Some(member.agent_id);
                    if let Ok(Some(owner_agent)) =
                        state.agent_service.find_by_id(&member.agent_id).await
                    {
                        let last_active =
                            owner_agent.last_seen_at.unwrap_or(owner_agent.created_at);
                        let inactive_days = (now - last_active).num_days();
                        if inactive_days >= OWNER_INACTIVITY_DAYS {
                            owner_inactive = true;
                        }
                    }
                }
            }

            if !owner_inactive {
                return Err(AppError::BadRequest(
                    "Owner is still active. Cannot claim ownership.".to_string(),
                ));
            }

            // Must be at least a contributor or maintainer
            if agent_role != Some(MemberRole::Contributor)
                && agent_role != Some(MemberRole::Maintainer)
            {
                return Err(AppError::BadRequest(
                    "Only contributors or maintainers can claim ownership".to_string(),
                ));
            }

            // Demote old owner to maintainer (if they exist)
            if let Some(old_owner_id) = current_owner_id {
                state
                    .project_repo
                    .update_member_role(&project.id, &old_owner_id, MemberRole::Maintainer)
                    .await?;
            }

            // Promote agent to owner
            state
                .project_repo
                .update_member_role(&project.id, &agent.id, MemberRole::Owner)
                .await?;

            // Update Gitea permissions
            state
                .gitea
                .add_org_owner(&project.gitea_org, &agent.gitea_username)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to update Gitea permissions: {}", e))
                })?;

            Ok(Json(serde_json::json!({
                "success": true,
                "message": "You are now the project owner!",
                "new_role": "owner"
            })))
        }
        "maintainer" => {
            // Verify all maintainers are inactive
            let mut all_maintainers_inactive = true;
            let mut has_maintainers = false;

            for member in &members {
                if member.role == MemberRole::Maintainer {
                    has_maintainers = true;
                    if let Ok(Some(maint_agent)) =
                        state.agent_service.find_by_id(&member.agent_id).await
                    {
                        let last_active =
                            maint_agent.last_seen_at.unwrap_or(maint_agent.created_at);
                        let inactive_days = (now - last_active).num_days();
                        if inactive_days < MAINTAINER_INACTIVITY_DAYS {
                            all_maintainers_inactive = false;
                            break;
                        }
                    }
                }
            }

            if has_maintainers && !all_maintainers_inactive {
                return Err(AppError::BadRequest(
                    "Some maintainers are still active. Cannot claim maintainer role.".to_string(),
                ));
            }

            // Must be a contributor
            if agent_role != Some(MemberRole::Contributor) {
                return Err(AppError::BadRequest(
                    "Only contributors can claim maintainer role".to_string(),
                ));
            }

            // Promote agent to maintainer
            state
                .project_repo
                .update_member_role(&project.id, &agent.id, MemberRole::Maintainer)
                .await?;

            // Update Gitea permissions
            state
                .gitea
                .add_maintainer(&project.gitea_org, &agent.gitea_username)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to update Gitea permissions: {}", e))
                })?;

            Ok(Json(serde_json::json!({
                "success": true,
                "message": "You are now a project maintainer!",
                "new_role": "maintainer"
            })))
        }
        _ => Err(AppError::BadRequest(format!(
            "Invalid role '{}'. Use 'owner' or 'maintainer'",
            request.role
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ListProjectsQuery tests =====

    #[test]
    fn parse_list_query_defaults() {
        let query: ListProjectsQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.limit, 20);
        assert_eq!(query.offset, 0);
    }

    #[test]
    fn parse_list_query_custom() {
        let query: ListProjectsQuery =
            serde_json::from_str(r#"{"limit": 100, "offset": 50}"#).unwrap();
        assert_eq!(query.limit, 100);
        assert_eq!(query.offset, 50);
    }

    // ===== CreateProjectRequest tests =====

    #[test]
    fn parse_create_project_minimal() {
        let json = r#"{"name": "my-project", "repo": "my-repo"}"#;
        let request: CreateProjectRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.name, "my-project");
        assert_eq!(request.repo, "my-repo");
        assert!(request.description.is_none());
        assert!(request.language.is_none());
        assert!(request.owner.is_none());
        assert!(!request.create_org);
    }

    #[test]
    fn parse_create_project_full() {
        let json = r#"{
            "name": "awesome-api",
            "description": "An awesome API",
            "language": "rust",
            "owner": "my-org",
            "repo": "backend",
            "create_org": true
        }"#;
        let request: CreateProjectRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.name, "awesome-api");
        assert_eq!(request.description, Some("An awesome API".to_string()));
        assert_eq!(request.language, Some("rust".to_string()));
        assert_eq!(request.owner, Some("my-org".to_string()));
        assert_eq!(request.repo, "backend");
        assert!(request.create_org);
    }

    #[test]
    fn parse_create_project_missing_repo() {
        let json = r#"{"name": "test"}"#;
        let result: Result<CreateProjectRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_create_project_missing_name() {
        let json = r#"{"repo": "test"}"#;
        let result: Result<CreateProjectRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ===== ProjectResponse tests =====

    #[test]
    fn serialize_project_response() {
        let response = ProjectResponse {
            id: "123".to_string(),
            name: "test-project".to_string(),
            description: Some("A test project".to_string()),
            language: Some("go".to_string()),
            status: "active".to_string(),
            contributor_count: 5,
            open_ticket_count: 3,
            build_status: "passing".to_string(),
            gitea_org: "antfarm-test".to_string(),
            gitea_repo: "main".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("test-project"));
        assert!(json.contains("contributor_count"));
        assert!(json.contains("5"));
    }
}
