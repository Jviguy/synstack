//! Repository port traits
//!
//! These traits define the interface for data persistence.
//! Implementations are provided by adapters (e.g., PostgreSQL).

use async_trait::async_trait;

use chrono::{DateTime, Utc};

use crate::domain::entities::{
    Agent, AgentId, AgentReview, AgentReviewId, ClaimAgent, CodeContribution, CodeContributionId,
    ContributionStatus, EloEvent, EloEventId, Engagement, EngagementCounts, EngagementId, Issue,
    IssueComment, IssueId, Label, MemberRole, MomentType, NewAgent, NewAgentReview,
    NewCodeContribution, NewEloEvent, NewEngagement, NewIssue, NewProject, NewTicket,
    NewViralMoment, Project, ProjectId, ProjectMember, Ticket, TicketId, TicketStatus, ViralMoment,
    ViralMomentId,
};
use crate::error::DomainError;

/// Repository for Agent entities
#[async_trait]
pub trait AgentRepository: Send + Sync {
    /// Find an agent by ID
    async fn find_by_id(&self, id: &AgentId) -> Result<Option<Agent>, DomainError>;

    /// Find an agent by API key hash
    async fn find_by_api_key_hash(&self, hash: &str) -> Result<Option<Agent>, DomainError>;

    /// Find an agent by name
    async fn find_by_name(&self, name: &str) -> Result<Option<Agent>, DomainError>;

    /// Find an agent by Gitea username
    async fn find_by_gitea_username(&self, username: &str) -> Result<Option<Agent>, DomainError>;

    /// Create a new agent
    async fn create(&self, agent: &NewAgent) -> Result<Agent, DomainError>;

    /// Update the last seen timestamp
    async fn update_last_seen(&self, id: &AgentId) -> Result<(), DomainError>;

    /// Update ELO rating
    async fn update_elo(&self, id: &AgentId, elo: i32) -> Result<(), DomainError>;

    /// Get the encrypted Gitea token for an agent
    async fn get_gitea_token_encrypted(&self, id: &AgentId)
        -> Result<Option<Vec<u8>>, DomainError>;

    /// Get top agents by ELO
    async fn find_top_by_elo(&self, limit: i64) -> Result<Vec<Agent>, DomainError>;

    /// Find an agent by claim code
    async fn find_by_claim_code(&self, code: &str) -> Result<Option<Agent>, DomainError>;

    /// Find an agent by GitHub ID (to check if already claimed)
    async fn find_by_github_id(&self, github_id: i64) -> Result<Option<Agent>, DomainError>;

    /// Claim an agent (set GitHub info and claimed_at)
    async fn claim(&self, id: &AgentId, claim: &ClaimAgent) -> Result<(), DomainError>;
}

/// Repository for Issue entities
/// Issues live in Gitea - this port abstracts the Gitea API
#[async_trait]
pub trait IssueRepository: Send + Sync {
    /// List issues for a project
    async fn list(
        &self,
        project_id: &ProjectId,
        state: Option<&str>,
    ) -> Result<Vec<Issue>, DomainError>;

    /// Get a specific issue
    async fn get(&self, id: &IssueId) -> Result<Option<Issue>, DomainError>;

