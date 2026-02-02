//! Mock implementations of port traits
//!
//! These are in-memory implementations that can be configured for testing.
//! They store data in memory and allow tests to verify behavior.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::domain::entities::{
    Agent, AgentId, AgentReview, AgentReviewId, BuildStatus, ClaimAgent, CodeContribution,
    CodeContributionId, ContributionStatus, EloEvent, EloEventId, Issue, IssueComment, IssueId,
    IssueState, Label, MemberRole, NewAgent, NewAgentReview, NewCodeContribution, NewEloEvent,
    NewIssue, NewProject, NewTicket, Project, ProjectId, ProjectMember, ProjectStatus, Ticket,
    TicketId, TicketPriority, TicketStatus, Tier,
};
use crate::domain::ports::{
    AgentRepository, AgentReviewRepository, AgentStats, AnalyticsClient, AnalyticsEvent,
    CodeContributionRepository, DifficultyBreakdown, EloEventRepository, GiteaBranch, GiteaClient,
    GiteaCombinedStatus, GiteaComment, GiteaCommit, GiteaIssue, GiteaIssueComment, GiteaLabel,
    GiteaOrg, GiteaPRBranch, GiteaPRReview, GiteaPullRequest, GiteaReaction, GiteaRepo, GiteaUser,
    IssueRepository, LeaderboardEntry, ProjectRepository, ProjectStats, TicketRepository,
    TimeRange,
};
use crate::error::{AnalyticsError, DomainError, GiteaError};

// ============================================================================
// In-Memory Agent Repository
// ============================================================================

#[derive(Default)]
pub struct InMemoryAgentRepository {
    agents: Arc<RwLock<HashMap<AgentId, Agent>>>,
    by_api_key: Arc<RwLock<HashMap<String, AgentId>>>,
    by_name: Arc<RwLock<HashMap<String, AgentId>>>,
    by_claim_code: Arc<RwLock<HashMap<String, AgentId>>>,
    by_github_id: Arc<RwLock<HashMap<i64, AgentId>>>,
    tokens: Arc<RwLock<HashMap<AgentId, Vec<u8>>>>,
}

impl InMemoryAgentRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populate with an agent for testing
    pub fn with_agent(self, agent: Agent) -> Self {
        {
            let mut agents = self.agents.write().unwrap();
            let mut by_api_key = self.by_api_key.write().unwrap();
            let mut by_name = self.by_name.write().unwrap();
            let mut by_claim_code = self.by_claim_code.write().unwrap();
            let mut by_github_id = self.by_github_id.write().unwrap();

            by_api_key.insert(agent.api_key_hash.clone(), agent.id);
            by_name.insert(agent.name.clone(), agent.id);
            if let Some(ref code) = agent.claim_code {
                by_claim_code.insert(code.clone(), agent.id);
            }
            if let Some(github_id) = agent.github_id {
                by_github_id.insert(github_id, agent.id);
            }
            agents.insert(agent.id, agent);
        }
        self
    }
}

#[async_trait]
impl AgentRepository for InMemoryAgentRepository {
    async fn find_by_id(&self, id: &AgentId) -> Result<Option<Agent>, DomainError> {
        let agents = self.agents.read().unwrap();
        Ok(agents.get(id).cloned())
    }

    async fn find_by_api_key_hash(&self, hash: &str) -> Result<Option<Agent>, DomainError> {
        let by_api_key = self.by_api_key.read().unwrap();
        let agents = self.agents.read().unwrap();

        if let Some(id) = by_api_key.get(hash) {
            Ok(agents.get(id).cloned())
        } else {
            Ok(None)
        }
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Agent>, DomainError> {
        let by_name = self.by_name.read().unwrap();
        let agents = self.agents.read().unwrap();

        if let Some(id) = by_name.get(name) {
            Ok(agents.get(id).cloned())
        } else {
            Ok(None)
        }
    }

    async fn find_by_gitea_username(&self, username: &str) -> Result<Option<Agent>, DomainError> {
        let agents = self.agents.read().unwrap();
        Ok(agents
            .values()
            .find(|a| a.gitea_username == username)
            .cloned())
    }

    async fn create(&self, new_agent: &NewAgent) -> Result<Agent, DomainError> {
        let agent = Agent {
            id: AgentId(uuid::Uuid::new_v4()),
            name: new_agent.name.clone(),
            api_key_hash: new_agent.api_key_hash.clone(),
            gitea_username: new_agent.gitea_username.clone(),
            elo: 1000,
            tier: Tier::Bronze,
            created_at: Utc::now(),
            last_seen_at: None,
            claim_code: Some(new_agent.claim_code.clone()),
            claimed_at: None,
            github_id: None,
            github_username: None,
            github_avatar_url: None,
        };

        let mut agents = self.agents.write().unwrap();
        let mut by_api_key = self.by_api_key.write().unwrap();
        let mut by_name = self.by_name.write().unwrap();
        let mut by_claim_code = self.by_claim_code.write().unwrap();
        let mut tokens = self.tokens.write().unwrap();

        by_api_key.insert(agent.api_key_hash.clone(), agent.id);
        by_name.insert(agent.name.clone(), agent.id);
        by_claim_code.insert(new_agent.claim_code.clone(), agent.id);
        tokens.insert(agent.id, new_agent.gitea_token_encrypted.clone());
        agents.insert(agent.id, agent.clone());

        Ok(agent)
    }

    async fn update_last_seen(&self, id: &AgentId) -> Result<(), DomainError> {
        let mut agents = self.agents.write().unwrap();
        if let Some(agent) = agents.get_mut(id) {
            agent.last_seen_at = Some(Utc::now());
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Agent {} not found", id)))
        }
    }

    async fn update_elo(&self, id: &AgentId, elo: i32) -> Result<(), DomainError> {
        let mut agents = self.agents.write().unwrap();
        if let Some(agent) = agents.get_mut(id) {
            agent.elo = elo;
            agent.tier = Tier::from_elo(elo);
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Agent {} not found", id)))
        }
    }

    async fn get_gitea_token_encrypted(
        &self,
        id: &AgentId,
    ) -> Result<Option<Vec<u8>>, DomainError> {
        let tokens = self.tokens.read().unwrap();
        Ok(tokens.get(id).cloned())
    }

    async fn find_top_by_elo(&self, limit: i64) -> Result<Vec<Agent>, DomainError> {
        let agents = self.agents.read().unwrap();
        let mut sorted: Vec<_> = agents.values().cloned().collect();
        sorted.sort_by(|a, b| b.elo.cmp(&a.elo));
        Ok(sorted.into_iter().take(limit as usize).collect())
    }

    async fn find_by_claim_code(&self, code: &str) -> Result<Option<Agent>, DomainError> {
        let by_claim_code = self.by_claim_code.read().unwrap();
        let agents = self.agents.read().unwrap();

        if let Some(id) = by_claim_code.get(code) {
            Ok(agents.get(id).cloned())
        } else {
            Ok(None)
        }
    }

    async fn find_by_github_id(&self, github_id: i64) -> Result<Option<Agent>, DomainError> {
        let by_github_id = self.by_github_id.read().unwrap();
        let agents = self.agents.read().unwrap();

        if let Some(id) = by_github_id.get(&github_id) {
            Ok(agents.get(id).cloned())
        } else {
            Ok(None)
        }
    }

    async fn claim(&self, id: &AgentId, claim: &ClaimAgent) -> Result<(), DomainError> {
        let mut agents = self.agents.write().unwrap();
        let mut by_claim_code = self.by_claim_code.write().unwrap();
        let mut by_github_id = self.by_github_id.write().unwrap();

        if let Some(agent) = agents.get_mut(id) {
            // Remove old claim code from index
            if let Some(code) = &agent.claim_code {
                by_claim_code.remove(code);
            }

            // Update agent with claim info
            agent.claimed_at = Some(Utc::now());
            agent.github_id = Some(claim.github_id);
            agent.github_username = Some(claim.github_username.clone());
            agent.github_avatar_url = claim.github_avatar_url.clone();
            agent.claim_code = None; // Clear claim code after claiming

            // Add to github_id index
            by_github_id.insert(claim.github_id, *id);

            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Agent {} not found", id)))
        }
    }
}

// ============================================================================
// In-Memory Issue Repository
// ============================================================================

/// In-memory implementation of IssueRepository for testing
/// Note: In production, GiteaIssueRepository is used (issues live in Gitea)
#[derive(Default)]
pub struct InMemoryIssueRepository {
    issues: Arc<RwLock<HashMap<IssueId, Issue>>>,
    comments: Arc<RwLock<HashMap<IssueId, Vec<IssueComment>>>>,
    next_number: Arc<RwLock<i64>>,
    next_comment_id: Arc<RwLock<i64>>,
    available_labels: Arc<RwLock<Vec<Label>>>,
}

impl InMemoryIssueRepository {
    pub fn new() -> Self {
        Self {
            issues: Arc::new(RwLock::new(HashMap::new())),
            comments: Arc::new(RwLock::new(HashMap::new())),
            next_number: Arc::new(RwLock::new(1)),
            next_comment_id: Arc::new(RwLock::new(1)),
            available_labels: Arc::new(RwLock::new(vec![
                Label {
                    name: "bug".to_string(),
                    color: "ff0000".to_string(),
                    description: Some("Something isn't working".to_string()),
                },
                Label {
                    name: "enhancement".to_string(),
                    color: "00ff00".to_string(),
                    description: Some("New feature or request".to_string()),
                },
            ])),
        }
    }

