//! Gitea adapter for IssueRepository
//!
//! Issues live in Gitea - this adapter calls the Gitea API.

use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::entities::{
    Issue, IssueComment, IssueId, IssueState, Label, NewIssue, Project, ProjectId,
};
use crate::domain::ports::{GiteaClient, GiteaIssue, IssueRepository, ProjectRepository};
use crate::error::DomainError;

/// Gitea implementation of IssueRepository
pub struct GiteaIssueRepository {
    gitea: Arc<dyn GiteaClient>,
    project_repo: Arc<dyn ProjectRepository>,
}

impl GiteaIssueRepository {
    pub fn new(gitea: Arc<dyn GiteaClient>, project_repo: Arc<dyn ProjectRepository>) -> Self {
        Self {
            gitea,
            project_repo,
        }
    }

    async fn get_project(&self, project_id: &ProjectId) -> Result<Project, DomainError> {
        self.project_repo
            .find_by_id(project_id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("Project {} not found", project_id.0)))
    }

    fn convert_issue(&self, project_id: ProjectId, gi: GiteaIssue) -> Issue {
        Issue {
            id: IssueId::new(project_id, gi.number),
            title: gi.title,
            body: gi.body,
            state: gi.state.parse().unwrap_or(IssueState::Open),
            url: gi.html_url,
            labels: gi
                .labels
                .into_iter()
                .map(|l| Label {
                    name: l.name,
                    color: l.color,
                    description: l.description,
                })
                .collect(),
            assignees: gi.assignees.into_iter().map(|a| a.login).collect(),
        }
    }
}

#[async_trait]
impl IssueRepository for GiteaIssueRepository {
    async fn list(
        &self,
        project_id: &ProjectId,
        state: Option<&str>,
    ) -> Result<Vec<Issue>, DomainError> {
        let project = self.get_project(project_id).await?;

        let gitea_issues = self
            .gitea
            .list_issues(&project.gitea_org, &project.gitea_repo, state)
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(gitea_issues
            .into_iter()
            .map(|gi| self.convert_issue(*project_id, gi))
            .collect())
    }

    async fn get(&self, id: &IssueId) -> Result<Option<Issue>, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        match self
            .gitea
            .get_issue(&project.gitea_org, &project.gitea_repo, id.number)
            .await
        {
            Ok(gi) => Ok(Some(self.convert_issue(id.project_id, gi))),
            Err(crate::error::GiteaError::IssueNotFound { .. }) => Ok(None),
            Err(crate::error::GiteaError::Api { status: 404, .. }) => Ok(None),
            Err(e) => Err(DomainError::Internal(format!("Gitea error: {}", e))),
        }
    }

    async fn create(
        &self,
        project_id: &ProjectId,
        issue: &NewIssue,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let project = self.get_project(project_id).await?;

        let gi = self
            .gitea
            .create_issue(
                &project.gitea_org,
                &project.gitea_repo,
                &issue.title,
                Some(&issue.body),
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(self.convert_issue(*project_id, gi))
    }

    async fn update(
        &self,
        id: &IssueId,
        title: Option<&str>,
        body: Option<&str>,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let gi = self
            .gitea
            .update_issue(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                title,
                body,
                None, // don't change state
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(self.convert_issue(id.project_id, gi))
    }

    async fn close(&self, id: &IssueId, agent_token: Option<&str>) -> Result<Issue, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let gi = self
            .gitea
            .update_issue(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                None,
                None,
                Some("closed"),
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(self.convert_issue(id.project_id, gi))
    }

    async fn reopen(&self, id: &IssueId, agent_token: Option<&str>) -> Result<Issue, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let gi = self
            .gitea
            .update_issue(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                None,
                None,
                Some("open"),
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(self.convert_issue(id.project_id, gi))
    }

    async fn list_comments(&self, id: &IssueId) -> Result<Vec<IssueComment>, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let comments = self
            .gitea
            .list_issue_comments(&project.gitea_org, &project.gitea_repo, id.number)
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(comments
            .into_iter()
            .map(|c| IssueComment {
                id: c.id,
                body: c.body,
                author: c.user.login,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect())
    }

    async fn add_comment(
        &self,
        id: &IssueId,
        body: &str,
        agent_token: Option<&str>,
    ) -> Result<IssueComment, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let c = self
            .gitea
            .create_issue_comment(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                body,
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(IssueComment {
            id: c.id,
            body: c.body,
            author: c.user.login,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
    }

    async fn edit_comment(
        &self,
        id: &IssueId,
        comment_id: i64,
        body: &str,
        agent_token: Option<&str>,
    ) -> Result<IssueComment, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let c = self
            .gitea
            .edit_issue_comment(
                &project.gitea_org,
                &project.gitea_repo,
                comment_id,
                body,
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(IssueComment {
            id: c.id,
            body: c.body,
            author: c.user.login,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
    }

    async fn delete_comment(
        &self,
        id: &IssueId,
        comment_id: i64,
        agent_token: Option<&str>,
    ) -> Result<(), DomainError> {
        let project = self.get_project(&id.project_id).await?;

        self.gitea
            .delete_issue_comment(
                &project.gitea_org,
                &project.gitea_repo,
                comment_id,
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))
    }

    async fn list_labels(&self, id: &IssueId) -> Result<Vec<Label>, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let labels = self
            .gitea
            .list_issue_labels(&project.gitea_org, &project.gitea_repo, id.number)
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(labels
            .into_iter()
            .map(|l| Label {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect())
    }

    async fn add_labels(
        &self,
        id: &IssueId,
        labels: Vec<String>,
        agent_token: Option<&str>,
    ) -> Result<Vec<Label>, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let result = self
            .gitea
            .add_issue_labels(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                labels,
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(result
            .into_iter()
            .map(|l| Label {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect())
    }

    async fn remove_label(
        &self,
        id: &IssueId,
        label: &str,
        agent_token: Option<&str>,
    ) -> Result<(), DomainError> {
        let project = self.get_project(&id.project_id).await?;

        self.gitea
            .remove_issue_label(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                label,
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))
    }

    async fn assign(
        &self,
        id: &IssueId,
        assignees: Vec<String>,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let gi = self
            .gitea
            .add_issue_assignees(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                assignees,
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(self.convert_issue(id.project_id, gi))
    }

    async fn unassign(
        &self,
        id: &IssueId,
        assignee: &str,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let project = self.get_project(&id.project_id).await?;

        let gi = self
            .gitea
            .remove_issue_assignee(
                &project.gitea_org,
                &project.gitea_repo,
                id.number,
                assignee,
                agent_token,
            )
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(self.convert_issue(id.project_id, gi))
    }

    async fn list_available_labels(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<Label>, DomainError> {
        let project = self.get_project(project_id).await?;

        let labels = self
            .gitea
            .list_repo_labels(&project.gitea_org, &project.gitea_repo)
            .await
            .map_err(|e| DomainError::Internal(format!("Gitea error: {}", e)))?;

        Ok(labels
            .into_iter()
            .map(|l| Label {
                name: l.name,
                color: l.color,
                description: l.description,
            })
            .collect())
    }
}
