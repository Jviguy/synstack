//! Work Loop Service
//!
//! Orchestrates the agent work loop:
//! - Ticket assignment (our DB)
//! - PR creation (Gitea)
//! - PR review (Gitea)
//!
//! Philosophy: Gitea is the source of truth for git operations.
//! We only track ticket assignments locally.

use std::sync::Arc;

use crate::domain::entities::{Agent, Project, Ticket, TicketStatus};
use crate::domain::ports::{
    GiteaClient, GiteaPRReview, GiteaPullRequest, ProjectRepository, TicketRepository,
};
use crate::error::{AppError, DomainError};

/// Result of assigning a ticket
#[derive(Debug)]
pub struct AssignResult {
    pub ticket: Ticket,
    pub message: String,
}

/// Result of submitting a PR
#[derive(Debug)]
pub struct SubmitResult {
    pub pr: GiteaPullRequest,
    pub message: String,
}

/// Result of reviewing a PR
#[derive(Debug)]
pub struct ReviewResult {
    pub review: GiteaPRReview,
    pub message: String,
}

/// Current work status for an agent
#[derive(Debug)]
pub struct WorkStatus {
    pub assigned_tickets: Vec<Ticket>,
    pub open_prs: Vec<(Project, GiteaPullRequest)>,
}

/// Service for managing the agent work loop
pub struct WorkLoopService<TR, PR, GC>
where
    TR: TicketRepository,
    PR: ProjectRepository,
    GC: GiteaClient,
{
    tickets: Arc<TR>,
    projects: Arc<PR>,
    /// Gitea client for git operations (exposed for handlers that need to query PRs)
    pub gitea: Arc<GC>,
}

