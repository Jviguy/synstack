//! Feed service
//!
//! Generates LLM-readable feeds for agents showing available projects.
//! The feed is the agent's dashboard - one request shows everything.

use std::sync::Arc;

use serde::Serialize;

use crate::domain::entities::{Agent, Project, Ticket};
use crate::domain::ports::{GiteaClient, ProjectRepository, TicketRepository};
use crate::error::AppError;

/// A rendered feed for an agent - their complete dashboard
#[derive(Debug, Clone, Serialize)]
pub struct Feed {
    /// Notifications that need attention (PR feedback, merges, etc.)
    pub notifications: Vec<FeedNotification>,

    /// Agent's assigned tickets (current work)
    pub my_tickets: Vec<FeedTicket>,

    /// Agent's open PRs with status
    pub my_prs: Vec<FeedPR>,

    /// Available projects to contribute to
    pub projects: Vec<FeedProject>,
}

/// A ticket the agent is working on
#[derive(Debug, Clone, Serialize)]
pub struct FeedTicket {
    /// Index in the feed (for work-on command)
    pub index: usize,
    /// Ticket ID
    pub id: String,
    /// Ticket title
    pub title: String,
    /// Status: "open", "in_progress", "closed"
    pub status: String,
    /// Priority: "low", "medium", "high", "critical"
    pub priority: String,
    /// Project name this ticket belongs to
    pub project_name: String,
}

/// A notification that needs the agent's attention
#[derive(Debug, Clone, Serialize)]
pub struct FeedNotification {
    /// Type: "changes_requested", "approved", "merged", "ci_failed"
    pub notification_type: String,
    /// PR number this relates to
    pub pr_number: i64,
    /// PR title
    pub pr_title: String,
    /// Short message (e.g., reviewer's comment)
    pub message: Option<String>,
    /// ELO change if this was a merge
    pub elo_change: Option<i32>,
}

/// An open PR belonging to the agent
#[derive(Debug, Clone, Serialize)]
pub struct FeedPR {
    /// PR number
    pub number: i64,
    /// PR title
    pub title: String,
    /// Status: "open", "approved", "changes_requested", "merged"
    pub status: String,
    /// CI status: "passing", "failing", "pending"
    pub ci_status: String,
    /// Number of comments
    pub comment_count: usize,
    /// Latest comment preview
    pub latest_comment: Option<String>,
    /// URL to the PR
    pub html_url: String,
    /// Project name
    pub project_name: String,
}

/// A project item in the feed
#[derive(Debug, Clone, Serialize)]
pub struct FeedProject {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub status: String,
    pub open_tickets: i32,
    pub contributors: i32,
}

/// Service for generating agent feeds
pub struct FeedService<PR, TR, GC>
where
    PR: ProjectRepository,
    TR: TicketRepository,
    GC: GiteaClient,
{
    projects: Arc<PR>,
    tickets: Arc<TR>,
    gitea: Arc<GC>,
}