    pub fn with_issue(self, issue: Issue) -> Self {
        {
            let mut issues = self.issues.write().unwrap();
            issues.insert(issue.id, issue);
        }
        self
    }
}

#[async_trait]
impl IssueRepository for InMemoryIssueRepository {
    async fn list(
        &self,
        project_id: &ProjectId,
        state: Option<&str>,
    ) -> Result<Vec<Issue>, DomainError> {
        let issues = self.issues.read().unwrap();
        let result: Vec<Issue> = issues
            .values()
            .filter(|i| i.id.project_id == *project_id)
            .filter(|i| match state {
                Some("open") => i.state == IssueState::Open,
                Some("closed") => i.state == IssueState::Closed,
                _ => true,
            })
            .cloned()
            .collect();
        Ok(result)
    }

    async fn get(&self, id: &IssueId) -> Result<Option<Issue>, DomainError> {
        let issues = self.issues.read().unwrap();
        Ok(issues.get(id).cloned())
    }

    async fn create(
        &self,
        project_id: &ProjectId,
        issue: &NewIssue,
        _agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let number = {
            let mut n = self.next_number.write().unwrap();
            let current = *n;
            *n += 1;
            current
        };

        let new_issue = Issue {
            id: IssueId::new(*project_id, number),
            title: issue.title.clone(),
            body: Some(issue.body.clone()),
            state: IssueState::Open,
            url: format!("https://gitea.test/org/repo/issues/{}", number),
            labels: vec![],
            assignees: vec![],
        };

        let mut issues = self.issues.write().unwrap();
        issues.insert(new_issue.id, new_issue.clone());
        Ok(new_issue)
    }

    async fn update(
        &self,
        id: &IssueId,
        title: Option<&str>,
        body: Option<&str>,
        _agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let mut issues = self.issues.write().unwrap();
        let issue = issues
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;

        if let Some(t) = title {
            issue.title = t.to_string();
        }
        if let Some(b) = body {
            issue.body = Some(b.to_string());
        }
        Ok(issue.clone())
    }

    async fn close(&self, id: &IssueId, _agent_token: Option<&str>) -> Result<Issue, DomainError> {
        let mut issues = self.issues.write().unwrap();
        let issue = issues
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;
        issue.state = IssueState::Closed;
        Ok(issue.clone())
    }

    async fn reopen(&self, id: &IssueId, _agent_token: Option<&str>) -> Result<Issue, DomainError> {
        let mut issues = self.issues.write().unwrap();
        let issue = issues
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;
        issue.state = IssueState::Open;
        Ok(issue.clone())
    }

    async fn list_comments(&self, id: &IssueId) -> Result<Vec<IssueComment>, DomainError> {
        let comments = self.comments.read().unwrap();
        Ok(comments.get(id).cloned().unwrap_or_default())
    }

    async fn add_comment(
        &self,
        id: &IssueId,
        body: &str,
        _agent_token: Option<&str>,
    ) -> Result<IssueComment, DomainError> {
        let comment_id = {
            let mut n = self.next_comment_id.write().unwrap();
            let current = *n;
            *n += 1;
            current
        };

        let comment = IssueComment {
            id: comment_id,
            body: body.to_string(),
            author: "test-agent".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let mut comments = self.comments.write().unwrap();
        comments.entry(*id).or_default().push(comment.clone());
        Ok(comment)
    }

    async fn edit_comment(
        &self,
        id: &IssueId,
        comment_id: i64,
        body: &str,
        _agent_token: Option<&str>,
    ) -> Result<IssueComment, DomainError> {
        let mut comments = self.comments.write().unwrap();
        let issue_comments = comments
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound("No comments".to_string()))?;

        let comment = issue_comments
            .iter_mut()
            .find(|c| c.id == comment_id)
            .ok_or_else(|| DomainError::NotFound(format!("Comment {} not found", comment_id)))?;

        comment.body = body.to_string();
        comment.updated_at = "2026-01-02T00:00:00Z".to_string();
        Ok(comment.clone())
    }

    async fn delete_comment(
        &self,
        id: &IssueId,
        comment_id: i64,
        _agent_token: Option<&str>,
    ) -> Result<(), DomainError> {
        let mut comments = self.comments.write().unwrap();
        if let Some(issue_comments) = comments.get_mut(id) {
            issue_comments.retain(|c| c.id != comment_id);
        }
        Ok(())
    }

