//! Gitea client port trait
//!
//! Defines the interface for interacting with the Gitea API.

use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::GiteaError;

/// Helper to deserialize null as default (empty vec, etc.)
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

/// Gitea user representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaUser {
    pub id: i64,
    pub login: String,
    pub email: String,
    pub full_name: Option<String>,
}

/// Gitea organization representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaOrg {
    pub id: i64,
    pub name: String,
    pub full_name: Option<String>,
    pub description: Option<String>,
}

/// Gitea repository representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub clone_url: String,
    pub ssh_url: String,
    pub html_url: String,
    pub default_branch: String,
    pub private: bool,
}

/// Gitea branch representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaBranch {
    pub name: String,
    pub commit: GiteaCommit,
}

/// Gitea commit representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaCommit {
    pub id: String,
    pub message: String,
}

/// Gitea pull request representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaPullRequest {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
    pub head: GiteaPRBranch,
    pub base: GiteaPRBranch,
    pub merged: bool,
    pub user: Option<GiteaUser>,
}

/// Branch info in a PR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaPRBranch {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
}

/// Gitea issue representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaIssue {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub labels: Vec<GiteaLabel>,
    pub assignee: Option<GiteaUser>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub assignees: Vec<GiteaUser>,
}

/// Gitea label representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaLabel {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

/// Gitea issue comment representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaIssueComment {
    pub id: i64,
    pub body: String,
    pub user: GiteaUser,
    pub created_at: String,
    pub updated_at: String,
}

/// Comment on a PR or issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaComment {
    pub id: i64,
    pub body: String,
    pub user: GiteaUser,
    pub created_at: String,
    pub updated_at: String,
}

/// PR review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaPRReview {
    pub id: i64,
    pub user: GiteaUser,
    pub state: String, // "APPROVED", "CHANGES_REQUESTED", "COMMENT", "PENDING"
    pub body: Option<String>,
    pub submitted_at: Option<String>,
}

/// Combined PR status (CI checks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaCombinedStatus {
    pub state: String, // "success", "pending", "failure", "error"
    pub statuses: Vec<GiteaStatus>,
}

/// Individual status check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaStatus {
    pub state: String,
    pub context: String,
    pub description: Option<String>,
    pub target_url: Option<String>,
}

/// Reaction on an issue or comment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaReaction {
    pub id: i64,
    pub user: GiteaUser,
    pub content: String, // "laugh", "heart", "+1", "-1", "hooray", "confused", "eyes", "rocket"
    pub created_at: String,
}

/// Gitea webhook payload for push events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GiteaPushEvent {
    pub ref_name: String,
    pub before: String,
    pub after: String,
    pub repository: GiteaRepo,
    pub pusher: GiteaUser,
}

/// Gitea webhook payload for PR events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GiteaPREvent {
    pub action: String,
    pub number: i64,
    pub pull_request: GiteaPullRequest,
    pub repository: GiteaRepo,
    pub sender: GiteaUser,
}

/// Port trait for Gitea API operations
#[async_trait]
pub trait GiteaClient: Send + Sync {
    // User management

    /// Create a new Gitea user
    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<GiteaUser, GiteaError>;

    /// Get a user by username
    async fn get_user(&self, username: &str) -> Result<GiteaUser, GiteaError>;

    /// Create an access token for a user (requires user's password for basic auth)
    async fn create_access_token(
        &self,
        username: &str,
        password: &str,
        token_name: &str,
    ) -> Result<String, GiteaError>;

    /// Delete an access token
    async fn delete_access_token(&self, username: &str, token_name: &str)
        -> Result<(), GiteaError>;

    // Organization management

    /// Create a new organization
    async fn create_org(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<GiteaOrg, GiteaError>;

    /// Get an organization by name
    async fn get_org(&self, name: &str) -> Result<GiteaOrg, GiteaError>;

    /// Add a user to an organization
    async fn add_org_member(&self, org: &str, username: &str) -> Result<(), GiteaError>;

    /// Add a user as an organization owner (adds to Owners team)
    async fn add_org_owner(&self, org: &str, username: &str) -> Result<(), GiteaError>;

    /// Create a team in an organization
    /// Returns the team ID
    async fn create_team(
        &self,
        org: &str,
        name: &str,
        description: Option<&str>,
        permission: &str, // "read", "write", "admin"
    ) -> Result<i64, GiteaError>;

    /// Add a user as a maintainer (adds to Maintainers team, creates if needed)
    async fn add_maintainer(&self, org: &str, username: &str) -> Result<(), GiteaError>;

    /// Remove a user from maintainer role
    async fn remove_maintainer(&self, org: &str, username: &str) -> Result<(), GiteaError>;

    /// List maintainers of an organization
    async fn list_maintainers(&self, org: &str) -> Result<Vec<String>, GiteaError>;

    /// List organizations owned by a user
    async fn list_user_orgs(&self, username: &str) -> Result<Vec<GiteaOrg>, GiteaError>;

    /// Check if a user is an owner of an organization
    async fn is_org_owner(&self, org: &str, username: &str) -> Result<bool, GiteaError>;

    // Repository management

    /// Create a repository in an organization
    /// Set auto_init to false to create an empty repo (agent will push first commit)
    async fn create_org_repo(
        &self,
        org: &str,
        name: &str,
        description: Option<&str>,
        private: bool,
        auto_init: bool,
    ) -> Result<GiteaRepo, GiteaError>;

    /// Create a repository in a user's personal namespace
    /// Uses the user's token for proper ownership
    async fn create_user_repo(
        &self,
        username: &str,
        name: &str,
        description: Option<&str>,
        private: bool,
        auto_init: bool,
        user_token: &str,
    ) -> Result<GiteaRepo, GiteaError>;

    /// Get a repository
    async fn get_repo(&self, owner: &str, name: &str) -> Result<GiteaRepo, GiteaError>;

    /// Fork a repository to user's account
    async fn fork_repo(
        &self,
        owner: &str,
        repo: &str,
        new_owner: &str,
    ) -> Result<GiteaRepo, GiteaError>;

    /// Delete a repository
    async fn delete_repo(&self, owner: &str, name: &str) -> Result<(), GiteaError>;

    /// Create a file in a repository
    async fn create_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &str,
        message: &str,
        user_token: Option<&str>,
    ) -> Result<(), GiteaError>;

