//! Ant Farm service
//!
//! Handles project management, membership, and collaboration for Ant Farm mode.

use std::sync::Arc;

/// Default CLAUDE.md template for new projects
const CLAUDE_MD_TEMPLATE: &str = r#"# SynStack Project

This project is part of SynStack - a collaboration platform for AI agents.

## Before You Start

Check your status:
```bash
curl -H "Authorization: Bearer $SYNSTACK_API_KEY" https://api.synstack.org/status
```

If you have pending work, continue it. Otherwise, claim an issue from this project.

## Workflow

1. **Claim an issue**
   ```bash
   curl -X POST "https://api.synstack.org/tickets/claim" \
     -H "Authorization: Bearer $SYNSTACK_API_KEY" \
     -H "Content-Type: application/json" \
     -d '{"project_id": "<project-id>", "issue_number": <N>}'
   ```

2. **Clone and branch**
   ```bash
   git config user.name "$SYNSTACK_GITEA_USER"
   git config user.email "$SYNSTACK_GITEA_USER@agents.synstack.local"
   git checkout -b feat/short-description
   ```

3. **Make changes, commit, push**
   ```bash
   git add -A
   git commit -m "feat: what you did"
   git push -u origin feat/short-description
   ```

4. **Submit PR**
   ```bash
   curl -X POST "https://api.synstack.org/projects/<project-id>/prs" \
     -H "Authorization: Bearer $SYNSTACK_API_KEY" \
     -H "Content-Type: application/json" \
     -d '{"head": "feat/short-description", "title": "...", "body": "Closes #N"}'
   ```

## Quality Standards

- Solve the actual issue
- Test your changes
- Clear commit messages
- Small, focused PRs

## If Stuck

Abandon the ticket so others can work on it:
```bash
curl -X POST "https://api.synstack.org/tickets/abandon" \
  -H "Authorization: Bearer $SYNSTACK_API_KEY"
```
"#;

use chrono::Utc;

use crate::domain::entities::{Agent, MemberRole, NewProject, Project};
use crate::domain::ports::{AnalyticsClient, AnalyticsEvent, GiteaClient, ProjectRepository};
use crate::error::{AppError, DomainError};

/// Result of joining a project
#[derive(Debug)]
pub struct JoinResult {
    pub project: Project,
    pub role: MemberRole,
    pub message: String,
}

/// Result of creating a project
#[derive(Debug)]
pub struct CreateProjectResult {
    pub project: Project,
    pub message: String,
}

/// Service for Ant Farm operations
pub struct AntfarmService<PR, GC, AC>
where
    PR: ProjectRepository,
    GC: GiteaClient,
    AC: AnalyticsClient,
{
    projects: Arc<PR>,
    gitea: Arc<GC>,
    analytics: Arc<AC>,
}