    async fn list_labels(&self, id: &IssueId) -> Result<Vec<Label>, DomainError> {
        let issues = self.issues.read().unwrap();
        let issue = issues
            .get(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;
        Ok(issue.labels.clone())
    }

    async fn add_labels(
        &self,
        id: &IssueId,
        labels: Vec<String>,
        _agent_token: Option<&str>,
    ) -> Result<Vec<Label>, DomainError> {
        let mut issues = self.issues.write().unwrap();
        let issue = issues
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;

        for label_name in labels {
            if !issue.labels.iter().any(|l| l.name == label_name) {
                issue.labels.push(Label {
                    name: label_name,
                    color: "cccccc".to_string(),
                    description: None,
                });
            }
        }
        Ok(issue.labels.clone())
    }

    async fn remove_label(
        &self,
        id: &IssueId,
        label: &str,
        _agent_token: Option<&str>,
    ) -> Result<(), DomainError> {
        let mut issues = self.issues.write().unwrap();
        let issue = issues
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;
        issue.labels.retain(|l| l.name != label);
        Ok(())
    }

    async fn assign(
        &self,
        id: &IssueId,
        assignees: Vec<String>,
        _agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let mut issues = self.issues.write().unwrap();
        let issue = issues
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;

        for assignee in assignees {
            if !issue.assignees.contains(&assignee) {
                issue.assignees.push(assignee);
            }
        }
        Ok(issue.clone())
    }

    async fn unassign(
        &self,
        id: &IssueId,
        assignee: &str,
        _agent_token: Option<&str>,
    ) -> Result<Issue, DomainError> {
        let mut issues = self.issues.write().unwrap();
        let issue = issues
            .get_mut(id)
            .ok_or_else(|| DomainError::NotFound(format!("Issue #{} not found", id.number)))?;
        issue.assignees.retain(|a| a != assignee);
        Ok(issue.clone())
    }

    async fn list_available_labels(
        &self,
        _project_id: &ProjectId,
    ) -> Result<Vec<Label>, DomainError> {
        let labels = self.available_labels.read().unwrap();
        Ok(labels.clone())
    }
}

// ============================================================================
// In-Memory Project Repository
// ============================================================================

#[derive(Default)]
pub struct InMemoryProjectRepository {
    projects: Arc<RwLock<HashMap<ProjectId, Project>>>,
    members: Arc<RwLock<Vec<ProjectMember>>>,
}

impl InMemoryProjectRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_project(self, project: Project) -> Self {
        {
            let mut projects = self.projects.write().unwrap();
            projects.insert(project.id, project);
        }
        self
    }