    /// Add a collaborator to a repository
    async fn add_collaborator(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
        permission: &str,
    ) -> Result<(), GiteaError>;

    // Branch management

    /// Get a branch
    async fn get_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<GiteaBranch, GiteaError>;

    /// List branches
    async fn list_branches(&self, owner: &str, repo: &str) -> Result<Vec<GiteaBranch>, GiteaError>;

    // Pull request management

    /// Create a pull request
    /// If auth_token is provided, use it instead of admin token (for agent attribution)
    #[allow(clippy::too_many_arguments)]
    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        head: &str,
        base: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaPullRequest, GiteaError>;

    /// Get a pull request
    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<GiteaPullRequest, GiteaError>;

    /// List pull requests
    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
    ) -> Result<Vec<GiteaPullRequest>, GiteaError>;

    /// Get PRs authored by a specific user in a repo
    async fn get_user_prs(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
    ) -> Result<Vec<GiteaPullRequest>, GiteaError>;

    /// Merge a pull request
    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        merge_style: &str,
        auth_token: Option<&str>,
    ) -> Result<(), GiteaError>;

    /// Close a pull request
    async fn close_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<(), GiteaError>;

    // PR comments and reviews

    /// Get comments on a PR
    async fn get_pr_comments(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaComment>, GiteaError>;

    /// Post a comment on a PR
    /// If auth_token is provided, use it instead of admin token (for agent attribution)
    async fn post_pr_comment(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        body: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaComment, GiteaError>;

    /// Get reviews on a PR
    async fn get_pr_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaPRReview>, GiteaError>;

    /// Submit a review on a PR
    /// state should be one of: "APPROVED", "REQUEST_CHANGES", "COMMENT"
    /// If auth_token is provided, use it instead of admin token (for agent attribution)
    async fn submit_pr_review(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        state: &str,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Result<GiteaPRReview, GiteaError>;

    /// Get combined commit status (CI checks)
    async fn get_commit_status(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
    ) -> Result<GiteaCombinedStatus, GiteaError>;

    // Webhook management

    /// Create a webhook on a repository
    async fn create_webhook(
        &self,
        owner: &str,
        repo: &str,
        url: &str,
        events: Vec<String>,
        secret: Option<&str>,
    ) -> Result<i64, GiteaError>;

    /// Delete a webhook
    async fn delete_webhook(&self, owner: &str, repo: &str, hook_id: i64)
        -> Result<(), GiteaError>;

    // Reactions

    /// Get reactions on an issue or PR
    async fn get_issue_reactions(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Vec<GiteaReaction>, GiteaError>;

    /// Add a reaction to an issue or PR
    async fn post_issue_reaction(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
        content: &str,
    ) -> Result<GiteaReaction, GiteaError>;

    /// Delete a reaction from an issue or PR
    async fn delete_issue_reaction(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
        reaction_id: i64,
    ) -> Result<(), GiteaError>;

    /// Get reactions on a comment
    async fn get_comment_reactions(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
    ) -> Result<Vec<GiteaReaction>, GiteaError>;

    /// Add a reaction to a comment
    async fn post_comment_reaction(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        content: &str,
    ) -> Result<GiteaReaction, GiteaError>;

    // Issue management

    /// Create an issue in a repository
    /// If auth_token is provided, use it instead of admin token (for agent attribution)
    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError>;

    /// List issues in a repository
    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
    ) -> Result<Vec<GiteaIssue>, GiteaError>;

    /// Get a specific issue
    async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<GiteaIssue, GiteaError>;

    /// Update an issue (title, body, state)
    #[allow(clippy::too_many_arguments)]
    async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        title: Option<&str>,
        body: Option<&str>,
        state: Option<&str>,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError>;

    // Issue comments

    /// List comments on an issue
    async fn list_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaIssueComment>, GiteaError>;

    /// Create a comment on an issue
    async fn create_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        body: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssueComment, GiteaError>;

    /// Edit an issue comment
    async fn edit_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        body: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssueComment, GiteaError>;

    /// Delete an issue comment
    async fn delete_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        auth_token: Option<&str>,
    ) -> Result<(), GiteaError>;

    // Issue labels

    /// List labels on an issue
    async fn list_issue_labels(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaLabel>, GiteaError>;

    /// Add labels to an issue
    async fn add_issue_labels(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        labels: Vec<String>,
        auth_token: Option<&str>,
    ) -> Result<Vec<GiteaLabel>, GiteaError>;

    /// Remove a label from an issue
    async fn remove_issue_label(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        label: &str,
        auth_token: Option<&str>,
    ) -> Result<(), GiteaError>;

    // Issue assignees

    /// Add assignees to an issue
    async fn add_issue_assignees(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        assignees: Vec<String>,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError>;

    /// Remove an assignee from an issue
    async fn remove_issue_assignee(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        assignee: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError>;

    // Repository labels

    /// List all labels in a repository
    async fn list_repo_labels(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<GiteaLabel>, GiteaError>;
}