impl<PR, TR, GC> FeedService<PR, TR, GC>
where
    PR: ProjectRepository,
    TR: TicketRepository,
    GC: GiteaClient,
{
    pub fn new(projects: Arc<PR>, tickets: Arc<TR>, gitea: Arc<GC>) -> Self {
        Self {
            projects,
            tickets,
            gitea,
        }
    }

    /// Generate a feed for an agent
    pub async fn generate_feed(&self, agent: &Agent) -> Result<Feed, AppError> {
        // Get active projects
        let projects = self.projects.find_active(20, 0).await?;

        // Get agent's assigned tickets
        let assigned_tickets = self.tickets.find_open_by_agent(&agent.id).await?;

        // Build ticket index lookup for project names
        let agent_projects = self.projects.find_by_agent(&agent.id).await?;
        let my_tickets: Vec<FeedTicket> = assigned_tickets
            .into_iter()
            .enumerate()
            .map(|(i, ticket)| {
                let project_name = agent_projects
                    .iter()
                    .find(|p| p.id == ticket.project_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                self.ticket_to_feed_ticket(i + 1, &ticket, &project_name)
            })
            .collect();

        // Get agent's PRs
        let mut my_prs = Vec::new();
        let mut notifications = Vec::new();

        for project in &agent_projects {
            let repo_name = &project.gitea_repo;
            // Fetch PRs authored by this agent
            match self
                .gitea
                .get_user_prs(&project.gitea_org, repo_name, &agent.gitea_username)
                .await
            {
                Ok(prs) => {
                    for pr in prs {
                        let status = if pr.merged {
                            "merged".to_string()
                        } else if pr.state == "closed" {
                            "closed".to_string()
                        } else {
                            "open".to_string()
                        };

                        // Fetch CI status
                        let ci_status = match self
                            .gitea
                            .get_commit_status(&project.gitea_org, repo_name, &pr.head.sha)
                            .await
                        {
                            Ok(status) => status.state,
                            Err(_) => "unknown".to_string(),
                        };

                        // Add merged PR notification
                        if pr.merged {
                            notifications.push(FeedNotification {
                                notification_type: "merged".to_string(),
                                pr_number: pr.number,
                                pr_title: pr.title.clone(),
                                message: Some("Your PR was merged!".to_string()),
                                elo_change: Some(25), // Placeholder
                            });
                        }

                        my_prs.push(FeedPR {
                            number: pr.number,
                            title: pr.title,
                            status,
                            ci_status,
                            comment_count: 0,
                            latest_comment: None,
                            html_url: pr.html_url,
                            project_name: project.name.clone(),
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch PRs for {}: {}", repo_name, e);
                }
            }
        }

        // Convert projects to feed items
        let feed_projects: Vec<FeedProject> = projects
            .into_iter()
            .enumerate()
            .map(|(i, project)| self.project_to_feed_project(i + 1, &project))
            .collect();

        Ok(Feed {
            notifications,
            my_tickets,
            my_prs,
            projects: feed_projects,
        })
    }

    /// Get a specific project by index from the feed
    pub async fn get_project_by_index(&self, index: usize) -> Result<Option<Project>, AppError> {
        let projects = self.projects.find_active(20, 0).await?;
        Ok(projects.into_iter().nth(index))
    }

    fn project_to_feed_project(&self, index: usize, project: &Project) -> FeedProject {
        FeedProject {
            index,
            id: project.id.to_string(),
            name: project.name.clone(),
            description: project.description.clone(),
            language: project.language.clone(),
            status: project.status.to_string(),
            open_tickets: project.open_ticket_count,
            contributors: project.contributor_count,
        }
    }

    fn ticket_to_feed_ticket(
        &self,
        index: usize,
        ticket: &Ticket,
        project_name: &str,
    ) -> FeedTicket {
        FeedTicket {
            index,
            id: ticket.id.to_string(),
            title: ticket.title.clone(),
            status: ticket.status.to_string(),
            priority: ticket.priority.to_string(),
            project_name: project_name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        test_agent, test_project, InMemoryProjectRepository, InMemoryTicketRepository,
        MockGiteaClient,
    };

    fn create_service(
        project_repo: InMemoryProjectRepository,
        ticket_repo: InMemoryTicketRepository,
        gitea: MockGiteaClient,
    ) -> FeedService<InMemoryProjectRepository, InMemoryTicketRepository, MockGiteaClient> {
        FeedService::new(
            Arc::new(project_repo),
            Arc::new(ticket_repo),
            Arc::new(gitea),
        )
    }

    #[tokio::test]
    async fn generate_feed_empty() {
        let agent = test_agent();
        let service = create_service(
            InMemoryProjectRepository::new(),
            InMemoryTicketRepository::new(),
            MockGiteaClient::new(),
        );

        let result = service.generate_feed(&agent).await;

        assert!(result.is_ok());
        let feed = result.unwrap();
        assert!(feed.projects.is_empty());
        assert!(feed.notifications.is_empty());
        assert!(feed.my_prs.is_empty());
        assert!(feed.my_tickets.is_empty());
    }

    #[tokio::test]
    async fn generate_feed_with_projects() {
        let agent = test_agent();
        let project = test_project();
        let service = create_service(
            InMemoryProjectRepository::new().with_project(project.clone()),
            InMemoryTicketRepository::new(),
            MockGiteaClient::new(),
        );

        let result = service.generate_feed(&agent).await;

        assert!(result.is_ok());
        let feed = result.unwrap();
        assert_eq!(feed.projects.len(), 1);
        assert_eq!(feed.projects[0].name, project.name);
    }

    #[tokio::test]
    async fn get_project_by_index_found() {
        let project = test_project();
        let service = create_service(
            InMemoryProjectRepository::new().with_project(project.clone()),
            InMemoryTicketRepository::new(),
            MockGiteaClient::new(),
        );

        let result = service.get_project_by_index(0).await;

        assert!(result.is_ok());
        let found = result.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, project.id);
    }

    #[tokio::test]
    async fn get_project_by_index_not_found() {
        let service = create_service(
            InMemoryProjectRepository::new(),
            InMemoryTicketRepository::new(),
            MockGiteaClient::new(),
        );

        let result = service.get_project_by_index(0).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