    /// Add a project and an agent as a member (convenience for tests)
    pub fn with_project_and_member(self, project: Project, agent_id: AgentId) -> Self {
        {
            let mut projects = self.projects.write().unwrap();
            projects.insert(project.id, project.clone());
        }
        {
            let mut members = self.members.write().unwrap();
            members.push(ProjectMember {
                project_id: project.id,
                agent_id,
                role: MemberRole::Contributor,
                joined_at: Utc::now(),
            });
        }
        self
    }
}

#[async_trait]
impl ProjectRepository for InMemoryProjectRepository {
    async fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, DomainError> {
        let projects = self.projects.read().unwrap();
        Ok(projects.get(id).cloned())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Project>, DomainError> {
        let projects = self.projects.read().unwrap();
        Ok(projects.values().find(|p| p.name == name).cloned())
    }

    async fn find_active(&self, limit: i64, offset: i64) -> Result<Vec<Project>, DomainError> {
        let projects = self.projects.read().unwrap();
        Ok(projects
            .values()
            .filter(|p| p.status == ProjectStatus::Active)
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn find_all(&self, limit: i64, offset: i64) -> Result<Vec<Project>, DomainError> {
        let projects = self.projects.read().unwrap();
        Ok(projects
            .values()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn create(&self, new_project: &NewProject) -> Result<Project, DomainError> {
        let project = Project {
            id: ProjectId(uuid::Uuid::new_v4()),
            name: new_project.name.clone(),
            description: new_project.description.clone(),
            gitea_org: new_project.gitea_org.clone(),
            gitea_repo: new_project.gitea_repo.clone(),
            language: new_project.language.clone(),
            status: ProjectStatus::Active,
            contributor_count: 0,
            open_ticket_count: 0,
            build_status: BuildStatus::Unknown,
            created_by: new_project.created_by,
            created_at: Utc::now(),
        };

        let mut projects = self.projects.write().unwrap();
        projects.insert(project.id, project.clone());
        Ok(project)
    }

    async fn update_status(
        &self,
        id: &ProjectId,
        status: ProjectStatus,
    ) -> Result<(), DomainError> {
        let mut projects = self.projects.write().unwrap();
        if let Some(project) = projects.get_mut(id) {
            project.status = status;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Project {} not found", id)))
        }
    }

    async fn update_stats(
        &self,
        id: &ProjectId,
        contributor_count: i32,
        open_ticket_count: i32,
    ) -> Result<(), DomainError> {
        let mut projects = self.projects.write().unwrap();
        if let Some(project) = projects.get_mut(id) {
            project.contributor_count = contributor_count;
            project.open_ticket_count = open_ticket_count;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Project {} not found", id)))
        }
    }

    async fn adjust_ticket_count(&self, id: &ProjectId, delta: i32) -> Result<(), DomainError> {
        let mut projects = self.projects.write().unwrap();
        if let Some(project) = projects.get_mut(id) {
            project.open_ticket_count += delta;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Project {} not found", id)))
        }
    }

    async fn get_members(&self, id: &ProjectId) -> Result<Vec<ProjectMember>, DomainError> {
        let members = self.members.read().unwrap();
        Ok(members
            .iter()
            .filter(|m| m.project_id == *id)
            .cloned()
            .collect())
    }

    async fn add_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
        role: MemberRole,
    ) -> Result<ProjectMember, DomainError> {
        let member = ProjectMember {
            project_id: *project_id,
            agent_id: *agent_id,
            role,
            joined_at: Utc::now(),
        };

        let mut members = self.members.write().unwrap();
        members.push(member.clone());
        Ok(member)
    }

    async fn is_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<bool, DomainError> {
        let members = self.members.read().unwrap();
        Ok(members
            .iter()
            .any(|m| m.project_id == *project_id && m.agent_id == *agent_id))
    }

    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Project>, DomainError> {
        let members = self.members.read().unwrap();
        let projects = self.projects.read().unwrap();

        let project_ids: Vec<ProjectId> = members
            .iter()
            .filter(|m| m.agent_id == *agent_id)
            .map(|m| m.project_id)
            .collect();

        Ok(projects
            .values()
            .filter(|p| project_ids.contains(&p.id))
            .cloned()
            .collect())
    }

    async fn get_member_role(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<Option<MemberRole>, DomainError> {
        let members = self.members.read().unwrap();
        Ok(members
            .iter()
            .find(|m| m.project_id == *project_id && m.agent_id == *agent_id)
            .map(|m| m.role))
    }

    async fn update_member_role(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
        role: MemberRole,
    ) -> Result<(), DomainError> {
        let mut members = self.members.write().unwrap();
        if let Some(member) = members
            .iter_mut()
            .find(|m| m.project_id == *project_id && m.agent_id == *agent_id)
        {
            member.role = role;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!(
                "Member not found in project {}",
                project_id
            )))
        }
    }

    async fn remove_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<(), DomainError> {
        let mut members = self.members.write().unwrap();
        let initial_len = members.len();
        members.retain(|m| !(m.project_id == *project_id && m.agent_id == *agent_id));
        if members.len() == initial_len {
            Err(DomainError::NotFound(format!(
                "Member not found in project {}",
                project_id
            )))
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// Mock Gitea Client
// ============================================================================

/// Key for identifying a repo (org/repo pair)
type RepoKey = (String, String);
/// Key for identifying a branch (org/repo/branch)
type BranchKey = (String, String, String);
/// Key for identifying a PR (org/repo/number)
type PrKey = (String, String, i64);
/// Key for identifying user PRs (org/repo/username)
type UserPrKey = (String, String, String);

/// A mock Gitea client that tracks calls and returns configurable responses
#[derive(Default)]
pub struct MockGiteaClient {
    pub users_created: Arc<RwLock<Vec<String>>>,
    pub should_fail: Arc<RwLock<bool>>,
    /// Branches that exist (org, repo, branch)
    branches: Arc<RwLock<HashMap<BranchKey, GiteaBranch>>>,
    /// PRs that exist (org, repo, number)
    prs: Arc<RwLock<HashMap<PrKey, GiteaPullRequest>>>,
    /// Repos where PR creation is enabled
    pr_creation_enabled: Arc<RwLock<std::collections::HashSet<RepoKey>>>,
    /// User PRs (org, repo, username) -> list of PRs
    user_prs: Arc<RwLock<HashMap<UserPrKey, Vec<GiteaPullRequest>>>>,
}

impl MockGiteaClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing() -> Self {
        Self {
            users_created: Arc::new(RwLock::new(Vec::new())),
            should_fail: Arc::new(RwLock::new(true)),
            branches: Arc::new(RwLock::new(HashMap::new())),
            prs: Arc::new(RwLock::new(HashMap::new())),
            pr_creation_enabled: Arc::new(RwLock::new(std::collections::HashSet::new())),
            user_prs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Configure a branch to exist
    pub fn with_branch(self, org: &str, repo: &str, branch: &str) -> Self {
        {
            let mut branches = self.branches.write().unwrap();
            branches.insert(
                (org.to_string(), repo.to_string(), branch.to_string()),
                GiteaBranch {
                    name: branch.to_string(),
                    commit: GiteaCommit {
                        id: "abc123".to_string(),
                        message: format!("Commit on {}", branch),
                    },
                },
            );
        }
        self
    }

    /// Configure a PR to exist
    pub fn with_pr(self, org: &str, repo: &str, number: i64) -> Self {
        {
            let mut prs = self.prs.write().unwrap();
            prs.insert(
                (org.to_string(), repo.to_string(), number),
                GiteaPullRequest {
                    id: number,
                    number,
                    title: format!("PR #{}", number),
                    body: None,
                    state: "open".to_string(),
                    html_url: format!("https://gitea.local/{}/{}/pulls/{}", org, repo, number),
                    head: GiteaPRBranch {
                        ref_name: "feature".to_string(),
                        sha: "abc123".to_string(),
                    },
                    base: GiteaPRBranch {
                        ref_name: "main".to_string(),
                        sha: "def456".to_string(),
                    },
                    merged: false,
                    user: None,
                },
            );
        }
        self
    }

    /// Enable PR creation for a repo
    pub fn with_pr_creation(self, org: &str, repo: &str) -> Self {
        {
            let mut enabled = self.pr_creation_enabled.write().unwrap();
            enabled.insert((org.to_string(), repo.to_string()));
        }
        self
    }

    /// Configure user PRs to return
    pub fn with_user_prs(self, org: &str, repo: &str, username: &str) -> Self {
        {
            let mut user_prs = self.user_prs.write().unwrap();
            user_prs.insert(
                (org.to_string(), repo.to_string(), username.to_string()),
                vec![GiteaPullRequest {
                    id: 1,
                    number: 1,
                    title: "Test PR".to_string(),
                    body: None,
                    state: "open".to_string(),
                    html_url: format!("https://gitea.local/{}/{}/pulls/1", org, repo),
                    head: GiteaPRBranch {
                        ref_name: "feature".to_string(),
                        sha: "abc123".to_string(),
                    },
                    base: GiteaPRBranch {
                        ref_name: "main".to_string(),
                        sha: "def456".to_string(),
                    },
                    merged: false,
                    user: None,
                }],
            );
        }
        self
    }
}

#[async_trait]
impl GiteaClient for MockGiteaClient {
    async fn create_user(
        &self,
        username: &str,
        email: &str,
        _password: &str,
    ) -> Result<GiteaUser, GiteaError> {
        if *self.should_fail.read().unwrap() {
            return Err(GiteaError::Api {
                status: 500,
                message: "Mock failure".to_string(),
            });
        }

        self.users_created
            .write()
            .unwrap()
            .push(username.to_string());

        Ok(GiteaUser {
            id: 1,
            login: username.to_string(),
            email: email.to_string(),
            full_name: None,
        })
    }

    async fn get_user(&self, username: &str) -> Result<GiteaUser, GiteaError> {
        if *self.should_fail.read().unwrap() {
            return Err(GiteaError::UserNotFound(username.to_string()));
        }

        Ok(GiteaUser {
            id: 1,
            login: username.to_string(),
            email: format!("{}@test.com", username),
            full_name: None,
        })
    }

    async fn create_access_token(
        &self,
        _username: &str,
        _password: &str,
        _token_name: &str,
    ) -> Result<String, GiteaError> {
        if *self.should_fail.read().unwrap() {
            return Err(GiteaError::Unauthorized);
        }
        Ok("mock-token-12345".to_string())
    }

    async fn delete_access_token(
        &self,
        _username: &str,
        _token_name: &str,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn create_org(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<GiteaOrg, GiteaError> {
        if *self.should_fail.read().unwrap() {
            return Err(GiteaError::Api {
                status: 500,
                message: "Mock failure".to_string(),
            });
        }

        Ok(GiteaOrg {
            id: 1,
            name: name.to_string(),
            full_name: None,
            description: description.map(String::from),
        })
    }

    async fn get_org(&self, name: &str) -> Result<GiteaOrg, GiteaError> {
        Ok(GiteaOrg {
            id: 1,
            name: name.to_string(),
            full_name: None,
            description: None,
        })
    }

    async fn add_org_member(&self, _org: &str, _username: &str) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn add_org_owner(&self, _org: &str, _username: &str) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn create_team(
        &self,
        _org: &str,
        _name: &str,
        _description: Option<&str>,
        _permission: &str,
    ) -> Result<i64, GiteaError> {
        Ok(1)
    }

    async fn add_maintainer(&self, _org: &str, _username: &str) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn remove_maintainer(&self, _org: &str, _username: &str) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn list_maintainers(&self, _org: &str) -> Result<Vec<String>, GiteaError> {
        Ok(vec![])
    }

    async fn list_user_orgs(&self, _username: &str) -> Result<Vec<GiteaOrg>, GiteaError> {
        Ok(vec![])
    }

    async fn is_org_owner(&self, _org: &str, _username: &str) -> Result<bool, GiteaError> {
        Ok(true)
    }

    async fn create_org_repo(
        &self,
        org: &str,
        name: &str,
        description: Option<&str>,
        private: bool,
        _auto_init: bool,
    ) -> Result<GiteaRepo, GiteaError> {
        if *self.should_fail.read().unwrap() {
            return Err(GiteaError::Api {
                status: 500,
                message: "Mock failure".to_string(),
            });
        }

        Ok(GiteaRepo {
            id: 1,
            name: name.to_string(),
            full_name: format!("{}/{}", org, name),
            description: description.map(String::from),
            clone_url: format!("https://gitea.local/{}/{}.git", org, name),
            ssh_url: format!("git@gitea.local:{}/{}.git", org, name),
            html_url: format!("https://gitea.local/{}/{}", org, name),
            default_branch: "main".to_string(),
            private,
        })
    }

    async fn create_user_repo(
        &self,
        username: &str,
        name: &str,
        description: Option<&str>,
        private: bool,
        _auto_init: bool,
        _user_token: &str,
    ) -> Result<GiteaRepo, GiteaError> {
        if *self.should_fail.read().unwrap() {
            return Err(GiteaError::Api {
                status: 500,
                message: "Mock failure".to_string(),
            });
        }

        Ok(GiteaRepo {
            id: 1,
            name: name.to_string(),
            full_name: format!("{}/{}", username, name),
            description: description.map(String::from),
            clone_url: format!("https://gitea.local/{}/{}.git", username, name),
            ssh_url: format!("git@gitea.local:{}/{}.git", username, name),
            html_url: format!("https://gitea.local/{}/{}", username, name),
            default_branch: "main".to_string(),
            private,
        })
    }

    async fn get_repo(&self, owner: &str, name: &str) -> Result<GiteaRepo, GiteaError> {
        Ok(GiteaRepo {
            id: 1,
            name: name.to_string(),
            full_name: format!("{}/{}", owner, name),
            description: None,
            clone_url: format!("https://gitea.local/{}/{}.git", owner, name),
            ssh_url: format!("git@gitea.local:{}/{}.git", owner, name),
            html_url: format!("https://gitea.local/{}/{}", owner, name),
            default_branch: "main".to_string(),
            private: false,
        })
    }

    async fn fork_repo(
        &self,
        _owner: &str,
        repo: &str,
        new_owner: &str,
    ) -> Result<GiteaRepo, GiteaError> {
        Ok(GiteaRepo {
            id: 2,
            name: repo.to_string(),
            full_name: format!("{}/{}", new_owner, repo),
            description: None,
            clone_url: format!("https://gitea.local/{}/{}.git", new_owner, repo),
            ssh_url: format!("git@gitea.local:{}/{}.git", new_owner, repo),
            html_url: format!("https://gitea.local/{}/{}", new_owner, repo),
            default_branch: "main".to_string(),
            private: false,
        })
    }

    async fn delete_repo(&self, _owner: &str, _name: &str) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn create_file(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _content: &str,
        _message: &str,
        _user_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn add_collaborator(
        &self,
        _owner: &str,
        _repo: &str,
        _username: &str,
        _permission: &str,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn get_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<GiteaBranch, GiteaError> {
        let branches = self.branches.read().unwrap();
        let key = (owner.to_string(), repo.to_string(), branch.to_string());
        if let Some(b) = branches.get(&key) {
            Ok(b.clone())
        } else {
            // If no branches configured, fail (tests need to explicitly set up branches)
            Err(GiteaError::Api {
                status: 404,
                message: format!("Branch '{}' not found", branch),
            })
        }
    }

    async fn list_branches(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> Result<Vec<GiteaBranch>, GiteaError> {
        Ok(vec![GiteaBranch {
            name: "main".to_string(),
            commit: GiteaCommit {
                id: "abc123".to_string(),
                message: "Initial commit".to_string(),
            },
        }])
    }

    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        head: &str,
        base: &str,
        _auth_token: Option<&str>,
    ) -> Result<GiteaPullRequest, GiteaError> {
        // Check if PR creation is enabled for this repo
        let enabled = self.pr_creation_enabled.read().unwrap();
        let key = (owner.to_string(), repo.to_string());
        if !enabled.contains(&key) {
            return Err(GiteaError::Api {
                status: 500,
                message: "PR creation not enabled for this repo in mock".to_string(),
            });
        }

        Ok(GiteaPullRequest {
            id: 1,
            number: 1,
            title: title.to_string(),
            body: body.map(String::from),
            state: "open".to_string(),
            html_url: format!("https://gitea.local/{}/{}/pulls/1", owner, repo),
            head: GiteaPRBranch {
                ref_name: head.to_string(),
                sha: "abc123".to_string(),
            },
            base: GiteaPRBranch {
                ref_name: base.to_string(),
                sha: "def456".to_string(),
            },
            merged: false,
            user: None,
        })
    }

    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<GiteaPullRequest, GiteaError> {
        let prs = self.prs.read().unwrap();
        let key = (owner.to_string(), repo.to_string(), number);
        if let Some(pr) = prs.get(&key) {
            Ok(pr.clone())
        } else {
            Err(GiteaError::Api {
                status: 404,
                message: format!("PR #{} not found", number),
            })
        }
    }

    async fn list_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _state: Option<&str>,
    ) -> Result<Vec<GiteaPullRequest>, GiteaError> {
        Ok(vec![])
    }

    async fn get_user_prs(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
    ) -> Result<Vec<GiteaPullRequest>, GiteaError> {
        let user_prs = self.user_prs.read().unwrap();
        let key = (owner.to_string(), repo.to_string(), username.to_string());
        Ok(user_prs.get(&key).cloned().unwrap_or_default())
    }

    async fn merge_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
        _merge_style: &str,
        _auth_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn close_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn get_pr_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
    ) -> Result<Vec<GiteaComment>, GiteaError> {
        Ok(vec![GiteaComment {
            id: 1,
            body: "Looks good!".to_string(),
            user: GiteaUser {
                id: 1,
                login: "reviewer".to_string(),
                email: "reviewer@test.com".to_string(),
                full_name: None,
            },
            created_at: "2026-01-31T12:00:00Z".to_string(),
            updated_at: "2026-01-31T12:00:00Z".to_string(),
        }])
    }

    async fn post_pr_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
        body: &str,
        _auth_token: Option<&str>,
    ) -> Result<GiteaComment, GiteaError> {
        Ok(GiteaComment {
            id: 2,
            body: body.to_string(),
            user: GiteaUser {
                id: 1,
                login: "test-agent".to_string(),
                email: "agent@test.com".to_string(),
                full_name: None,
            },
            created_at: "2026-01-31T12:00:00Z".to_string(),
            updated_at: "2026-01-31T12:00:00Z".to_string(),
        })
    }

    async fn get_pr_reviews(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
    ) -> Result<Vec<GiteaPRReview>, GiteaError> {
        Ok(vec![])
    }

    async fn submit_pr_review(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
        state: &str,
        body: Option<&str>,
        _auth_token: Option<&str>,
    ) -> Result<GiteaPRReview, GiteaError> {
        Ok(GiteaPRReview {
            id: 1,
            user: GiteaUser {
                id: 1,
                login: "reviewer".to_string(),
                email: "reviewer@test.com".to_string(),
                full_name: None,
            },
            state: state.to_string(),
            body: body.map(String::from),
            submitted_at: Some("2026-01-31T12:00:00Z".to_string()),
        })
    }

    async fn get_commit_status(
        &self,
        _owner: &str,
        _repo: &str,
        _ref_name: &str,
    ) -> Result<GiteaCombinedStatus, GiteaError> {
        Ok(GiteaCombinedStatus {
            state: "success".to_string(),
            statuses: vec![],
        })
    }

    async fn create_webhook(
        &self,
        _owner: &str,
        _repo: &str,
        _url: &str,
        _events: Vec<String>,
        _secret: Option<&str>,
    ) -> Result<i64, GiteaError> {
        Ok(1)
    }

    async fn delete_webhook(
        &self,
        _owner: &str,
        _repo: &str,
        _hook_id: i64,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn get_issue_reactions(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: i64,
    ) -> Result<Vec<GiteaReaction>, GiteaError> {
        Ok(vec![])
    }

    async fn post_issue_reaction(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: i64,
        content: &str,
    ) -> Result<GiteaReaction, GiteaError> {
        Ok(GiteaReaction {
            id: 1,
            user: GiteaUser {
                id: 1,
                login: "test-agent".to_string(),
                email: "agent@test.com".to_string(),
                full_name: None,
            },
            content: content.to_string(),
            created_at: "2026-01-31T12:00:00Z".to_string(),
        })
    }

    async fn delete_issue_reaction(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: i64,
        _reaction_id: i64,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn get_comment_reactions(
        &self,
        _owner: &str,
        _repo: &str,
        _comment_id: i64,
    ) -> Result<Vec<GiteaReaction>, GiteaError> {
        Ok(vec![])
    }

    async fn post_comment_reaction(
        &self,
        _owner: &str,
        _repo: &str,
        _comment_id: i64,
        content: &str,
    ) -> Result<GiteaReaction, GiteaError> {
        Ok(GiteaReaction {
            id: 2,
            user: GiteaUser {
                id: 1,
                login: "test-agent".to_string(),
                email: "agent@test.com".to_string(),
                full_name: None,
            },
            content: content.to_string(),
            created_at: "2026-01-31T12:00:00Z".to_string(),
        })
    }

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        _auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        Ok(GiteaIssue {
            id: 1,
            number: 1,
            title: title.to_string(),
            body: body.map(|s| s.to_string()),
            state: "open".to_string(),
            html_url: format!("https://gitea.example.com/{}/{}/issues/1", owner, repo),
            labels: vec![],
            assignee: None,
            assignees: vec![],
        })
    }

    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        _state: Option<&str>,
    ) -> Result<Vec<GiteaIssue>, GiteaError> {
        Ok(vec![GiteaIssue {
            id: 1,
            number: 1,
            title: "Test Issue".to_string(),
            body: Some("Test body".to_string()),
            state: "open".to_string(),
            html_url: format!("https://gitea.example.com/{}/{}/issues/1", owner, repo),
            labels: vec![],
            assignee: None,
            assignees: vec![],
        }])
    }

    async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<GiteaIssue, GiteaError> {
        Ok(GiteaIssue {
            id: number,
            number,
            title: format!("Issue #{}", number),
            body: Some("Issue body".to_string()),
            state: "open".to_string(),
            html_url: format!(
                "https://gitea.example.com/{}/{}/issues/{}",
                owner, repo, number
            ),
            labels: vec![],
            assignee: None,
            assignees: vec![],
        })
    }

    async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        title: Option<&str>,
        body: Option<&str>,
        state: Option<&str>,
        _auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        Ok(GiteaIssue {
            id: number,
            number,
            title: title.unwrap_or("Updated Issue").to_string(),
            body: body.map(|s| s.to_string()),
            state: state.unwrap_or("open").to_string(),
            html_url: format!(
                "https://gitea.example.com/{}/{}/issues/{}",
                owner, repo, number
            ),
            labels: vec![],
            assignee: None,
            assignees: vec![],
        })
    }

    async fn list_issue_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
    ) -> Result<Vec<GiteaIssueComment>, GiteaError> {
        Ok(vec![])
    }

    async fn create_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
        body: &str,
        _auth_token: Option<&str>,
    ) -> Result<GiteaIssueComment, GiteaError> {
        Ok(GiteaIssueComment {
            id: 1,
            body: body.to_string(),
            user: GiteaUser {
                id: 1,
                login: "test-agent".to_string(),
                email: "agent@test.com".to_string(),
                full_name: None,
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        })
    }

    async fn edit_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        comment_id: i64,
        body: &str,
        _auth_token: Option<&str>,
    ) -> Result<GiteaIssueComment, GiteaError> {
        Ok(GiteaIssueComment {
            id: comment_id,
            body: body.to_string(),
            user: GiteaUser {
                id: 1,
                login: "test-agent".to_string(),
                email: "agent@test.com".to_string(),
                full_name: None,
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
        })
    }

    async fn delete_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _comment_id: i64,
        _auth_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn list_issue_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
    ) -> Result<Vec<GiteaLabel>, GiteaError> {
        Ok(vec![])
    }

    async fn add_issue_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
        labels: Vec<String>,
        _auth_token: Option<&str>,
    ) -> Result<Vec<GiteaLabel>, GiteaError> {
        Ok(labels
            .into_iter()
            .map(|name| GiteaLabel {
                id: 1,
                name,
                color: "cccccc".to_string(),
                description: None,
            })
            .collect())
    }

    async fn remove_issue_label(
        &self,
        _owner: &str,
        _repo: &str,
        _number: i64,
        _label: &str,
        _auth_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        Ok(())
    }

    async fn add_issue_assignees(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        assignees: Vec<String>,
        _auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        Ok(GiteaIssue {
            id: number,
            number,
            title: format!("Issue #{}", number),
            body: Some("Issue body".to_string()),
            state: "open".to_string(),
            html_url: format!(
                "https://gitea.example.com/{}/{}/issues/{}",
                owner, repo, number
            ),
            labels: vec![],
            assignee: assignees.first().map(|a| GiteaUser {
                id: 1,
                login: a.clone(),
                email: format!("{}@test.com", a),
                full_name: None,
            }),
            assignees: assignees
                .into_iter()
                .map(|a| GiteaUser {
                    id: 1,
                    login: a.clone(),
                    email: format!("{}@test.com", a),
                    full_name: None,
                })
                .collect(),
        })
    }

    async fn remove_issue_assignee(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        _assignee: &str,
        _auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        Ok(GiteaIssue {
            id: number,
            number,
            title: format!("Issue #{}", number),
            body: Some("Issue body".to_string()),
            state: "open".to_string(),
            html_url: format!(
                "https://gitea.example.com/{}/{}/issues/{}",
                owner, repo, number
            ),
            labels: vec![],
            assignee: None,
            assignees: vec![],
        })
    }

    async fn list_repo_labels(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> Result<Vec<GiteaLabel>, GiteaError> {
        Ok(vec![
            GiteaLabel {
                id: 1,
                name: "bug".to_string(),
                color: "ff0000".to_string(),
                description: Some("Something isn't working".to_string()),
            },
            GiteaLabel {
                id: 2,
                name: "enhancement".to_string(),
                color: "00ff00".to_string(),
                description: Some("New feature or request".to_string()),
            },
        ])
    }
}

// ============================================================================
// Mock Analytics Client
// ============================================================================

/// A mock analytics client that tracks events
#[derive(Default)]
pub struct MockAnalyticsClient {
    pub events: Arc<RwLock<Vec<AnalyticsEvent>>>,
}

impl MockAnalyticsClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_events(&self) -> Vec<AnalyticsEvent> {
        self.events.read().unwrap().clone()
    }
}

#[async_trait]
impl AnalyticsClient for MockAnalyticsClient {
    async fn track(&self, event: AnalyticsEvent) -> Result<(), AnalyticsError> {
        self.events.write().unwrap().push(event);
        Ok(())
    }

    async fn get_agent_stats(&self, agent_id: &AgentId) -> Result<AgentStats, AnalyticsError> {
        Ok(AgentStats {
            agent_id: *agent_id,
            total_submissions: 0,
            successful_submissions: 0,
            failed_submissions: 0,
            total_claims: 0,
            abandoned_claims: 0,
            average_solve_time_secs: None,
            issues_solved_by_difficulty: DifficultyBreakdown::default(),
        })
    }

    async fn get_project_stats(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectStats, AnalyticsError> {
        Ok(ProjectStats {
            project_id: *project_id,
            total_commits: 0,
            total_pull_requests: 0,
            merged_pull_requests: 0,
            active_contributors: 0,
            lines_added: 0,
            lines_removed: 0,
        })
    }

    async fn get_leaderboard(
        &self,
        _time_range: TimeRange,
        _limit: i64,
    ) -> Result<Vec<LeaderboardEntry>, AnalyticsError> {
        Ok(vec![])
    }

    async fn get_total_issues_solved(&self) -> Result<i64, AnalyticsError> {
        Ok(0)
    }

    async fn get_active_agents_count(&self, _time_range: TimeRange) -> Result<i64, AnalyticsError> {
        Ok(0)
    }
}

// ============================================================================
// In-Memory Code Contribution Repository
// ============================================================================

#[derive(Default)]
pub struct InMemoryCodeContributionRepository {
    contributions: Arc<RwLock<HashMap<CodeContributionId, CodeContribution>>>,
}

impl InMemoryCodeContributionRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_contribution(self, contribution: CodeContribution) -> Self {
        {
            let mut contributions = self.contributions.write().unwrap();
            contributions.insert(contribution.id, contribution);
        }
        self
    }
}

#[async_trait]
impl CodeContributionRepository for InMemoryCodeContributionRepository {
    async fn find_by_id(
        &self,
        id: &CodeContributionId,
    ) -> Result<Option<CodeContribution>, DomainError> {
        let contributions = self.contributions.read().unwrap();
        Ok(contributions.get(id).cloned())
    }

    async fn find_by_commit_sha(&self, sha: &str) -> Result<Option<CodeContribution>, DomainError> {
        let contributions = self.contributions.read().unwrap();
        Ok(contributions
            .values()
            .find(|c| c.commit_sha == sha)
            .cloned())
    }

    async fn find_by_pr(
        &self,
        project_id: &ProjectId,
        pr_number: i64,
    ) -> Result<Option<CodeContribution>, DomainError> {
        let contributions = self.contributions.read().unwrap();
        Ok(contributions
            .values()
            .find(|c| c.project_id == *project_id && c.pr_number == pr_number)
            .cloned())
    }

    async fn find_by_agent(
        &self,
        agent_id: &AgentId,
    ) -> Result<Vec<CodeContribution>, DomainError> {
        let contributions = self.contributions.read().unwrap();
        Ok(contributions
            .values()
            .filter(|c| c.agent_id == *agent_id)
            .cloned()
            .collect())
    }

    async fn find_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<CodeContribution>, DomainError> {
        let contributions = self.contributions.read().unwrap();
        Ok(contributions
            .values()
            .filter(|c| c.project_id == *project_id)
            .cloned()
            .collect())
    }

    async fn find_eligible_for_longevity_bonus(
        &self,
        threshold: DateTime<Utc>,
    ) -> Result<Vec<CodeContribution>, DomainError> {
        let contributions = self.contributions.read().unwrap();
        Ok(contributions
            .values()
            .filter(|c| {
                c.status == ContributionStatus::Healthy
                    && !c.longevity_bonus_paid
                    && c.merged_at <= threshold
            })
            .cloned()
            .collect())
    }

    async fn create(
        &self,
        contribution: &NewCodeContribution,
    ) -> Result<CodeContribution, DomainError> {
        let new_contribution = CodeContribution {
            id: CodeContributionId::new(),
            agent_id: contribution.agent_id,
            project_id: contribution.project_id,
            pr_number: contribution.pr_number,
            commit_sha: contribution.commit_sha.clone(),
            status: ContributionStatus::Healthy,
            bug_count: 0,
            longevity_bonus_paid: false,
            dependent_prs_count: 0,
            merged_at: contribution.merged_at,
            reverted_at: None,
            replaced_at: None,
            created_at: Utc::now(),
        };

        let mut contributions = self.contributions.write().unwrap();
        contributions.insert(new_contribution.id, new_contribution.clone());
        Ok(new_contribution)
    }

    async fn update_status(
        &self,
        id: &CodeContributionId,
        status: ContributionStatus,
        timestamp: DateTime<Utc>,
    ) -> Result<(), DomainError> {
        let mut contributions = self.contributions.write().unwrap();
        if let Some(contribution) = contributions.get_mut(id) {
            contribution.status = status;
            match status {
                ContributionStatus::Reverted => contribution.reverted_at = Some(timestamp),
                ContributionStatus::Replaced => contribution.replaced_at = Some(timestamp),
                ContributionStatus::Healthy => {}
            }
            Ok(())
        } else {
            Err(DomainError::NotFound(format!(
                "Contribution {} not found",
                id
            )))
        }
    }

    async fn mark_longevity_bonus_paid(&self, id: &CodeContributionId) -> Result<(), DomainError> {
        let mut contributions = self.contributions.write().unwrap();
        if let Some(contribution) = contributions.get_mut(id) {
            contribution.longevity_bonus_paid = true;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!(
                "Contribution {} not found",
                id
            )))
        }
    }

    async fn increment_bug_count(&self, id: &CodeContributionId) -> Result<(), DomainError> {
        let mut contributions = self.contributions.write().unwrap();
        if let Some(contribution) = contributions.get_mut(id) {
            contribution.bug_count += 1;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!(
                "Contribution {} not found",
                id
            )))
        }
    }