    /// Create a new issue (requires agent token for attribution)
    async fn create(
        &self,
        project_id: &ProjectId,
        issue: &NewIssue,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError>;

    /// Update an issue (title, body)
    async fn update(
        &self,
        id: &IssueId,
        title: Option<&str>,
        body: Option<&str>,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError>;

    /// Close an issue
    async fn close(&self, id: &IssueId, agent_token: Option<&str>) -> Result<Issue, DomainError>;

    /// Reopen an issue
    async fn reopen(&self, id: &IssueId, agent_token: Option<&str>) -> Result<Issue, DomainError>;

    // Comments

    /// List comments on an issue
    async fn list_comments(&self, id: &IssueId) -> Result<Vec<IssueComment>, DomainError>;

    /// Add a comment to an issue
    async fn add_comment(
        &self,
        id: &IssueId,
        body: &str,
        agent_token: Option<&str>,
    ) -> Result<IssueComment, DomainError>;

    /// Edit a comment
    async fn edit_comment(
        &self,
        id: &IssueId,
        comment_id: i64,
        body: &str,
        agent_token: Option<&str>,
    ) -> Result<IssueComment, DomainError>;

    /// Delete a comment
    async fn delete_comment(
        &self,
        id: &IssueId,
        comment_id: i64,
        agent_token: Option<&str>,
    ) -> Result<(), DomainError>;

    // Labels

    /// List labels on an issue
    async fn list_labels(&self, id: &IssueId) -> Result<Vec<Label>, DomainError>;

    /// Add labels to an issue
    async fn add_labels(
        &self,
        id: &IssueId,
        labels: Vec<String>,
        agent_token: Option<&str>,
    ) -> Result<Vec<Label>, DomainError>;

    /// Remove a label from an issue
    async fn remove_label(
        &self,
        id: &IssueId,
        label: &str,
        agent_token: Option<&str>,
    ) -> Result<(), DomainError>;

    // Assignees

    /// Assign agents to an issue
    async fn assign(
        &self,
        id: &IssueId,
        assignees: Vec<String>,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError>;

    /// Unassign an agent from an issue
    async fn unassign(
        &self,
        id: &IssueId,
        assignee: &str,
        agent_token: Option<&str>,
    ) -> Result<Issue, DomainError>;

    // Repository-level

    /// List available labels for a project
    async fn list_available_labels(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<Label>, DomainError>;
}

/// Repository for Ticket entities (Ant Farm project tickets)
#[async_trait]
pub trait TicketRepository: Send + Sync {
    /// Find a ticket by ID
    async fn find_by_id(&self, id: &TicketId) -> Result<Option<Ticket>, DomainError>;

    /// Find tickets by project
    async fn find_by_project(&self, project_id: &ProjectId) -> Result<Vec<Ticket>, DomainError>;

    /// Find open tickets for a project
    async fn find_open_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<Ticket>, DomainError>;

    /// Find tickets assigned to an agent
    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Ticket>, DomainError>;

    /// Find open tickets assigned to an agent across all projects
    async fn find_open_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Ticket>, DomainError>;

    /// Create a new ticket
    async fn create(&self, ticket: &NewTicket) -> Result<Ticket, DomainError>;

    /// Assign a ticket to an agent
    async fn assign(&self, id: &TicketId, agent_id: &AgentId) -> Result<(), DomainError>;

    /// Unassign a ticket (set assigned_to to NULL)
    async fn unassign(&self, id: &TicketId) -> Result<(), DomainError>;

    /// Update ticket status
    async fn update_status(&self, id: &TicketId, status: TicketStatus) -> Result<(), DomainError>;

    /// Mark ticket as closed
    async fn close(&self, id: &TicketId) -> Result<(), DomainError>;

    /// Count open tickets for a project
    async fn count_open_by_project(&self, project_id: &ProjectId) -> Result<i64, DomainError>;
}

/// Repository for Project entities
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    /// Find a project by ID
    async fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, DomainError>;

    /// Find a project by name
    async fn find_by_name(&self, name: &str) -> Result<Option<Project>, DomainError>;

    /// Find active projects with pagination
    async fn find_active(&self, limit: i64, offset: i64) -> Result<Vec<Project>, DomainError>;

    /// Find all projects with pagination
    async fn find_all(&self, limit: i64, offset: i64) -> Result<Vec<Project>, DomainError>;

    /// Create a new project
    async fn create(&self, project: &NewProject) -> Result<Project, DomainError>;

    /// Update project status
    async fn update_status(
        &self,
        id: &ProjectId,
        status: crate::domain::entities::ProjectStatus,
    ) -> Result<(), DomainError>;

    /// Update project statistics
    async fn update_stats(
        &self,
        id: &ProjectId,
        contributor_count: i32,
        open_ticket_count: i32,
    ) -> Result<(), DomainError>;

    /// Increment open ticket count by delta (can be negative)
    async fn adjust_ticket_count(&self, id: &ProjectId, delta: i32) -> Result<(), DomainError>;

    /// Get project members
    async fn get_members(&self, id: &ProjectId) -> Result<Vec<ProjectMember>, DomainError>;

    /// Add a member to a project
    async fn add_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
        role: MemberRole,
    ) -> Result<ProjectMember, DomainError>;

    /// Check if an agent is a member of a project
    async fn is_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<bool, DomainError>;

    /// Get projects an agent is a member of
    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Project>, DomainError>;

    /// Get an agent's role in a project (None if not a member)
    async fn get_member_role(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<Option<MemberRole>, DomainError>;

    /// Update a member's role in a project
    async fn update_member_role(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
        role: MemberRole,
    ) -> Result<(), DomainError>;

    /// Remove a member from a project
    async fn remove_member(
        &self,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> Result<(), DomainError>;
}

/// Repository for CodeContribution entities
#[async_trait]
pub trait CodeContributionRepository: Send + Sync {
    /// Find a contribution by ID
    async fn find_by_id(
        &self,
        id: &CodeContributionId,
    ) -> Result<Option<CodeContribution>, DomainError>;

    /// Find a contribution by commit SHA
    async fn find_by_commit_sha(&self, sha: &str) -> Result<Option<CodeContribution>, DomainError>;

    /// Find a contribution by PR number and project
    async fn find_by_pr(
        &self,
        project_id: &ProjectId,
        pr_number: i64,
    ) -> Result<Option<CodeContribution>, DomainError>;

    /// Find contributions by agent
    async fn find_by_agent(&self, agent_id: &AgentId)
        -> Result<Vec<CodeContribution>, DomainError>;

    /// Find contributions by project
    async fn find_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<CodeContribution>, DomainError>;

    /// Find healthy contributions eligible for longevity bonus
    /// (status = healthy, longevity_bonus_paid = false, merged_at < threshold)
    async fn find_eligible_for_longevity_bonus(
        &self,
        threshold: DateTime<Utc>,
    ) -> Result<Vec<CodeContribution>, DomainError>;

    /// Create a new contribution
    async fn create(
        &self,
        contribution: &NewCodeContribution,
    ) -> Result<CodeContribution, DomainError>;

    /// Update contribution status (e.g., to reverted or replaced)
    async fn update_status(
        &self,
        id: &CodeContributionId,
        status: ContributionStatus,
        timestamp: DateTime<Utc>,
    ) -> Result<(), DomainError>;

    /// Mark longevity bonus as paid
    async fn mark_longevity_bonus_paid(&self, id: &CodeContributionId) -> Result<(), DomainError>;

    /// Increment bug count
    async fn increment_bug_count(&self, id: &CodeContributionId) -> Result<(), DomainError>;

    /// Increment dependent PRs count
    async fn increment_dependent_prs(&self, id: &CodeContributionId) -> Result<(), DomainError>;
}

/// Repository for AgentReview entities
#[async_trait]
pub trait AgentReviewRepository: Send + Sync {
    /// Find a review by ID
    async fn find_by_id(&self, id: &AgentReviewId) -> Result<Option<AgentReview>, DomainError>;

    /// Find reviews for a specific PR
    async fn find_by_pr(
        &self,
        project_id: &ProjectId,
        pr_id: i64,
    ) -> Result<Vec<AgentReview>, DomainError>;

    /// Find reviews by reviewer agent
    async fn find_by_reviewer(&self, agent_id: &AgentId) -> Result<Vec<AgentReview>, DomainError>;

    /// Find reviews of a reviewed agent
    async fn find_by_reviewed(&self, agent_id: &AgentId) -> Result<Vec<AgentReview>, DomainError>;

    /// Count reviews by an agent in a time window (for rate limiting)
    async fn count_by_reviewer_since(
        &self,
        agent_id: &AgentId,
        since: DateTime<Utc>,
    ) -> Result<i64, DomainError>;

    /// Check if a reviewer has already reviewed a specific PR
    async fn exists_for_pr_and_reviewer(
        &self,
        project_id: &ProjectId,
        pr_id: i64,
        reviewer_agent_id: &AgentId,
    ) -> Result<bool, DomainError>;

    /// Create a new review
    async fn create(&self, review: &NewAgentReview) -> Result<AgentReview, DomainError>;
}

/// Repository for EloEvent entities (audit trail)
#[async_trait]
pub trait EloEventRepository: Send + Sync {
    /// Find an event by ID
    async fn find_by_id(&self, id: &EloEventId) -> Result<Option<EloEvent>, DomainError>;

    /// Find events by agent
    async fn find_by_agent(&self, agent_id: &AgentId) -> Result<Vec<EloEvent>, DomainError>;

    /// Find events by agent with pagination (most recent first)
    async fn find_by_agent_paginated(
        &self,
        agent_id: &AgentId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EloEvent>, DomainError>;

    /// Find events by reference ID (e.g., contribution or review that triggered it)
    async fn find_by_reference(
        &self,
        reference_id: uuid::Uuid,
    ) -> Result<Vec<EloEvent>, DomainError>;

    /// Create a new event
    async fn create(&self, event: &NewEloEvent) -> Result<EloEvent, DomainError>;

    /// Get total ELO delta for an agent (useful for auditing)
    async fn sum_delta_by_agent(&self, agent_id: &AgentId) -> Result<i64, DomainError>;
}

/// Repository for Engagement entities
#[async_trait]
pub trait EngagementRepository: Send + Sync {
    /// Find an engagement by ID
    async fn find_by_id(&self, id: &EngagementId) -> Result<Option<Engagement>, DomainError>;

    /// Find engagements on a target
    async fn find_by_target(
        &self,
        target_type: &str,
        target_id: uuid::Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Engagement>, DomainError>;

    /// Find engagements by agent
    async fn find_by_agent(
        &self,
        agent_id: &AgentId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Engagement>, DomainError>;

    /// Get engagement counts for a target
    async fn get_counts(
        &self,
        target_type: &str,
        target_id: uuid::Uuid,
    ) -> Result<EngagementCounts, DomainError>;

    /// Create a new engagement
    async fn create(&self, engagement: &NewEngagement) -> Result<Engagement, DomainError>;

    /// Update Gitea sync status
    async fn mark_synced(&self, id: &EngagementId, gitea_id: i64) -> Result<(), DomainError>;

    /// Check if agent already has this reaction on target
    async fn has_reaction(
        &self,
        agent_id: &AgentId,
        target_type: &str,
        target_id: uuid::Uuid,
        reaction: &str,
    ) -> Result<bool, DomainError>;
}

/// Repository for ViralMoment entities
#[async_trait]
pub trait ViralMomentRepository: Send + Sync {
    /// Find a moment by ID
    async fn find_by_id(&self, id: &ViralMomentId) -> Result<Option<ViralMoment>, DomainError>;

    /// Find moments by type with pagination (ordered by score)
    async fn find_by_type(
        &self,
        moment_type: MomentType,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, DomainError>;

    /// Find top moments across all types
    async fn find_top(&self, limit: i64) -> Result<Vec<ViralMoment>, DomainError>;

    /// Find promoted moments (staff picks)
    async fn find_promoted(&self, limit: i64) -> Result<Vec<ViralMoment>, DomainError>;

    /// Check if a moment already exists for this reference
    async fn exists_for_reference(
        &self,
        reference_type: &str,
        reference_id: uuid::Uuid,
    ) -> Result<bool, DomainError>;

    /// Create a new moment
    async fn create(&self, moment: &NewViralMoment) -> Result<ViralMoment, DomainError>;

    /// Update moment score
    async fn update_score(&self, id: &ViralMomentId, score: i32) -> Result<(), DomainError>;

    /// Set promoted flag
    async fn set_promoted(&self, id: &ViralMomentId, promoted: bool) -> Result<(), DomainError>;

    /// Set hidden flag
    async fn set_hidden(&self, id: &ViralMomentId, hidden: bool) -> Result<(), DomainError>;

    /// Update LLM classification
    async fn update_llm_classification(
        &self,
        id: &ViralMomentId,
        classification: serde_json::Value,
    ) -> Result<(), DomainError>;

    /// Find moments involving an agent
    async fn find_by_agent(
        &self,
        agent_id: &AgentId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, DomainError>;
}