impl<TR, PR, GC> WorkLoopService<TR, PR, GC>
where
    TR: TicketRepository,
    PR: ProjectRepository,
    GC: GiteaClient,
{
    pub fn new(tickets: Arc<TR>, projects: Arc<PR>, gitea: Arc<GC>) -> Self {
        Self {
            tickets,
            projects,
            gitea,
        }
    }

    /// Assign a ticket to an agent (work-on command)
    pub async fn assign_ticket(
        &self,
        agent: &Agent,
        ticket: &Ticket,
        project: &Project,
    ) -> Result<AssignResult, AppError> {
        // Validate agent is a project member
        if !self.projects.is_member(&project.id, &agent.id).await? {
            return Err(AppError::Domain(DomainError::Forbidden(format!(
                "You must join project '{}' before working on tickets",
                project.name
            ))));
        }

        // Check ticket is available
        if !ticket.is_available() {
            if ticket.assigned_to == Some(agent.id) {
                return Err(AppError::BadRequest(
                    "You are already assigned to this ticket".to_string(),
                ));
            }
            return Err(AppError::BadRequest(format!(
                "Ticket '{}' is not available (status: {}, assigned: {})",
                ticket.title,
                ticket.status,
                ticket.assigned_to.map(|_| "yes").unwrap_or("no")
            )));
        }

        // Assign the ticket
        self.tickets.assign(&ticket.id, &agent.id).await?;
        self.tickets
            .update_status(&ticket.id, TicketStatus::InProgress)
            .await?;

        // Fetch updated ticket
        let updated =
            self.tickets.find_by_id(&ticket.id).await?.ok_or_else(|| {
                AppError::Internal("Ticket disappeared after assignment".to_string())
            })?;

        let ticket_prefix = ticket.id.0.to_string();
        let ticket_prefix = ticket_prefix.split('-').next().unwrap_or("fix");
        let message = format!(
            "You are now working on: {}\n\n\
            Clone the repo and create a branch:\n\
            ```\n\
            git clone git@gitea:{}/{}.git\n\
            git checkout -b fix/{}\n\
            ```\n\n\
            When ready, push your branch and run `submit <branch-name>`",
            ticket.title, project.gitea_org, project.gitea_repo, ticket_prefix
        );

        Ok(AssignResult {
            ticket: updated,
            message,
        })
    }

    /// Abandon current ticket assignment
    pub async fn abandon_ticket(&self, agent: &Agent) -> Result<String, AppError> {
        // Find tickets assigned to this agent
        let tickets = self.tickets.find_open_by_agent(&agent.id).await?;

        if tickets.is_empty() {
            return Err(AppError::BadRequest(
                "You don't have any assigned tickets to abandon".to_string(),
            ));
        }

        let mut abandoned = Vec::new();
        for ticket in tickets {
            self.tickets.unassign(&ticket.id).await?;
            self.tickets
                .update_status(&ticket.id, TicketStatus::Open)
                .await?;
            abandoned.push(ticket.title);
        }

        Ok(format!(
            "Abandoned {} ticket(s): {}",
            abandoned.len(),
            abandoned.join(", ")
        ))
    }

    /// Submit a PR from a branch (calls Gitea directly)
    ///
    /// If `gitea_token` is provided, the PR will be created using the agent's
    /// own Gitea token for proper attribution. Otherwise, falls back to admin token.
    pub async fn submit_pr(
        &self,
        agent: &Agent,
        project: &Project,
        branch: &str,
        title: Option<&str>,
        body: Option<&str>,
        gitea_token: Option<&str>,
    ) -> Result<SubmitResult, AppError> {
        // Validate agent is a project member
        if !self.projects.is_member(&project.id, &agent.id).await? {
            return Err(AppError::Domain(DomainError::Forbidden(format!(
                "You must join project '{}' before submitting PRs",
                project.name
            ))));
        }

        // Verify branch exists in Gitea
        self.gitea
            .get_branch(&project.gitea_org, &project.gitea_repo, branch)
            .await
            .map_err(|_| {
                AppError::BadRequest(format!(
                    "Branch '{}' not found. Make sure you've pushed your changes.",
                    branch
                ))
            })?;

        // Generate title from branch name if not provided
        let pr_title = title.unwrap_or(branch);

        // Create PR in Gitea using agent's token for proper attribution
        let pr = self
            .gitea
            .create_pull_request(
                &project.gitea_org,
                &project.gitea_repo,
                pr_title,
                body,
                branch,
                "main", // TODO: use project's default branch
                gitea_token,
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create PR in Gitea: {}", e)))?;

        let message = format!(
            "PR created: {}\n\n\
            URL: {}\n\n\
            Wait for peer review. Check status with `my-work`.",
            pr.title, pr.html_url
        );

        Ok(SubmitResult { pr, message })
    }

    /// Review a PR (calls Gitea directly)
    ///
    /// If `gitea_token` is provided, the review will be submitted using the agent's
    /// own Gitea token for proper attribution. Otherwise, falls back to admin token.
    pub async fn review_pr(
        &self,
        agent: &Agent,
        project: &Project,
        pr_number: i64,
        action: &str,
        comment: Option<&str>,
        gitea_token: Option<&str>,
    ) -> Result<ReviewResult, AppError> {
        // Validate agent is a project member
        if !self.projects.is_member(&project.id, &agent.id).await? {
            return Err(AppError::Domain(DomainError::Forbidden(format!(
                "You must join project '{}' before reviewing PRs",
                project.name
            ))));
        }

        // Get the PR to verify it exists and agent isn't reviewing their own
        let pr = self
            .gitea
            .get_pull_request(&project.gitea_org, &project.gitea_repo, pr_number)
            .await
            .map_err(|_| AppError::NotFound(format!("PR #{} not found", pr_number)))?;

        // Convert action to Gitea review event
        let event = match action.to_lowercase().as_str() {
            "approve" | "lgtm" => "APPROVE",
            "request-changes" | "request_changes" | "changes" => "REQUEST_CHANGES",
            "comment" => "COMMENT",
            _ => {
                return Err(AppError::BadRequest(format!(
                    "Invalid review action '{}'. Use: approve, request-changes, or comment",
                    action
                )))
            }
        };

        // Submit review to Gitea using agent's token for proper attribution
        let review = self
            .gitea
            .submit_pr_review(
                &project.gitea_org,
                &project.gitea_repo,
                pr_number,
                event,
                comment,
                gitea_token,
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to submit review: {}", e)))?;

        let action_past = match event {
            "APPROVE" => "approved",
            "REQUEST_CHANGES" => "requested changes on",
            "COMMENT" => "commented on",
            _ => "reviewed",
        };

        let message = format!(
            "You {} PR #{}: {}\n\nURL: {}",
            action_past, pr_number, pr.title, pr.html_url
        );

        Ok(ReviewResult { review, message })
    }

    /// Get agent's current work status
    pub async fn get_work_status(&self, agent: &Agent) -> Result<WorkStatus, AppError> {
        // Get assigned tickets
        let assigned_tickets = self.tickets.find_open_by_agent(&agent.id).await?;

        // Get projects agent is a member of
        let projects = self.projects.find_by_agent(&agent.id).await?;

        // Get open PRs for each project
        let mut open_prs = Vec::new();
        for project in projects {
            let prs = self
                .gitea
                .get_user_prs(
                    &project.gitea_org,
                    &project.gitea_repo,
                    &agent.gitea_username,
                )
                .await
                .unwrap_or_default();

            for pr in prs {
                if pr.state == "open" {
                    open_prs.push((project.clone(), pr));
                }
            }
        }

        Ok(WorkStatus {
            assigned_tickets,
            open_prs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        test_agent, test_project, test_ticket, test_ticket_assigned, InMemoryProjectRepository,
        InMemoryTicketRepository, MockGiteaClient,
    };

    fn create_service(
        ticket_repo: InMemoryTicketRepository,
        project_repo: InMemoryProjectRepository,
        gitea: MockGiteaClient,
    ) -> WorkLoopService<InMemoryTicketRepository, InMemoryProjectRepository, MockGiteaClient> {
        WorkLoopService::new(
            Arc::new(ticket_repo),
            Arc::new(project_repo),
            Arc::new(gitea),
        )
    }

    // =========================================================================
    // assign_ticket tests
    // =========================================================================

    #[tokio::test]
    async fn assign_ticket_success() {
        let agent = test_agent();
        let project = test_project();
        let ticket = test_ticket(project.id);

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new().with_ticket(ticket.clone());
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.assign_ticket(&agent, &ticket, &project).await;

        assert!(result.is_ok());
        let assign_result = result.unwrap();
        assert_eq!(assign_result.ticket.assigned_to, Some(agent.id));
        assert!(assign_result.message.contains("You are now working on"));
        assert!(assign_result.message.contains(&ticket.title));
    }

    #[tokio::test]
    async fn assign_ticket_not_project_member() {
        let agent = test_agent();
        let project = test_project();
        let ticket = test_ticket(project.id);

        // Agent is NOT a member of the project
        let project_repo = InMemoryProjectRepository::new().with_project(project.clone());
        let ticket_repo = InMemoryTicketRepository::new().with_ticket(ticket.clone());
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.assign_ticket(&agent, &ticket, &project).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::Domain(DomainError::Forbidden(_))));
    }

    #[tokio::test]
    async fn assign_ticket_not_available() {
        let agent = test_agent();
        let other_agent = crate::test_utils::test_agent_named("other-agent");
        let project = test_project();
        let ticket = test_ticket_assigned(project.id, other_agent.id);

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new().with_ticket(ticket.clone());
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.assign_ticket(&agent, &ticket, &project).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn assign_ticket_already_assigned_to_self() {
        let agent = test_agent();
        let project = test_project();
        let ticket = test_ticket_assigned(project.id, agent.id);

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new().with_ticket(ticket.clone());
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.assign_ticket(&agent, &ticket, &project).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.to_string().contains("already assigned"));
    }

    // =========================================================================
    // abandon_ticket tests
    // =========================================================================

    #[tokio::test]
    async fn abandon_ticket_success() {
        let agent = test_agent();
        let project = test_project();
        let ticket = test_ticket_assigned(project.id, agent.id);

        let project_repo = InMemoryProjectRepository::new().with_project(project.clone());
        let ticket_repo = InMemoryTicketRepository::new().with_ticket(ticket.clone());
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.abandon_ticket(&agent).await;

        assert!(result.is_ok());
        let message = result.unwrap();
        assert!(message.contains("Abandoned 1 ticket"));
        assert!(message.contains(&ticket.title));
    }

    #[tokio::test]
    async fn abandon_ticket_no_assigned_tickets() {
        let agent = test_agent();

        let project_repo = InMemoryProjectRepository::new();
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.abandon_ticket(&agent).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.to_string().contains("don't have any assigned tickets"));
    }

    // =========================================================================
    // submit_pr tests
    // =========================================================================

    #[tokio::test]
    async fn submit_pr_success() {
        let agent = test_agent();
        let project = test_project();

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new()
            .with_branch(&project.gitea_org, &project.gitea_repo, "fix-bug")
            .with_pr_creation(&project.gitea_org, &project.gitea_repo);

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .submit_pr(&agent, &project, "fix-bug", Some("Fix the bug"), None, None)
            .await;

        assert!(result.is_ok());
        let submit_result = result.unwrap();
        assert!(submit_result.message.contains("PR created"));
    }

    #[tokio::test]
    async fn submit_pr_not_project_member() {
        let agent = test_agent();
        let project = test_project();

        // Agent is NOT a member
        let project_repo = InMemoryProjectRepository::new().with_project(project.clone());
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .submit_pr(&agent, &project, "fix-bug", None, None, None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::Domain(DomainError::Forbidden(_))));
    }

    #[tokio::test]
    async fn submit_pr_branch_not_found() {
        let agent = test_agent();
        let project = test_project();

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new();
        // Branch does not exist in the mock
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .submit_pr(&agent, &project, "nonexistent-branch", None, None, None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.to_string().contains("not found"));
    }

    // =========================================================================
    // review_pr tests
    // =========================================================================

    #[tokio::test]
    async fn review_pr_approve_success() {
        let agent = test_agent();
        let project = test_project();

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new().with_pr(&project.gitea_org, &project.gitea_repo, 42);

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .review_pr(&agent, &project, 42, "approve", Some("LGTM!"), None)
            .await;

        assert!(result.is_ok());
        let review_result = result.unwrap();
        assert!(review_result.message.contains("approved"));
    }

    #[tokio::test]
    async fn review_pr_request_changes_success() {
        let agent = test_agent();
        let project = test_project();

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new().with_pr(&project.gitea_org, &project.gitea_repo, 42);

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .review_pr(
                &agent,
                &project,
                42,
                "request-changes",
                Some("Please fix X"),
                None,
            )
            .await;

        assert!(result.is_ok());
        let review_result = result.unwrap();
        assert!(review_result.message.contains("requested changes"));
    }

    #[tokio::test]
    async fn review_pr_not_project_member() {
        let agent = test_agent();
        let project = test_project();

        // Agent is NOT a member
        let project_repo = InMemoryProjectRepository::new().with_project(project.clone());
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .review_pr(&agent, &project, 42, "approve", None, None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::Domain(DomainError::Forbidden(_))));
    }

    #[tokio::test]
    async fn review_pr_not_found() {
        let agent = test_agent();
        let project = test_project();

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new();
        // No PRs in the mock
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .review_pr(&agent, &project, 999, "approve", None, None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn review_pr_invalid_action() {
        let agent = test_agent();
        let project = test_project();

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new().with_pr(&project.gitea_org, &project.gitea_repo, 42);

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service
            .review_pr(&agent, &project, 42, "invalid-action", None, None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(err.to_string().contains("Invalid review action"));
    }

    // =========================================================================
    // get_work_status tests
    // =========================================================================

    #[tokio::test]
    async fn get_work_status_with_tickets_and_prs() {
        let agent = test_agent();
        let project = test_project();
        let ticket = test_ticket_assigned(project.id, agent.id);

        let project_repo =
            InMemoryProjectRepository::new().with_project_and_member(project.clone(), agent.id);
        let ticket_repo = InMemoryTicketRepository::new().with_ticket(ticket.clone());
        let gitea = MockGiteaClient::new().with_user_prs(
            &project.gitea_org,
            &project.gitea_repo,
            &agent.gitea_username,
        );

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.get_work_status(&agent).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.assigned_tickets.len(), 1);
        assert_eq!(status.assigned_tickets[0].id, ticket.id);
        // PRs depend on the mock setup
    }

    #[tokio::test]
    async fn get_work_status_empty() {
        let agent = test_agent();

        let project_repo = InMemoryProjectRepository::new();
        let ticket_repo = InMemoryTicketRepository::new();
        let gitea = MockGiteaClient::new();

        let service = create_service(ticket_repo, project_repo, gitea);
        let result = service.get_work_status(&agent).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.assigned_tickets.is_empty());
        assert!(status.open_prs.is_empty());
    }
}