    async fn increment_dependent_prs(&self, id: &CodeContributionId) -> Result<(), DomainError> {
        let mut contributions = self.contributions.write().unwrap();
        if let Some(contribution) = contributions.get_mut(id) {
            contribution.dependent_prs_count += 1;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!(
                "Contribution {} not found",
                id
            )))
        }
    }
}

// ============================================================================
// In-Memory Agent Review Repository
// ============================================================================

#[derive(Default)]
pub struct InMemoryAgentReviewRepository {
    reviews: Arc<RwLock<HashMap<AgentReviewId, AgentReview>>>,
}

impl InMemoryAgentReviewRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_review(self, review: AgentReview) -> Self {
        {
            let mut reviews = self.reviews.write().unwrap();
            reviews.insert(review.id, review);
        }
        self
    }
}

#[async_trait]
impl AgentReviewRepository for InMemoryAgentReviewRepository {
    async fn find_by_id(&self, id: &AgentReviewId) -> Result<Option<AgentReview>, DomainError> {
        let reviews = self.reviews.read().unwrap();
        Ok(reviews.get(id).cloned())
    }

    async fn find_by_pr(
        &self,
        project_id: &ProjectId,
        pr_id: i64,
    ) -> Result<Vec<AgentReview>, DomainError> {
        let reviews = self.reviews.read().unwrap();
        Ok(reviews
            .values()
            .filter(|r| r.project_id == *project_id && r.pr_id == pr_id)
            .cloned()
            .collect())
    }

