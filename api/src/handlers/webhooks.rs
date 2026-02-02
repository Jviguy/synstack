//! Webhook handlers
//!
//! Handlers for Gitea webhooks.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::app::parse_revert_commit;
use crate::domain::entities::ReviewVerdict;
use crate::domain::ports::{GiteaClient, ProjectRepository};
use crate::error::AppError;
use crate::AppState;

/// Gitea webhook payload
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GiteaWebhookPayload {
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub ref_name: Option<String>,
    #[serde(rename = "ref")]
    #[serde(default)]
    pub ref_field: Option<String>,
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub repository: Option<Repository>,
    #[serde(default)]
    pub sender: Option<Sender>,
    #[serde(default)]
    pub pull_request: Option<PullRequest>,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub target_url: Option<String>,
    #[serde(default)]
    pub commits: Option<Vec<Commit>>,
    #[serde(default)]
    pub review: Option<Review>,
    #[serde(default)]
    pub issue: Option<Issue>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Commit {
    pub id: String,
    pub message: String,
    #[serde(default)]
    pub author: Option<CommitAuthor>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CommitAuthor {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Review {
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Issue {
    pub id: i64,
    pub number: i64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub owner: Owner,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Owner {
    pub login: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Sender {
    pub login: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PullRequest {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub state: String,
    #[serde(default)]
    pub merged: bool,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub head: Option<PullRequestHead>,
    #[serde(default)]
    pub user: Option<PullRequestUser>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PullRequestUser {
    pub login: String,
    #[serde(default)]
    pub id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PullRequestHead {
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(rename = "ref")]
    #[serde(default)]
    pub ref_name: Option<String>,
}

/// Verify HMAC-SHA256 signature
fn verify_signature(payload: &[u8], signature: Option<&str>, secret: &Option<String>) -> bool {
    let Some(secret) = secret else {
        // No secret configured, skip verification (development mode)
        tracing::warn!("Webhook secret not configured, skipping signature verification");
        return true;
    };

    let Some(sig_header) = signature else {
        tracing::warn!("No signature provided in webhook request");
        return false;
    };

    // Gitea sends signature as "sha256=<hex>"
    let expected_hex = sig_header.strip_prefix("sha256=").unwrap_or(sig_header);

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => {
            tracing::error!("Invalid webhook secret key");
            return false;
        }
    };

    mac.update(payload);

    let expected_bytes = match hex::decode(expected_hex) {
        Ok(bytes) => bytes,
        Err(_) => {
            tracing::warn!("Invalid signature format");
            return false;
        }
    };

    mac.verify_slice(&expected_bytes).is_ok()
}

/// POST /webhooks/gitea
///
/// Handle Gitea webhook events.
pub async fn gitea_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    // Verify signature
    let signature = headers
        .get("X-Gitea-Signature")
        .and_then(|h| h.to_str().ok());

    if !verify_signature(&body, signature, &state.config.webhook_secret) {
        tracing::warn!("Webhook signature verification failed");
        return Err(AppError::Unauthorized);
    }

    // Parse JSON payload
    let payload: GiteaWebhookPayload = serde_json::from_slice(&body).map_err(|e| {
        tracing::warn!(error = %e, "Failed to parse webhook payload");
        AppError::BadRequest(format!("Invalid JSON: {}", e))
    })?;

    // Get event type from header
    let event_type = headers
        .get("X-Gitea-Event")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    tracing::info!(
        event_type = event_type,
        repo = ?payload.repository.as_ref().map(|r| &r.full_name),
        action = ?payload.action,
        "Received Gitea webhook"
    );

    match event_type {
        "push" => handle_push_event(&state, &payload).await?,
        "pull_request" => handle_pr_event(&state, &payload).await?,
        "pull_request_review" => handle_review_event(&state, &payload).await?,
        "issues" => handle_issue_event(&state, &payload).await?,
        _ => {
            tracing::debug!("Ignoring unhandled event type: {}", event_type);
        }
    }

    Ok(StatusCode::OK)
}

async fn handle_push_event(
    state: &AppState,
    payload: &GiteaWebhookPayload,
) -> Result<(), AppError> {
    let ref_name = payload.ref_field.as_deref().or(payload.ref_name.as_deref());

    if let (Some(repo), Some(ref_name), Some(after)) =
        (&payload.repository, ref_name, &payload.after)
    {
        tracing::info!(
            repo = %repo.full_name,
            ref_name = %ref_name,
            commit = %after,
            "Push event received"
        );

        // Check commits for reverts
        if let Some(commits) = &payload.commits {
            for commit in commits {
                // Check if this is a revert commit
                if let Some(reverted_sha) = parse_revert_commit(&commit.message) {
                    tracing::info!(
                        commit_id = %commit.id,
                        reverted_sha = %reverted_sha,
                        "Revert commit detected"
                    );

                    // Process the revert
                    match state
                        .reactive_elo_service
                        .on_commit_reverted(&reverted_sha, &commit.id)
                        .await
                    {
                        Ok(Some(result)) => {
                            tracing::info!(
                                agent_id = %result.agent_id,
                                delta = result.delta,
                                "Revert ELO penalty applied"
                            );

                            // Create viral moment for Hall of Shame
                            if let Ok(Some(agent)) =
                                state.agent_service.find_by_id(&result.agent_id).await
                            {
                                let project_name = repo.name.clone();
                                let revert_reason = commit.message.lines().nth(1);
                                let commit_title =
                                    commit.message.lines().next().unwrap_or("Reverted commit");

                                match state
                                    .viral_moment_service
                                    .check_hall_of_shame_revert(
                                        &agent,
                                        0, // PR number not easily available from push event
                                        commit_title,
                                        &project_name,
                                        revert_reason,
                                    )
                                    .await
                                {
                                    Ok(Some(moment)) => {
                                        tracing::info!(
                                            moment_id = %moment.id,
                                            agent = %agent.name,
                                            "Hall of Shame moment created"
                                        );
                                    }
                                    Ok(None) => {
                                        tracing::debug!("Revert did not meet shame threshold");
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Failed to create shame moment");
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            tracing::debug!(
                                reverted_sha = %reverted_sha,
                                "No contribution found for reverted commit"
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                reverted_sha = %reverted_sha,
                                "Failed to process revert"
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_pr_event(state: &AppState, payload: &GiteaWebhookPayload) -> Result<(), AppError> {
    let (Some(repo), Some(pr), Some(action)) =
        (&payload.repository, &payload.pull_request, &payload.action)
    else {
        return Ok(());
    };

    tracing::info!(
        repo = %repo.full_name,
        pr_number = pr.number,
        action = %action,
        merged = pr.merged,
        "Pull request event received"
    );

    match action.as_str() {
        "opened" => {
            tracing::info!(
                repo = %repo.full_name,
                pr_number = pr.number,
                "PR opened"
            );

            // Check for battle (multiple agents racing on same issue)
            check_for_battle(state, &repo.owner.login, &repo.name, pr).await;
        }
        "closed" => {
            if pr.merged {
                // PR was merged - process as a contribution
                let Some(pr_user) = &pr.user else {
                    tracing::debug!("PR has no user field");
                    return Ok(());
                };

                let Some(head) = &pr.head else {
                    tracing::debug!("PR has no head field");
                    return Ok(());
                };

                let commit_sha = head.sha.as_deref().unwrap_or("");

                // Look up the agent
                let agent = state
                    .agent_service
                    .find_by_gitea_username(&pr_user.login)
                    .await
                    .ok()
                    .flatten();

                let Some(agent) = agent else {
                    tracing::debug!(username = %pr_user.login, "Agent not found for PR author");
                    return Ok(());
                };

                // Look up the project
                let project = state
                    .project_repo
                    .find_by_name(&repo.name)
                    .await
                    .ok()
                    .flatten();

                let Some(project) = project else {
                    tracing::debug!(repo = %repo.name, "Project not found");
                    return Ok(());
                };

                // Record the contribution and award ELO
                match state
                    .reactive_elo_service
                    .on_pr_merged(&agent.id, &project.id, pr.number, commit_sha)
                    .await
                {
                    Ok(result) => {
                        tracing::info!(
                            agent_id = %result.agent_id,
                            pr_number = pr.number,
                            delta = result.delta,
                            "PR merge ELO awarded"
                        );
                    }
                    Err(e) => {
                        tracing::error!(error = %e, pr_number = pr.number, "Failed to process PR merge");
                    }
                }

                // Check for upset (low-ELO agent beat higher-ELO competitors)
                check_for_upset(
                    state,
                    &repo.owner.login,
                    &repo.name,
                    &project.name,
                    pr,
                    &agent,
                )
                .await;
            } else {
                // PR was closed without merge - this is a rejection
                tracing::info!(
                    repo = %repo.full_name,
                    pr_number = pr.number,
                    "PR closed without merge (rejection)"
                );

                let Some(pr_user) = &pr.user else {
                    tracing::debug!("PR has no user field");
                    return Ok(());
                };

                // Look up the agent
                let agent = state
                    .agent_service
                    .find_by_gitea_username(&pr_user.login)
                    .await
                    .ok()
                    .flatten();

                let Some(agent) = agent else {
                    tracing::debug!(username = %pr_user.login, "Agent not found for PR author");
                    return Ok(());
                };

                // Look up the project
                let project = state
                    .project_repo
                    .find_by_name(&repo.name)
                    .await
                    .ok()
                    .flatten();

                let Some(project) = project else {
                    tracing::debug!(repo = %repo.name, "Project not found");
                    return Ok(());
                };

                // Apply ELO penalty for rejected PR
                match state
                    .reactive_elo_service
                    .on_pr_rejected(&agent.id, &project.id, pr.number)
                    .await
                {
                    Ok(result) => {
                        tracing::info!(
                            agent_id = %result.agent_id,
                            pr_number = pr.number,
                            delta = result.delta,
                            "PR rejection ELO penalty applied"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to apply PR rejection penalty");
                    }
                }

                // Create Hall of Shame moment for rejection
                // rejection_count = 1 for now (would need to track multiple rejections separately)
                match state
                    .viral_moment_service
                    .check_hall_of_shame_rejection(&agent, pr.number, &pr.title, &project.name, 1)
                    .await
                {
                    Ok(Some(moment)) => {
                        tracing::info!(
                            moment_id = %moment.id,
                            agent = %agent.name,
                            "Hall of Shame moment created for PR rejection"
                        );
                    }
                    Ok(None) => {
                        tracing::debug!("PR rejection did not meet shame threshold");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to create shame moment for rejection");
                    }
                }
            }
        }
        _ => {
            tracing::debug!(action = %action, "Ignoring unhandled PR action");
        }
    }

    Ok(())
}

async fn handle_review_event(
    state: &AppState,
    payload: &GiteaWebhookPayload,
) -> Result<(), AppError> {
    let (Some(repo), Some(pr), Some(action), Some(sender)) = (
        &payload.repository,
        &payload.pull_request,
        &payload.action,
        &payload.sender,
    ) else {
        return Ok(());
    };

    tracing::info!(
        repo = %repo.full_name,
        pr_number = pr.number,
        action = %action,
        "Pull request review event received"
    );

    // Only process submitted reviews
    if action != "submitted" {
        return Ok(());
    }

    // Get review details
    let Some(review) = &payload.review else {
        return Ok(());
    };

    let Some(review_state) = &review.state else {
        return Ok(());
    };

    // Map review state to verdict
    let verdict = match review_state.to_lowercase().as_str() {
        "approved" => ReviewVerdict::Approved,
        "changes_requested" => ReviewVerdict::ChangesRequested,
        _ => {
            tracing::debug!(review_state = %review_state, "Ignoring review state");
            return Ok(());
        }
    };

    // Get PR author
    let Some(pr_user) = &pr.user else {
        tracing::debug!("PR has no user field, skipping review processing");
        return Ok(());
    };

    // Look up agents by gitea username
    let reviewer_agent = state
        .agent_service
        .find_by_gitea_username(&sender.login)
        .await
        .ok()
        .flatten();

    let reviewed_agent = state
        .agent_service
        .find_by_gitea_username(&pr_user.login)
        .await
        .ok()
        .flatten();

    let (Some(reviewer), Some(reviewed)) = (reviewer_agent, reviewed_agent) else {
        tracing::debug!(
            reviewer = %sender.login,
            reviewed = %pr_user.login,
            "Could not find agents for review"
        );
        return Ok(());
    };

    // Look up project by repo name
    let project = state
        .project_repo
        .find_by_name(&repo.name)
        .await
        .ok()
        .flatten();
    let Some(project) = project else {
        tracing::debug!(repo = %repo.name, "Project not found for review");
        return Ok(());
    };

    // Record the peer review
    match state
        .reactive_elo_service
        .on_peer_review(pr.number, &project.id, &reviewer.id, &reviewed.id, verdict)
        .await
    {
        Ok(Some(result)) => {
            tracing::info!(
                reviewed_agent_id = %result.agent_id,
                delta = result.delta,
                "High-ELO approval bonus applied"
            );
        }
        Ok(None) => {
            tracing::debug!("Review recorded but no bonus applied");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to process peer review");
        }
    }

    // Check for drama (conflicting reviews)
    check_for_drama(
        state,
        &repo.owner.login,
        &repo.name,
        pr.number,
        &pr.title,
        &project.name,
    )
    .await;

    Ok(())
}

/// Check if a PR has conflicting reviews that create drama
async fn check_for_drama(
    state: &AppState,
    owner: &str,
    repo: &str,
    pr_number: i64,
    pr_title: &str,
    project_name: &str,
) {
    // Fetch all reviews for this PR
    let reviews = match state.gitea.get_pr_reviews(owner, repo, pr_number).await {
        Ok(reviews) => reviews,
        Err(e) => {
            tracing::debug!(error = %e, "Failed to fetch PR reviews for drama check");
            return;
        }
    };

    // Separate into approvers and rejectors
    let mut approvers = Vec::new();
    let mut rejectors = Vec::new();

    for review in &reviews {
        let state_upper = review.state.to_uppercase();
        if state_upper != "APPROVED" && state_upper != "CHANGES_REQUESTED" {
            continue; // Skip comments and pending reviews
        }

        // Look up the agent
        if let Ok(Some(agent)) = state
            .agent_service
            .find_by_gitea_username(&review.user.login)
            .await
        {
            if state_upper == "APPROVED" {
                approvers.push(agent);
            } else {
                rejectors.push(agent);
            }
        }
    }

    // Check for drama if we have both
    if !approvers.is_empty() && !rejectors.is_empty() {
        match state
            .viral_moment_service
            .check_drama(pr_number, project_name, pr_title, &approvers, &rejectors)
            .await
        {
            Ok(Some(moment)) => {
                tracing::info!(
                    moment_id = %moment.id,
                    approvers = approvers.len(),
                    rejectors = rejectors.len(),
                    "Drama moment created for conflicting reviews"
                );
            }
            Ok(None) => {
                tracing::debug!("Conflicting reviews did not meet drama threshold");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create drama moment");
            }
        }
    }
}

/// Check if multiple agents are racing to solve the same issue
async fn check_for_battle(state: &AppState, owner: &str, repo: &str, new_pr: &PullRequest) {
    // Get all open PRs for this repo
    let open_prs = match state
        .gitea
        .list_pull_requests(owner, repo, Some("open"))
        .await
    {
        Ok(prs) => prs,
        Err(e) => {
            tracing::debug!(error = %e, "Failed to fetch open PRs for battle check");
            return;
        }
    };

    // Need at least 2 PRs to have a battle
    if open_prs.len() < 2 {
        return;
    }

    // Look for PRs that might be competing (similar branch names or issue references)
    // Extract issue number from the new PR's branch name or title
    let issue_ref = extract_issue_reference(&new_pr.title).or_else(|| {
        new_pr
            .head
            .as_ref()
            .and_then(|h| extract_issue_reference(h.ref_name.as_deref().unwrap_or("")))
    });

    let Some(issue_num) = issue_ref else {
        tracing::debug!("Could not extract issue reference from PR");
        return;
    };

    // Find competing PRs (same issue reference)
    let mut racers: Vec<(crate::domain::entities::Agent, String)> = Vec::new();

    for pr in &open_prs {
        let pr_issue_ref = extract_issue_reference(&pr.title)
            .or_else(|| extract_issue_reference(&pr.head.ref_name));

        if pr_issue_ref == Some(issue_num) {
            // This PR is also targeting the same issue
            if let Some(pr_user) = &pr.user {
                if let Ok(Some(agent)) = state
                    .agent_service
                    .find_by_gitea_username(&pr_user.login)
                    .await
                {
                    let status = if pr.number == new_pr.number {
                        "just joined".to_string()
                    } else {
                        "racing".to_string()
                    };
                    racers.push((agent, status));
                }
            }
        }
    }

    // Need at least 2 racers for a battle
    if racers.len() < 2 {
        return;
    }

    // Create the battle moment
    let issue_id = uuid::Uuid::new_v4(); // Use PR number as pseudo-issue ID
    let issue_title = format!("Issue #{}", issue_num);

    match state
        .viral_moment_service
        .track_battle(issue_id, &issue_title, racers.clone())
        .await
    {
        Ok(Some(moment)) => {
            tracing::info!(
                moment_id = %moment.id,
                racers = racers.len(),
                "Battle moment created for competing PRs"
            );
        }
        Ok(None) => {
            tracing::debug!("Battle did not meet threshold");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to create battle moment");
        }
    }
}

/// Extract issue number from a string (branch name, PR title)
fn extract_issue_reference(s: &str) -> Option<i64> {
    // Common patterns: "fix-123", "issue-123", "#123", "Fixes #123", "Closes #123"
    let patterns = [
        regex::Regex::new(r"(?i)(?:fix|issue|close|closes|fixes|resolve|resolves)[s\-_]?#?(\d+)")
            .ok()?,
        regex::Regex::new(r"#(\d+)").ok()?,
        regex::Regex::new(r"(?:fix|issue|close)[-_](\d+)").ok()?,
    ];

    for pattern in &patterns {
        if let Some(caps) = pattern.captures(s) {
            if let Some(num) = caps.get(1) {
                if let Ok(n) = num.as_str().parse() {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Check if a low-ELO agent's PR won over higher-ELO competitors
async fn check_for_upset(
    state: &AppState,
    owner: &str,
    repo: &str,
    _project_name: &str,
    merged_pr: &PullRequest,
    winner: &crate::domain::entities::Agent,
) {
    // Extract issue reference from merged PR
    let issue_ref = extract_issue_reference(&merged_pr.title).or_else(|| {
        merged_pr
            .head
            .as_ref()
            .and_then(|h| extract_issue_reference(h.ref_name.as_deref().unwrap_or("")))
    });

    let Some(issue_num) = issue_ref else {
        return;
    };

    // Get recently closed PRs (potential losers)
    let closed_prs = match state
        .gitea
        .list_pull_requests(owner, repo, Some("closed"))
        .await
    {
        Ok(prs) => prs,
        Err(e) => {
            tracing::debug!(error = %e, "Failed to fetch closed PRs for upset check");
            return;
        }
    };

    // Find PRs that were competing for the same issue but lost (not merged)
    let mut losers: Vec<crate::domain::entities::Agent> = Vec::new();

    for pr in &closed_prs {
        // Skip the merged PR itself
        if pr.number == merged_pr.number {
            continue;
        }

        // Skip PRs that were merged (they weren't "losers")
        if pr.merged {
            continue;
        }

        let pr_issue_ref = extract_issue_reference(&pr.title)
            .or_else(|| extract_issue_reference(&pr.head.ref_name));

        if pr_issue_ref == Some(issue_num) {
            // This PR was also targeting the same issue but wasn't merged
            if let Some(pr_user) = &pr.user {
                if let Ok(Some(agent)) = state
                    .agent_service
                    .find_by_gitea_username(&pr_user.login)
                    .await
                {
                    // Only count as loser if they have higher ELO
                    if agent.elo > winner.elo {
                        losers.push(agent);
                    }
                }
            }
        }
    }

    if losers.is_empty() {
        return;
    }

    // Create upset moment
    let issue_id = uuid::Uuid::new_v4();
    let issue_title = merged_pr.title.clone();
    let difficulty = "medium"; // Could be determined by labels

    match state
        .viral_moment_service
        .check_upset(winner, &losers, issue_id, &issue_title, difficulty)
        .await
    {
        Ok(Some(moment)) => {
            tracing::info!(
                moment_id = %moment.id,
                winner = %winner.name,
                losers = losers.len(),
                "Upset moment created: low-ELO agent beat higher-ELO competitors"
            );
        }
        Ok(None) => {
            tracing::debug!("Upset did not meet threshold");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to create upset moment");
        }
    }
}

async fn handle_issue_event(
    state: &AppState,
    payload: &GiteaWebhookPayload,
) -> Result<(), AppError> {
    let (Some(repo), Some(issue), Some(action)) =
        (&payload.repository, &payload.issue, &payload.action)
    else {
        return Ok(());
    };

    tracing::info!(
        repo = %repo.full_name,
        issue_number = issue.number,
        action = %action,
        "Issue event received"
    );

    // Only process opened issues (potential bug reports)
    if action != "opened" {
        return Ok(());
    }

    // Check if issue title/body suggests it's a bug report
    let title_lower = issue.title.to_lowercase();
    let is_bug =
        title_lower.contains("bug") || title_lower.contains("fix") || title_lower.contains("error");

    if !is_bug {
        return Ok(());
    }

    // Look for PR references in the issue body
    let body = issue.body.as_deref().unwrap_or("");
    let references = crate::app::parse_bug_references(body);

    if references.is_empty() {
        return Ok(());
    }

    // Look up project
    let project = state
        .project_repo
        .find_by_name(&repo.name)
        .await
        .ok()
        .flatten();
    let Some(project) = project else {
        tracing::debug!(repo = %repo.name, "Project not found for issue");
        return Ok(());
    };

    let default_url = format!("{}#{}", repo.full_name, issue.number);
    let issue_url = issue.html_url.as_deref().unwrap_or(&default_url);

    // Process each PR reference
    for (pr_number, _) in references {
        match state
            .reactive_elo_service
            .on_bug_referenced(&project.id, pr_number, issue_url)
            .await
        {
            Ok(Some(result)) => {
                tracing::info!(
                    agent_id = %result.agent_id,
                    pr_number = pr_number,
                    delta = result.delta,
                    "Bug reference ELO penalty applied"
                );
            }
            Ok(None) => {
                tracing::debug!(
                    pr_number = pr_number,
                    "No contribution found for referenced PR"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, pr_number = pr_number, "Failed to process bug reference");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_push_payload() {
        let json = r#"{
            "ref": "refs/heads/main",
            "before": "abc123",
            "after": "def456",
            "repository": {
                "id": 1,
                "name": "test-repo",
                "full_name": "org/test-repo",
                "owner": { "login": "org" }
            },
            "sender": { "login": "user" }
        }"#;

        let payload: GiteaWebhookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.ref_field.as_deref(), Some("refs/heads/main"));
        assert_eq!(payload.after.as_deref(), Some("def456"));
        assert!(payload.repository.is_some());
    }

    #[test]
    fn parse_pr_payload() {
        let json = r#"{
            "action": "opened",
            "repository": {
                "id": 1,
                "name": "test-repo",
                "full_name": "org/test-repo",
                "owner": { "login": "org" },
                "html_url": "https://gitea.example.com/org/test-repo"
            },
            "sender": { "login": "user" },
            "pull_request": {
                "id": 42,
                "number": 1,
                "title": "Fix bug",
                "state": "open",
                "merged": false,
                "html_url": "https://gitea.example.com/org/test-repo/pulls/1",
                "head": {
                    "sha": "abc123",
                    "ref": "fix-bug"
                }
            }
        }"#;

        let payload: GiteaWebhookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.action.as_deref(), Some("opened"));
        let pr = payload.pull_request.unwrap();
        assert_eq!(pr.number, 1);
        assert_eq!(pr.title, "Fix bug");
        assert!(!pr.merged);
    }

    #[test]
    fn parse_pr_merged_payload() {
        let json = r#"{
            "action": "closed",
            "repository": {
                "id": 1,
                "name": "test-repo",
                "full_name": "org/test-repo",
                "owner": { "login": "org" }
            },
            "pull_request": {
                "id": 42,
                "number": 1,
                "title": "Fix bug",
                "state": "closed",
                "merged": true
            }
        }"#;

        let payload: GiteaWebhookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.action.as_deref(), Some("closed"));
        let pr = payload.pull_request.unwrap();
        assert!(pr.merged);
    }

    #[test]
    fn parse_minimal_payload() {
        let json = r#"{}"#;
        let payload: GiteaWebhookPayload = serde_json::from_str(json).unwrap();
        assert!(payload.action.is_none());
        assert!(payload.repository.is_none());
    }

    #[test]
    fn verify_signature_no_secret() {
        assert!(verify_signature(b"test", None, &None));
        assert!(verify_signature(b"test", Some("invalid"), &None));
    }

    #[test]
    fn verify_signature_missing_when_required() {
        let secret = Some("test-secret".to_string());
        assert!(!verify_signature(b"test", None, &secret));
    }
}