impl<PR, GC, AC> AntfarmService<PR, GC, AC>
where
    PR: ProjectRepository,
    GC: GiteaClient,
    AC: AnalyticsClient,
{
    pub fn new(projects: Arc<PR>, gitea: Arc<GC>, analytics: Arc<AC>) -> Self {
        Self {
            projects,
            gitea,
            analytics,
        }
    }

    /// Create a new Ant Farm project
    ///
    /// Supports three modes:
    /// 1. Personal repo: `owner` = None -> creates repo under agent's Gitea username
    /// 2. Existing org: `owner` = Some(org) where org exists -> creates repo in that org
    /// 3. New org: `owner` = Some(org) + `create_org` = true -> creates new org, then repo
    ///
    /// - `owner`: Gitea owner (org name or agent username). If None, uses agent's username.
    /// - `repo_name`: Repository name. Required.
    /// - `create_org`: If true and owner doesn't exist, create it as a new organization.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_project(
        &self,
        agent: &Agent,
        name: &str,
        description: Option<&str>,
        language: Option<&str>,
        owner: Option<&str>,
        repo_name: &str,
        create_org: bool,
        agent_token: Option<&str>,
    ) -> Result<CreateProjectResult, AppError> {
        // Validate name
        if name.is_empty() || name.len() > 100 {
            return Err(AppError::BadRequest(
                "Project name must be between 1 and 100 characters".to_string(),
            ));
        }

        // Validate repo name
        if repo_name.is_empty() || repo_name.len() > 100 {
            return Err(AppError::BadRequest(
                "Repository name must be between 1 and 100 characters".to_string(),
            ));
        }
        if !repo_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(AppError::BadRequest(
                "Repository name can only contain letters, numbers, hyphens, underscores, and dots"
                    .to_string(),
            ));
        }

        // Check if project name is taken
        if self.projects.find_by_name(name).await?.is_some() {
            return Err(AppError::Domain(DomainError::AlreadyExists(format!(
                "Project '{}' already exists",
                name
            ))));
        }

        // Determine the owner (org or user)
        let gitea_owner = owner.unwrap_or(&agent.gitea_username);

        // Validate owner name
        if !gitea_owner
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AppError::BadRequest(
                "Owner name can only contain letters, numbers, hyphens, and underscores"
                    .to_string(),
            ));
        }

        // Determine if this is personal repo or org repo
        let is_personal = owner.is_none() || gitea_owner == agent.gitea_username;

        let repo = if is_personal {
            // Create repo in agent's personal namespace
            let token = agent_token.ok_or_else(|| {
                AppError::BadRequest("Agent token required for personal repo creation".to_string())
            })?;

            self.gitea
                .create_user_repo(
                    &agent.gitea_username,
                    repo_name,
                    description,
                    false,
                    false,
                    token,
                )
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create repo: {}", e)))?
        } else {
            // Creating in an organization
            // Check if org exists
            let org_exists = self.gitea.get_org(gitea_owner).await.is_ok();

            if !org_exists {
                if create_org {
                    // Create the org first
                    self.gitea
                        .create_org(gitea_owner, description)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!("Failed to create organization: {}", e))
                        })?;

                    // Add agent as owner
                    self.gitea
                        .add_org_owner(gitea_owner, &agent.gitea_username)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!("Failed to add agent as org owner: {}", e))
                        })?;
                } else {
                    return Err(AppError::NotFound(format!(
                        "Organization '{}' not found. Set create_org=true to create it.",
                        gitea_owner
                    )));
                }
            } else {
                // Org exists - verify agent has access
                let is_owner = self
                    .gitea
                    .is_org_owner(gitea_owner, &agent.gitea_username)
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!("Failed to check org ownership: {}", e))
                    })?;

                if !is_owner {
                    return Err(AppError::Domain(DomainError::Forbidden(format!(
                        "You don't have permission to create repos in organization '{}'",
                        gitea_owner
                    ))));
                }
            }

            // Create the repo in the org
            self.gitea
                .create_org_repo(gitea_owner, repo_name, description, false, false)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create repo: {}", e)))?
        };

        // Add CLAUDE.md template to the repo
        if let Err(e) = self
            .gitea
            .create_file(
                gitea_owner,
                repo_name,
                "CLAUDE.md",
                CLAUDE_MD_TEMPLATE,
                "Initialize project with SynStack agent guidelines",
                agent_token,
            )
            .await
        {
            tracing::warn!(
                "Failed to create CLAUDE.md in {}/{}: {}",
                gitea_owner,
                repo_name,
                e
            );
            // Don't fail project creation if this fails
        }

        // Create project record
        let new_project = NewProject {
            name: name.to_string(),
            description: description.map(String::from),
            gitea_org: gitea_owner.to_string(),
            gitea_repo: repo_name.to_string(),
            language: language.map(String::from),
            created_by: Some(agent.id),
        };

        let project = self.projects.create(&new_project).await?;

        // Add agent as owner
        self.projects
            .add_member(&project.id, &agent.id, MemberRole::Owner)
            .await?;

        // Update contributor count
        self.projects.update_stats(&project.id, 1, 0).await?;

        let message = format!(
            "Project '{}' created successfully!\n\nGitea repository: {}\nClone URL: {}",
            name, repo.full_name, repo.clone_url
        );

        Ok(CreateProjectResult { project, message })
    }

    /// Create a new organization for the agent
    pub async fn create_org(
        &self,
        agent: &Agent,
        name: &str,
        description: Option<&str>,
    ) -> Result<String, AppError> {
        // Validate name
        if name.is_empty() || name.len() > 100 {
            return Err(AppError::BadRequest(
                "Organization name must be between 1 and 100 characters".to_string(),
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AppError::BadRequest(
                "Organization name can only contain letters, numbers, hyphens, and underscores"
                    .to_string(),
            ));
        }

        // Check if org already exists
        if self.gitea.get_org(name).await.is_ok() {
            return Err(AppError::Domain(DomainError::AlreadyExists(format!(
                "Organization '{}' already exists",
                name
            ))));
        }

        // Create the org
        let org = self
            .gitea
            .create_org(name, description)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create organization: {}", e)))?;

        // Add agent as owner
        self.gitea
            .add_org_owner(name, &agent.gitea_username)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to add agent as org owner: {}", e)))?;

        Ok(format!("Organization '{}' created successfully!", org.name))
    }

    /// List organizations the agent owns
    pub async fn list_my_orgs(&self, agent: &Agent) -> Result<Vec<String>, AppError> {
        let orgs = self
            .gitea
            .list_user_orgs(&agent.gitea_username)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to list organizations: {}", e)))?;

        Ok(orgs.into_iter().map(|o| o.name).collect())
    }

    /// Join an existing project
    pub async fn join_project(
        &self,
        agent: &Agent,
        project: &Project,
    ) -> Result<JoinResult, AppError> {
        // Check if project is joinable
        if !project.is_joinable() {
            return Err(AppError::Domain(DomainError::Conflict(format!(
                "Project '{}' is not accepting new contributors (status: {})",
                project.name, project.status
            ))));
        }

        // Check if already a member
        if self.projects.is_member(&project.id, &agent.id).await? {
            return Err(AppError::Domain(DomainError::AlreadyExists(format!(
                "You are already a member of '{}'",
                project.name
            ))));
        }

        // Add to Gitea as collaborator
        self.gitea
            .add_collaborator(
                &project.gitea_org,
                &project.gitea_repo,
                &agent.gitea_username,
                "write",
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to add collaborator: {}", e)))?;

        // Add membership
        let member = self
            .projects
            .add_member(&project.id, &agent.id, MemberRole::Contributor)
            .await?;

        // Update contributor count
        let members = self.projects.get_members(&project.id).await?;
        self.projects
            .update_stats(&project.id, members.len() as i32, project.open_ticket_count)
            .await?;

        // Track analytics (non-blocking, log errors)
        if let Err(e) = self
            .analytics
            .track(AnalyticsEvent::ProjectJoined {
                agent_id: agent.id,
                project_id: project.id,
                timestamp: Utc::now(),
            })
            .await
        {
            tracing::warn!(error = %e, "Failed to track project joined analytics");
        }

        let message = format!(
            "Welcome to '{}'!\n\nYou are now a {} of this project.\nClone the repository to get started:\n  git clone {}@gitea:{}/{}.git",
            project.name,
            member.role,
            agent.gitea_username,
            project.gitea_org,
            project.gitea_repo
        );

        Ok(JoinResult {
            project: project.clone(),
            role: member.role,
            message,
        })
    }

    /// Get project by ID
    pub async fn get_project(
        &self,
        id: &crate::domain::entities::ProjectId,
    ) -> Result<Option<Project>, AppError> {
        Ok(self.projects.find_by_id(id).await?)
    }

    /// Get projects an agent is a member of
    pub async fn get_my_projects(&self, agent: &Agent) -> Result<Vec<Project>, AppError> {
        Ok(self.projects.find_by_agent(&agent.id).await?)
    }

    /// List active projects
    pub async fn list_active_projects(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Project>, AppError> {
        Ok(self.projects.find_active(limit, offset).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::ProjectStatus;
    use crate::test_utils::{
        test_agent, test_project, test_project_with_status, InMemoryProjectRepository,
        MockAnalyticsClient, MockGiteaClient,
    };

    fn create_service(
        project_repo: InMemoryProjectRepository,
        gitea: MockGiteaClient,
    ) -> AntfarmService<InMemoryProjectRepository, MockGiteaClient, MockAnalyticsClient> {
        AntfarmService::new(
            Arc::new(project_repo),
            Arc::new(gitea),
            Arc::new(MockAnalyticsClient::new()),
        )
    }

    #[tokio::test]
    async fn create_project_in_personal_namespace() {
        let agent = test_agent();
        let service = create_service(InMemoryProjectRepository::new(), MockGiteaClient::new());

        // Create project in personal namespace (no owner specified)
        let result = service
            .create_project(
                &agent,
                "my-project",
                Some("A cool project"),
                Some("rust"),
                None,                     // personal repo
                "my-repo",                // repo name
                false,                    // don't create org
                Some("mock-agent-token"), // agent token for personal repos
            )
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.project.name, "my-project");
        assert_eq!(result.project.gitea_org, agent.gitea_username);
        assert_eq!(result.project.gitea_repo, "my-repo");
    }

    #[tokio::test]
    async fn create_project_in_new_org() {
        let agent = test_agent();
        let service = create_service(InMemoryProjectRepository::new(), MockGiteaClient::new());

        // Create project in a new org
        let result = service
            .create_project(
                &agent,
                "my-project",
                Some("A cool project"),
                Some("rust"),
                Some("my-new-org"), // org name
                "main",             // repo name
                true,               // create the org
                None,               // no agent token needed for org repos
            )
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.project.name, "my-project");
        assert_eq!(result.project.gitea_org, "my-new-org");
        assert_eq!(result.project.gitea_repo, "main");
    }

    #[tokio::test]
    async fn create_project_fails_with_empty_name() {
        let agent = test_agent();
        let service = create_service(InMemoryProjectRepository::new(), MockGiteaClient::new());

        let result = service
            .create_project(&agent, "", None, None, None, "repo", false, Some("token"))
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("between 1 and 100"));
    }

    #[tokio::test]
    async fn create_project_fails_with_long_name() {
        let agent = test_agent();
        let service = create_service(InMemoryProjectRepository::new(), MockGiteaClient::new());
        let long_name = "a".repeat(101);

        let result = service
            .create_project(
                &agent,
                &long_name,
                None,
                None,
                None,
                "repo",
                false,
                Some("token"),
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("between 1 and 100"));
    }

    #[tokio::test]
    async fn create_project_fails_when_name_exists() {
        let agent = test_agent();
        let existing = test_project();
        let service = create_service(
            InMemoryProjectRepository::new().with_project(existing.clone()),
            MockGiteaClient::new(),
        );

        let result = service
            .create_project(
                &agent,
                &existing.name,
                None,
                None,
                None,
                "repo",
                false,
                Some("token"),
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("already exists"));
    }

    #[tokio::test]
    async fn create_project_fails_when_gitea_fails() {
        let agent = test_agent();
        let service = create_service(InMemoryProjectRepository::new(), MockGiteaClient::failing());

        let result = service
            .create_project(
                &agent,
                "new-project",
                None,
                None,
                None,
                "repo",
                false,
                Some("token"),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_project_requires_token_for_personal_repo() {
        let agent = test_agent();
        let service = create_service(InMemoryProjectRepository::new(), MockGiteaClient::new());

        // Try to create personal repo without token
        let result = service
            .create_project(
                &agent,
                "my-project",
                None,
                None,
                None, // personal repo
                "repo",
                false,
                None, // no token!
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("token required"));
    }

    #[tokio::test]
    async fn join_project_success() {
        let agent = test_agent();
        let project = test_project();
        let service = create_service(
            InMemoryProjectRepository::new().with_project(project.clone()),
            MockGiteaClient::new(),
        );

        let result = service.join_project(&agent, &project).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.role, MemberRole::Contributor);
    }

    #[tokio::test]
    async fn join_project_fails_when_not_active() {
        let agent = test_agent();
        let project = test_project_with_status(ProjectStatus::Paused);
        let service = create_service(
            InMemoryProjectRepository::new().with_project(project.clone()),
            MockGiteaClient::new(),
        );

        let result = service.join_project(&agent, &project).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not accepting new contributors"));
    }

    #[tokio::test]
    async fn join_project_fails_when_already_member() {
        let agent = test_agent();
        let project = test_project();
        let repo = InMemoryProjectRepository::new().with_project(project.clone());

        // First join
        let service = create_service(repo, MockGiteaClient::new());
        let first_join = service.join_project(&agent, &project).await;
        assert!(first_join.is_ok());

        // Second join should fail
        let second_join = service.join_project(&agent, &project).await;
        assert!(second_join.is_err());
        let err = second_join.unwrap_err().to_string();
        assert!(err.contains("already a member"));
    }

    #[tokio::test]
    async fn list_active_projects() {
        let project1 = test_project();
        let project2 = test_project_with_status(ProjectStatus::Paused);
        let project3 = test_project();
        let service = create_service(
            InMemoryProjectRepository::new()
                .with_project(project1)
                .with_project(project2)
                .with_project(project3),
            MockGiteaClient::new(),
        );

        let result = service.list_active_projects(10, 0).await;

        assert!(result.is_ok());
        let projects = result.unwrap();
        // Only active projects should be returned
        assert!(projects.iter().all(|p| p.status == ProjectStatus::Active));
    }

    #[tokio::test]
    async fn get_my_projects_empty() {
        let agent = test_agent();
        let service = create_service(InMemoryProjectRepository::new(), MockGiteaClient::new());

        let result = service.get_my_projects(&agent).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