    async fn find_by_reviewer(&self, agent_id: &AgentId) -> Result<Vec<AgentReview>, DomainError> {
        let reviews = self.reviews.read().unwrap();
        Ok(reviews
            .values()
            .filter(|r| r.reviewer_agent_id == *agent_id)
            .cloned()
            .collect())
    }

    async fn find_by_reviewed(&self, agent_id: &AgentId) -> Result<Vec<AgentReview>, DomainError> {
        let reviews = self.reviews.read().unwrap();
        Ok(reviews
            .values()
            .filter(|r| r.reviewed_agent_id == *agent_id)
            .cloned()
            .collect())
    }

    async fn count_by_reviewer_since(
        &self,
        agent_id: &AgentId,
        since: DateTime<Utc>,
    ) -> Result<i64, DomainError> {
        let reviews = self.reviews.read().unwrap();
        let count = reviews
            .values()
            .filter(|r| r.reviewer_agent_id == *agent_id && r.created_at >= since)
            .count();
        Ok(count as i64)
    }

    async fn exists_for_pr_and_reviewer(
        &self,
        project_id: &ProjectId,
        pr_id: i64,
        reviewer_agent_id: &AgentId,
    ) -> Result<bool, DomainError> {
        let reviews = self.reviews.read().unwrap();
        Ok(reviews.values().any(|r| {
            r.project_id == *project_id
                && r.pr_id == pr_id
                && r.reviewer_agent_id == *reviewer_agent_id
        }))
    }

    async fn create(&self, review: &NewAgentReview) -> Result<AgentReview, DomainError> {
        // Check for self-review
        if review.is_self_review() {
            return Err(DomainError::Validation(
                "Cannot review your own PR".to_string(),
            ));
        }

        let new_review = AgentReview {
            id: AgentReviewId::new(),
            pr_id: review.pr_id,
            project_id: review.project_id,
            reviewer_agent_id: review.reviewer_agent_id,
            reviewed_agent_id: review.reviewed_agent_id,
            verdict: review.verdict,
            reviewer_elo_at_time: review.reviewer_elo_at_time,
            created_at: Utc::now(),
        };

        let mut reviews = self.reviews.write().unwrap();
        reviews.insert(new_review.id, new_review.clone());
        Ok(new_review)
    }
}

// ============================================================================
// In-Memory ELO Event Repository
// ============================================================================

#[derive(Default)]
pub struct InMemoryEloEventRepository {
    events: Arc<RwLock<HashMap<EloEventId, EloEvent>>>,
}

impl InMemoryEloEventRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_event(self, event: EloEvent) -> Self {
        {
            let mut events = self.events.write().unwrap();
            events.insert(event.id, event);
        }
        self
    }

    /// Get all events for inspection in tests
    pub fn get_all_events(&self) -> Vec<EloEvent> {
        self.events.read().unwrap().values().cloned().collect()
    }
}

#[async_trait]
impl EloEventRepository for InMemoryEloEventRepository {
    async fn find_by_id(&self, id: &EloEventId) -> Result<Option<EloEvent>, DomainError> {
        let events = self.events.read().unwrap();
        Ok(events.get(id).cloned())
    }

    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<EloEvent>, DomainError> {
        let events = self.events.read().unwrap();
        Ok(events
            .values()
            .filter(|e| e.agent_id == *agent_id)
            .cloned()
            .collect())
    }

    async fn find_by_agent_paginated(
        &self,
        agent_id: &AgentId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EloEvent>, DomainError> {
        let events = self.events.read().unwrap();
        let mut agent_events: Vec<_> = events
            .values()
            .filter(|e| e.agent_id == *agent_id)
            .cloned()
            .collect();

        // Sort by created_at descending
        agent_events.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(agent_events
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    async fn find_by_reference(
        &self,
        reference_id: uuid::Uuid,
    ) -> Result<Vec<EloEvent>, DomainError> {
        let events = self.events.read().unwrap();
        Ok(events
            .values()
            .filter(|e| e.reference_id == Some(reference_id))
            .cloned()
            .collect())
    }

    async fn create(&self, event: &NewEloEvent) -> Result<EloEvent, DomainError> {
        let new_event = EloEvent {
            id: EloEventId::new(),
            agent_id: event.agent_id,
            event_type: event.event_type,
            delta: event.delta,
            old_elo: event.old_elo,
            new_elo: event.new_elo,
            reference_id: event.reference_id,
            details: event.details.clone(),
            created_at: Utc::now(),
        };

        let mut events = self.events.write().unwrap();
        events.insert(new_event.id, new_event.clone());
        Ok(new_event)
    }

    async fn sum_delta_by_agent(&self, agent_id: &AgentId) -> Result<i64, DomainError> {
        let events = self.events.read().unwrap();
        let sum: i64 = events
            .values()
            .filter(|e| e.agent_id == *agent_id)
            .map(|e| e.delta as i64)
            .sum();
        Ok(sum)
    }
}

// ============================================================================
// In-Memory Ticket Repository
// ============================================================================

#[derive(Default)]
pub struct InMemoryTicketRepository {
    tickets: Arc<RwLock<HashMap<TicketId, Ticket>>>,
}

impl InMemoryTicketRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populate with a ticket for testing
    pub fn with_ticket(self, ticket: Ticket) -> Self {
        {
            let mut tickets = self.tickets.write().unwrap();
            tickets.insert(ticket.id, ticket);
        }
        self
    }
}

#[async_trait]
impl TicketRepository for InMemoryTicketRepository {
    async fn find_by_id(&self, id: &TicketId) -> Result<Option<Ticket>, DomainError> {
        let tickets = self.tickets.read().unwrap();
        Ok(tickets.get(id).cloned())
    }

    async fn find_by_project(&self, project_id: &ProjectId) -> Result<Vec<Ticket>, DomainError> {
        let tickets = self.tickets.read().unwrap();
        Ok(tickets
            .values()
            .filter(|t| t.project_id == *project_id)
            .cloned()
            .collect())
    }

    async fn find_open_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<Ticket>, DomainError> {
        let tickets = self.tickets.read().unwrap();
        Ok(tickets
            .values()
            .filter(|t| t.project_id == *project_id && t.status == TicketStatus::Open)
            .cloned()
            .collect())
    }

    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Ticket>, DomainError> {
        let tickets = self.tickets.read().unwrap();
        Ok(tickets
            .values()
            .filter(|t| t.assigned_to == Some(*agent_id))
            .cloned()
            .collect())
    }

    async fn find_open_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Ticket>, DomainError> {
        let tickets = self.tickets.read().unwrap();
        Ok(tickets
            .values()
            .filter(|t| {
                t.assigned_to == Some(*agent_id)
                    && (t.status == TicketStatus::Open || t.status == TicketStatus::InProgress)
            })
            .cloned()
            .collect())
    }

    async fn create(&self, ticket: &NewTicket) -> Result<Ticket, DomainError> {
        let new_ticket = Ticket {
            id: TicketId::new(),
            project_id: ticket.project_id,
            title: ticket.title.clone(),
            body: ticket.body.clone(),
            gitea_issue_number: ticket.gitea_issue_number,
            gitea_issue_url: ticket.gitea_issue_url.clone(),
            status: TicketStatus::Open,
            priority: ticket.priority,
            assigned_to: None,
            created_by: ticket.created_by,
            created_at: Utc::now(),
            closed_at: None,
        };

        let mut tickets = self.tickets.write().unwrap();
        tickets.insert(new_ticket.id, new_ticket.clone());
        Ok(new_ticket)
    }

    async fn assign(&self, id: &TicketId, agent_id: &AgentId) -> Result<(), DomainError> {
        let mut tickets = self.tickets.write().unwrap();
        if let Some(ticket) = tickets.get_mut(id) {
            ticket.assigned_to = Some(*agent_id);
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Ticket {} not found", id)))
        }
    }

    async fn unassign(&self, id: &TicketId) -> Result<(), DomainError> {
        let mut tickets = self.tickets.write().unwrap();
        if let Some(ticket) = tickets.get_mut(id) {
            ticket.assigned_to = None;
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Ticket {} not found", id)))
        }
    }

    async fn update_status(&self, id: &TicketId, status: TicketStatus) -> Result<(), DomainError> {
        let mut tickets = self.tickets.write().unwrap();
        if let Some(ticket) = tickets.get_mut(id) {
            ticket.status = status;
            if status == TicketStatus::Closed {
                ticket.closed_at = Some(Utc::now());
            }
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Ticket {} not found", id)))
        }
    }

    async fn close(&self, id: &TicketId) -> Result<(), DomainError> {
        self.update_status(id, TicketStatus::Closed).await
    }

    async fn count_open_by_project(&self, project_id: &ProjectId) -> Result<i64, DomainError> {
        let tickets = self.tickets.read().unwrap();
        Ok(tickets
            .values()
            .filter(|t| t.project_id == *project_id && t.status == TicketStatus::Open)
            .count() as i64)
    }
}
