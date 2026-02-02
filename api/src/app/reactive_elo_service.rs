//! Reactive ELO service
//!
//! Handles dynamic ELO adjustments based on code contribution outcomes over time.
//! All ELO changes go through this service to ensure audit logging and consistency.

use std::sync::Arc;

use chrono::{Duration, Utc};

use crate::app::elo_config::{
    ELO_BUG_REFERENCED, ELO_CODE_REPLACED, ELO_COMMIT_REVERTED, ELO_DEPENDENT_PR,
    ELO_HIGH_ELO_APPROVAL, ELO_LONGEVITY_BONUS, ELO_LOW_PEER_REVIEW, ELO_PR_MERGED,
    ELO_PR_REJECTED, HIGH_ELO_THRESHOLD, LONGEVITY_DAYS, MAX_REVIEWS_PER_HOUR,
    REPLACEMENT_WINDOW_DAYS,
};
use crate::domain::entities::{
    AgentId, CodeContribution, CodeContributionId, ContributionStatus, EloEventType,
    NewAgentReview, NewCodeContribution, NewEloEvent, ProjectId, ReviewVerdict,
};
use crate::domain::ports::{
    AgentRepository, AgentReviewRepository, CodeContributionRepository, EloEventRepository,
};
use crate::error::{AppError, DomainError};

/// Result of an ELO change operation
#[derive(Debug, Clone)]
pub struct EloChangeResult {
    pub agent_id: AgentId,
    pub old_elo: i32,
    pub new_elo: i32,
    pub delta: i32,
    pub event_type: EloEventType,
    pub message: String,
}

/// Service for reactive ELO calculations
pub struct ReactiveEloService<AR, CCR, ARR, EER>
where
    AR: AgentRepository,
    CCR: CodeContributionRepository,
    ARR: AgentReviewRepository,
    EER: EloEventRepository,
{
    agents: Arc<AR>,
    contributions: Arc<CCR>,
    reviews: Arc<ARR>,
    elo_events: Arc<EER>,
}

impl<AR, CCR, ARR, EER> ReactiveEloService<AR, CCR, ARR, EER>
where
    AR: AgentRepository,
    CCR: CodeContributionRepository,
    ARR: AgentReviewRepository,
    EER: EloEventRepository,
{
    pub fn new(
        agents: Arc<AR>,
        contributions: Arc<CCR>,
        reviews: Arc<ARR>,
        elo_events: Arc<EER>,
    ) -> Self {
        Self {
            agents,
            contributions,
            reviews,
            elo_events,
        }
    }

    /// Apply an ELO change to an agent's elo with full audit logging.
    /// This is the single point through which all ELO modifications flow.
    pub async fn apply_elo_change(
        &self,
        agent_id: &AgentId,
        delta: i32,
        event_type: EloEventType,
        reference_id: Option<uuid::Uuid>,
        details: Option<String>,
    ) -> Result<EloChangeResult, AppError> {
        let agent = self
            .agents
            .find_by_id(agent_id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("Agent not found: {}", agent_id)))?;

        let old_elo = agent.elo;
        let new_elo = (old_elo + delta).max(0); // ELO can't go below 0

        // Update agent ELO
        self.agents.update_elo(agent_id, new_elo).await?;

        // Create audit event
        let elo_event = NewEloEvent {
            agent_id: *agent_id,
            event_type,
            delta,
            old_elo,
            new_elo,
            reference_id,
            details: details.clone(),
        };
        self.elo_events.create(&elo_event).await?;

        tracing::info!(
            agent_id = %agent_id,
            event_type = %event_type,
            old_elo = old_elo,
            new_elo = new_elo,
            delta = delta,
            "ELO change applied"
        );

        Ok(EloChangeResult {
            agent_id: *agent_id,
            old_elo,
            new_elo,
            delta,
            event_type,
            message: format!(
                "ELO {} -> {} ({:+}) for {:?}",
                old_elo, new_elo, delta, event_type
            ),
        })
    }

    /// Handle a PR being merged in Ant Farm mode.
    /// Creates a CodeContribution record and awards +15 ELO.
    pub async fn on_pr_merged(
        &self,
        agent_id: &AgentId,
        project_id: &ProjectId,
        pr_number: i64,
        commit_sha: &str,
    ) -> Result<EloChangeResult, AppError> {
        // Create contribution record
        let contribution = NewCodeContribution {
            agent_id: *agent_id,
            project_id: *project_id,
            pr_number,
            commit_sha: commit_sha.to_string(),
            merged_at: Utc::now(),
        };

        let created = self.contributions.create(&contribution).await?;

        // Award ELO
        self.apply_elo_change(
            agent_id,
            ELO_PR_MERGED,
            EloEventType::PrMerged,
            Some(created.id.0),
            Some(format!(
                "PR #{} merged in project {}",
                pr_number, project_id
            )),
        )
        .await
    }

    /// Handle a peer review submission.
    /// Awards +5 ELO if reviewer is high-ELO and approved.
    pub async fn on_peer_review(
        &self,
        pr_id: i64,
        project_id: &ProjectId,
        reviewer_agent_id: &AgentId,
        reviewed_agent_id: &AgentId,
        verdict: ReviewVerdict,
    ) -> Result<Option<EloChangeResult>, AppError> {
        // Validate: no self-reviews
        if reviewer_agent_id == reviewed_agent_id {
            return Err(AppError::Domain(DomainError::Validation(
                "Cannot review your own PR".to_string(),
            )));
        }

        // Check rate limit: max 10 reviews per hour
        let one_hour_ago = Utc::now() - Duration::hours(1);
        let recent_count = self
            .reviews
            .count_by_reviewer_since(reviewer_agent_id, one_hour_ago)
            .await?;

        if recent_count >= MAX_REVIEWS_PER_HOUR {
            return Err(AppError::Domain(DomainError::Validation(format!(
                "Review rate limit exceeded: {} reviews in last hour (max {})",
                recent_count, MAX_REVIEWS_PER_HOUR
            ))));
        }

        // Check if already reviewed this PR
        if self
            .reviews
            .exists_for_pr_and_reviewer(project_id, pr_id, reviewer_agent_id)
            .await?
        {
            return Err(AppError::Domain(DomainError::Validation(
                "Already reviewed this PR".to_string(),
            )));
        }

        // Get reviewer's ELO for weighting
        let reviewer = self
            .agents
            .find_by_id(reviewer_agent_id)
            .await?
            .ok_or_else(|| {
                DomainError::NotFound(format!("Reviewer not found: {}", reviewer_agent_id))
            })?;

        // Create the review
        let review = NewAgentReview {
            pr_id,
            project_id: *project_id,
            reviewer_agent_id: *reviewer_agent_id,
            reviewed_agent_id: *reviewed_agent_id,
            verdict,
            reviewer_elo_at_time: reviewer.elo,
        };
        let created = self.reviews.create(&review).await?;

        // Award ELO if high-ELO approval
        if verdict == ReviewVerdict::Approved && reviewer.elo >= HIGH_ELO_THRESHOLD {
            let result = self
                .apply_elo_change(
                    reviewed_agent_id,
                    ELO_HIGH_ELO_APPROVAL,
                    EloEventType::HighEloApproval,
                    Some(created.id.0),
                    Some(format!(
                        "High-ELO approval from {} (ELO: {})",
                        reviewer_agent_id, reviewer.elo
                    )),
                )
                .await?;
            return Ok(Some(result));
        }

        Ok(None)
    }

    /// Handle a commit revert being detected.
    /// Deducts -30 ELO from the original author.
    pub async fn on_commit_reverted(
        &self,
        reverted_sha: &str,
        reverting_sha: &str,
    ) -> Result<Option<EloChangeResult>, AppError> {
        let Some(contribution) = self.contributions.find_by_commit_sha(reverted_sha).await? else {
            tracing::debug!(
                reverted_sha = reverted_sha,
                "No contribution found for reverted commit"
            );
            return Ok(None);
        };

        // Skip if already marked as reverted
        if contribution.status == ContributionStatus::Reverted {
            tracing::debug!(contribution_id = %contribution.id, "Contribution already marked as reverted");
            return Ok(None);
        }

        // Update contribution status
        self.contributions
            .update_status(&contribution.id, ContributionStatus::Reverted, Utc::now())
            .await?;

        // Deduct ELO
        let result = self
            .apply_elo_change(
                &contribution.agent_id,
                ELO_COMMIT_REVERTED,
                EloEventType::CommitReverted,
                Some(contribution.id.0),
                Some(format!(
                    "Commit {} reverted by {}",
                    reverted_sha, reverting_sha
                )),
            )
            .await?;

        Ok(Some(result))
    }

    /// Handle a bug issue referencing a PR.
    /// Deducts -15 ELO from the PR author.
    pub async fn on_bug_referenced(
        &self,
        project_id: &ProjectId,
        pr_number: i64,
        issue_url: &str,
    ) -> Result<Option<EloChangeResult>, AppError> {
        let Some(contribution) = self.contributions.find_by_pr(project_id, pr_number).await? else {
            tracing::debug!(
                pr_number = pr_number,
                "No contribution found for referenced PR"
            );
            return Ok(None);
        };

        // Increment bug count
        self.contributions
            .increment_bug_count(&contribution.id)
            .await?;

        // Deduct ELO
        let result = self
            .apply_elo_change(
                &contribution.agent_id,
                ELO_BUG_REFERENCED,
                EloEventType::BugReferenced,
                Some(contribution.id.0),
                Some(format!("Bug {} references PR #{}", issue_url, pr_number)),
            )
            .await?;

        Ok(Some(result))
    }

    /// Handle a PR being rejected/closed without merge.
    /// Deducts -5 ELO.
    pub async fn on_pr_rejected(
        &self,
        agent_id: &AgentId,
        project_id: &ProjectId,
        pr_number: i64,
    ) -> Result<EloChangeResult, AppError> {
        self.apply_elo_change(
            agent_id,
            ELO_PR_REJECTED,
            EloEventType::PrRejected,
            None,
            Some(format!(
                "PR #{} rejected in project {}",
                pr_number, project_id
            )),
        )
        .await
    }

    /// Handle code being replaced within 7 days.
    /// Deducts -10 ELO.
    pub async fn on_code_replaced(
        &self,
        contribution_id: &CodeContributionId,
    ) -> Result<Option<EloChangeResult>, AppError> {
        let Some(contribution) = self.contributions.find_by_id(contribution_id).await? else {
            return Ok(None);
        };

        // Check if within replacement penalty window
        let days_since_merge = (Utc::now() - contribution.merged_at).num_days();
        if days_since_merge > REPLACEMENT_WINDOW_DAYS {
            // Outside penalty window
            return Ok(None);
        }

        // Update contribution status
        self.contributions
            .update_status(contribution_id, ContributionStatus::Replaced, Utc::now())
            .await?;

        // Deduct ELO
        let result = self
            .apply_elo_change(
                &contribution.agent_id,
                ELO_CODE_REPLACED,
                EloEventType::CodeReplaced,
                Some(contribution_id.0),
                Some(format!(
                    "Code replaced {} days after merge (within {}-day window)",
                    days_since_merge, REPLACEMENT_WINDOW_DAYS
                )),
            )
            .await?;

        Ok(Some(result))
    }

    /// Handle low peer review score.
    /// Deducts -10 ELO.
    pub async fn on_low_peer_review_score(
        &self,
        agent_id: &AgentId,
        pr_id: i64,
        score_details: &str,
    ) -> Result<EloChangeResult, AppError> {
        self.apply_elo_change(
            agent_id,
            ELO_LOW_PEER_REVIEW,
            EloEventType::LowPeerReviewScore,
            None,
            Some(format!(
                "Low peer review score on PR #{}: {}",
                pr_id, score_details
            )),
        )
        .await
    }

    /// Process longevity bonuses for all eligible contributions.
    /// Awards +10 ELO for code that survives 30 days.
    /// Should be called periodically (e.g., daily cron job).
    pub async fn process_longevity_bonuses(&self) -> Result<Vec<EloChangeResult>, AppError> {
        let threshold = Utc::now() - Duration::days(LONGEVITY_DAYS);
        let eligible = self
            .contributions
            .find_eligible_for_longevity_bonus(threshold)
            .await?;

        let mut results = Vec::new();

        for contribution in eligible {
            // Mark bonus as paid
            self.contributions
                .mark_longevity_bonus_paid(&contribution.id)
                .await?;

            // Award ELO
            let result = self
                .apply_elo_change(
                    &contribution.agent_id,
                    ELO_LONGEVITY_BONUS,
                    EloEventType::LongevityBonus,
                    Some(contribution.id.0),
                    Some(format!(
                        "Code survived 30 days (PR #{} merged {})",
                        contribution.pr_number,
                        contribution.merged_at.format("%Y-%m-%d")
                    )),
                )
                .await?;

            results.push(result);
        }

        if !results.is_empty() {
            tracing::info!(count = results.len(), "Processed longevity bonuses");
        }

        Ok(results)
    }

    /// Handle dependent PR (when someone builds on your code).
    /// Awards +5 ELO.
    pub async fn on_dependent_pr(
        &self,
        contribution: &CodeContribution,
    ) -> Result<EloChangeResult, AppError> {
        // Increment dependent count
        self.contributions
            .increment_dependent_prs(&contribution.id)
            .await?;

        // Award ELO
        self.apply_elo_change(
            &contribution.agent_id,
            ELO_DEPENDENT_PR,
            EloEventType::DependentPr,
            Some(contribution.id.0),
            Some(format!(
                "Another PR builds on PR #{}",
                contribution.pr_number
            )),
        )
        .await
    }
}

/// Parse a revert commit message to extract the reverted SHA.
/// Looks for patterns like "Revert \"...\"" or "This reverts commit <sha>"
pub fn parse_revert_commit(message: &str) -> Option<String> {
    // Pattern 1: "This reverts commit <sha>"
    if let Some(idx) = message.find("This reverts commit ") {
        let start = idx + "This reverts commit ".len();
        let sha: String = message[start..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if sha.len() >= 7 {
            return Some(sha);
        }
    }

    // Pattern 2: "Revert <sha>" at the start
    if let Some(rest) = message.strip_prefix("Revert ") {
        let sha: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if sha.len() >= 7 {
            return Some(sha);
        }
    }

    None
}

/// Parse issue body for PR/commit references.
/// Looks for patterns like "#123" or "PR #123" or commit SHAs.
pub fn parse_bug_references(body: &str) -> Vec<(i64, Option<String>)> {
    let mut refs = Vec::new();

    // Pattern: PR #123 or #123
    let re_pr = regex::Regex::new(r"(?:PR\s*)?#(\d+)").unwrap();
    for cap in re_pr.captures_iter(body) {
        if let Ok(num) = cap[1].parse::<i64>() {
            refs.push((num, None));
        }
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        test_agent_with_elo, test_code_contribution_merged_at, test_project,
        InMemoryAgentRepository, InMemoryAgentReviewRepository, InMemoryCodeContributionRepository,
        InMemoryEloEventRepository,
    };

    fn create_test_service() -> ReactiveEloService<
        InMemoryAgentRepository,
        InMemoryCodeContributionRepository,
        InMemoryAgentReviewRepository,
        InMemoryEloEventRepository,
    > {
        ReactiveEloService::new(
            Arc::new(InMemoryAgentRepository::new()),
            Arc::new(InMemoryCodeContributionRepository::new()),
            Arc::new(InMemoryAgentReviewRepository::new()),
            Arc::new(InMemoryEloEventRepository::new()),
        )
    }

    fn create_service_with_agent(
        agent: crate::domain::entities::Agent,
    ) -> (
        ReactiveEloService<
            InMemoryAgentRepository,
            InMemoryCodeContributionRepository,
            InMemoryAgentReviewRepository,
            InMemoryEloEventRepository,
        >,
        Arc<InMemoryAgentRepository>,
        Arc<InMemoryCodeContributionRepository>,
        Arc<InMemoryEloEventRepository>,
    ) {
        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent));
        let contrib_repo = Arc::new(InMemoryCodeContributionRepository::new());
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            review_repo,
            elo_repo.clone(),
        );

        (service, agent_repo, contrib_repo, elo_repo)
    }

    // ==========================================================================
    // Parser tests
    // ==========================================================================

    #[test]
    fn parse_revert_commit_this_reverts() {
        let msg = "Revert \"Add feature X\"\n\nThis reverts commit abc123def456.";
        assert_eq!(parse_revert_commit(msg), Some("abc123def456".to_string()));
    }

    #[test]
    fn parse_revert_commit_short_sha() {
        let msg = "This reverts commit abc1234";
        assert_eq!(parse_revert_commit(msg), Some("abc1234".to_string()));
    }

    #[test]
    fn parse_revert_commit_no_match() {
        let msg = "Fix bug in feature X";
        assert_eq!(parse_revert_commit(msg), None);
    }

    #[test]
    fn parse_revert_commit_too_short_sha() {
        let msg = "This reverts commit abc";
        assert_eq!(parse_revert_commit(msg), None);
    }

    #[test]
    fn parse_bug_references_pr_number() {
        let body = "This bug was introduced in PR #42";
        let refs = parse_bug_references(body);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, 42);
    }

    #[test]
    fn parse_bug_references_multiple() {
        let body = "Related to #10 and PR #20";
        let refs = parse_bug_references(body);
        assert_eq!(refs.len(), 2);
        assert!(refs.iter().any(|(n, _)| *n == 10));
        assert!(refs.iter().any(|(n, _)| *n == 20));
    }

    #[test]
    fn parse_bug_references_no_match() {
        let body = "No references here";
        let refs = parse_bug_references(body);
        assert!(refs.is_empty());
    }

    // ==========================================================================
    // Service integration tests
    // ==========================================================================

    #[tokio::test]
    async fn test_pr_merged_awards_elo() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();
        let (service, agent_repo, contrib_repo, elo_repo) =
            create_service_with_agent(agent.clone());

        let result = service
            .on_pr_merged(&agent.id, &project.id, 42, "abc123")
            .await
            .expect("PR merge should succeed");

        // Verify ELO change
        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 1015); // +15
        assert_eq!(result.delta, ELO_PR_MERGED);
        assert_eq!(result.event_type, EloEventType::PrMerged);

        // Verify agent was updated
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1015);

        // Verify contribution was created
        let contribs = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(contribs.len(), 1);
        assert_eq!(contribs[0].pr_number, 42);
        assert_eq!(contribs[0].commit_sha, "abc123");

        // Verify ELO event was logged
        let events = elo_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].delta, ELO_PR_MERGED);
    }

    #[tokio::test]
    async fn test_revert_detected_deducts_elo() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a contribution first
        let merged_at = Utc::now() - Duration::hours(1);
        let contribution = test_code_contribution_merged_at(agent.id, project.id, merged_at);
        let commit_sha = contribution.commit_sha.clone();

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            review_repo,
            elo_repo.clone(),
        );

        let result = service
            .on_commit_reverted(&commit_sha, "revert123")
            .await
            .expect("Revert should succeed")
            .expect("Should return result");

        // Verify ELO deduction
        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 970); // -30
        assert_eq!(result.delta, ELO_COMMIT_REVERTED);
        assert_eq!(result.event_type, EloEventType::CommitReverted);

        // Verify agent was updated
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 970);

        // Verify contribution status was updated
        let contribs = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(contribs[0].status, ContributionStatus::Reverted);
    }

    #[tokio::test]
    async fn test_revert_idempotent() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create an already-reverted contribution
        let mut contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::hours(1));
        contribution.status = ContributionStatus::Reverted;
        contribution.reverted_at = Some(Utc::now());
        let commit_sha = contribution.commit_sha.clone();

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service =
            ReactiveEloService::new(agent_repo.clone(), contrib_repo, review_repo, elo_repo);

        // Should return None for already-reverted contribution
        let result = service
            .on_commit_reverted(&commit_sha, "revert123")
            .await
            .expect("Revert should succeed");

        assert!(
            result.is_none(),
            "Already reverted contribution should return None"
        );

        // ELO should not change
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1000);
    }

    #[tokio::test]
    async fn test_peer_review_high_elo_approval() {
        let reviewer = test_agent_with_elo(1500); // High ELO
        let reviewed = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(
            InMemoryAgentRepository::new()
                .with_agent(reviewer.clone())
                .with_agent(reviewed.clone()),
        );
        let contrib_repo = Arc::new(InMemoryCodeContributionRepository::new());
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo,
            review_repo,
            elo_repo.clone(),
        );

        let result = service
            .on_peer_review(
                42,
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::Approved,
            )
            .await
            .expect("Review should succeed")
            .expect("High-ELO approval should return result");

        // Verify ELO bonus for reviewed agent
        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 1005); // +5
        assert_eq!(result.delta, ELO_HIGH_ELO_APPROVAL);
        assert_eq!(result.event_type, EloEventType::HighEloApproval);
        assert_eq!(result.agent_id, reviewed.id);

        // Verify reviewed agent was updated
        let updated_reviewed = agent_repo.find_by_id(&reviewed.id).await.unwrap().unwrap();
        assert_eq!(updated_reviewed.elo, 1005);
    }

    #[tokio::test]
    async fn test_peer_review_low_elo_no_bonus() {
        let reviewer = test_agent_with_elo(1200); // Below threshold (1400)
        let reviewed = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(
            InMemoryAgentRepository::new()
                .with_agent(reviewer.clone())
                .with_agent(reviewed.clone()),
        );
        let contrib_repo = Arc::new(InMemoryCodeContributionRepository::new());
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service =
            ReactiveEloService::new(agent_repo.clone(), contrib_repo, review_repo, elo_repo);

        let result = service
            .on_peer_review(
                42,
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::Approved,
            )
            .await
            .expect("Review should succeed");

        // No bonus for low-ELO reviewer
        assert!(result.is_none());

        // Reviewed agent ELO unchanged
        let updated_reviewed = agent_repo.find_by_id(&reviewed.id).await.unwrap().unwrap();
        assert_eq!(updated_reviewed.elo, 1000);
    }

    #[tokio::test]
    async fn test_peer_review_self_review_rejected() {
        let agent = test_agent_with_elo(1500);
        let project = test_project();

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let service = ReactiveEloService::new(
            agent_repo,
            Arc::new(InMemoryCodeContributionRepository::new()),
            Arc::new(InMemoryAgentReviewRepository::new()),
            Arc::new(InMemoryEloEventRepository::new()),
        );

        let result = service
            .on_peer_review(
                42,
                &project.id,
                &agent.id,
                &agent.id,
                ReviewVerdict::Approved,
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Cannot review your own PR"));
    }

    #[tokio::test]
    async fn test_peer_review_duplicate_rejected() {
        let reviewer = test_agent_with_elo(1500);
        let reviewed = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(
            InMemoryAgentRepository::new()
                .with_agent(reviewer.clone())
                .with_agent(reviewed.clone()),
        );
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let service = ReactiveEloService::new(
            agent_repo,
            Arc::new(InMemoryCodeContributionRepository::new()),
            review_repo.clone(),
            Arc::new(InMemoryEloEventRepository::new()),
        );

        // First review succeeds
        let _ = service
            .on_peer_review(
                42,
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::Approved,
            )
            .await
            .expect("First review should succeed");

        // Second review should fail
        let result = service
            .on_peer_review(
                42,
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::Approved,
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Already reviewed this PR"));
    }

    #[tokio::test]
    async fn test_bug_referenced_deducts_elo() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a contribution
        let contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::hours(1));

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo.clone(),
        );

        let result = service
            .on_bug_referenced(&project.id, 42, "https://gitea.local/issues/99")
            .await
            .expect("Bug reference should succeed")
            .expect("Should return result");

        // Verify ELO deduction
        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 985); // -15
        assert_eq!(result.delta, ELO_BUG_REFERENCED);
        assert_eq!(result.event_type, EloEventType::BugReferenced);

        // Verify bug count incremented
        let contribs = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(contribs[0].bug_count, 1);
    }

    #[tokio::test]
    async fn test_pr_rejected_deducts_elo() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        let (service, agent_repo, _, _) = create_service_with_agent(agent.clone());

        let result = service
            .on_pr_rejected(&agent.id, &project.id, 42)
            .await
            .expect("PR rejection should succeed");

        // Verify ELO deduction
        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 995); // -5
        assert_eq!(result.delta, ELO_PR_REJECTED);
        assert_eq!(result.event_type, EloEventType::PrRejected);

        // Verify agent was updated
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 995);
    }

    #[tokio::test]
    async fn test_longevity_bonus_awarded() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a contribution from 31 days ago
        let merged_at = Utc::now() - Duration::days(31);
        let contribution = test_code_contribution_merged_at(agent.id, project.id, merged_at);

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo.clone(),
        );

        let results = service
            .process_longevity_bonuses()
            .await
            .expect("Longevity processing should succeed");

        // Should have one result
        assert_eq!(results.len(), 1);
        let result = &results[0];

        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 1010); // +10
        assert_eq!(result.delta, ELO_LONGEVITY_BONUS);
        assert_eq!(result.event_type, EloEventType::LongevityBonus);

        // Verify agent was updated
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1010);

        // Verify contribution marked as bonus paid
        let contribs = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert!(contribs[0].longevity_bonus_paid);
    }

    #[tokio::test]
    async fn test_longevity_bonus_not_awarded_before_30_days() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a contribution from only 15 days ago
        let merged_at = Utc::now() - Duration::days(15);
        let contribution = test_code_contribution_merged_at(agent.id, project.id, merged_at);

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo,
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo,
        );

        let results = service
            .process_longevity_bonuses()
            .await
            .expect("Longevity processing should succeed");

        // No bonuses should be awarded
        assert!(results.is_empty());

        // Agent ELO unchanged
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1000);
    }

    #[tokio::test]
    async fn test_code_replaced_within_window() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a contribution from 3 days ago (within 7-day window)
        let merged_at = Utc::now() - Duration::days(3);
        let contribution = test_code_contribution_merged_at(agent.id, project.id, merged_at);
        let contribution_id = contribution.id;

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo,
        );

        let result = service
            .on_code_replaced(&contribution_id)
            .await
            .expect("Code replaced should succeed")
            .expect("Should return result within window");

        // Verify ELO deduction
        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 990); // -10
        assert_eq!(result.delta, ELO_CODE_REPLACED);
        assert_eq!(result.event_type, EloEventType::CodeReplaced);

        // Verify contribution status updated
        let contribs = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(contribs[0].status, ContributionStatus::Replaced);
    }

    #[tokio::test]
    async fn test_code_replaced_outside_window() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a contribution from 10 days ago (outside 7-day window)
        let merged_at = Utc::now() - Duration::days(10);
        let contribution = test_code_contribution_merged_at(agent.id, project.id, merged_at);
        let contribution_id = contribution.id;

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo,
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo,
        );

        let result = service
            .on_code_replaced(&contribution_id)
            .await
            .expect("Code replaced should succeed");

        // No penalty outside window
        assert!(result.is_none());

        // Agent ELO unchanged
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1000);
    }

    #[tokio::test]
    async fn test_elo_cannot_go_below_zero() {
        let agent = test_agent_with_elo(10); // Low ELO
        let project = test_project();

        // Create a contribution
        let contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::hours(1));
        let commit_sha = contribution.commit_sha.clone();

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo,
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo,
        );

        let result = service
            .on_commit_reverted(&commit_sha, "revert123")
            .await
            .expect("Revert should succeed")
            .expect("Should return result");

        // ELO should be clamped to 0, not negative
        assert_eq!(result.old_elo, 10);
        assert_eq!(result.new_elo, 0); // Clamped at 0, not -20
        assert_eq!(result.delta, ELO_COMMIT_REVERTED); // Still records full delta

        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 0);
    }

    #[tokio::test]
    async fn test_dependent_pr_awards_elo() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        let contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::hours(1));

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo = Arc::new(
            InMemoryCodeContributionRepository::new().with_contribution(contribution.clone()),
        );
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo,
        );

        let result = service
            .on_dependent_pr(&contribution)
            .await
            .expect("Dependent PR should succeed");

        // Verify ELO bonus
        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 1005); // +5
        assert_eq!(result.delta, ELO_DEPENDENT_PR);
        assert_eq!(result.event_type, EloEventType::DependentPr);

        // Verify dependent count incremented
        let contribs = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(contribs[0].dependent_prs_count, 1);
    }

    // ==========================================================================
    // Rate limiting tests
    // ==========================================================================

    #[tokio::test]
    async fn test_review_rate_limit_enforced() {
        use crate::app::elo_config::MAX_REVIEWS_PER_HOUR;
        use crate::test_utils::test_agent_review;

        let reviewer = test_agent_with_elo(1500);
        let reviewed = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(
            InMemoryAgentRepository::new()
                .with_agent(reviewer.clone())
                .with_agent(reviewed.clone()),
        );

        // Pre-populate with MAX_REVIEWS_PER_HOUR reviews from within the last hour
        let mut review_repo = InMemoryAgentReviewRepository::new();
        for i in 0..MAX_REVIEWS_PER_HOUR {
            let mut review = test_agent_review(
                reviewer.id,
                reviewed.id,
                project.id,
                ReviewVerdict::Approved,
            );
            review.pr_id = i; // Different PRs
            review_repo = review_repo.with_review(review);
        }
        let review_repo = Arc::new(review_repo);

        let service = ReactiveEloService::new(
            agent_repo,
            Arc::new(InMemoryCodeContributionRepository::new()),
            review_repo,
            Arc::new(InMemoryEloEventRepository::new()),
        );

        // Next review should fail rate limit
        let result = service
            .on_peer_review(
                999, // New PR
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::Approved,
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("rate limit exceeded"));
    }

    // ==========================================================================
    // Changes Requested verdict tests
    // ==========================================================================

    #[tokio::test]
    async fn test_peer_review_changes_requested_no_bonus() {
        let reviewer = test_agent_with_elo(1500); // High ELO
        let reviewed = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(
            InMemoryAgentRepository::new()
                .with_agent(reviewer.clone())
                .with_agent(reviewed.clone()),
        );
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            Arc::new(InMemoryCodeContributionRepository::new()),
            review_repo.clone(),
            elo_repo.clone(),
        );

        // Changes requested should NOT award bonus even from high-ELO reviewer
        let result = service
            .on_peer_review(
                42,
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::ChangesRequested,
            )
            .await
            .expect("Review should succeed");

        // No bonus for changes_requested
        assert!(result.is_none());

        // Reviewed agent ELO unchanged
        let updated_reviewed = agent_repo.find_by_id(&reviewed.id).await.unwrap().unwrap();
        assert_eq!(updated_reviewed.elo, 1000);

        // But review should still be recorded
        let reviews = review_repo.find_by_reviewer(&reviewer.id).await.unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].verdict, ReviewVerdict::ChangesRequested);
    }

    // ==========================================================================
    // Longevity bonus idempotence tests
    // ==========================================================================

    #[tokio::test]
    async fn test_longevity_bonus_not_paid_twice() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a contribution already marked as bonus paid
        let mut contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::days(31));
        contribution.longevity_bonus_paid = true;

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo,
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo.clone(),
        );

        let results = service
            .process_longevity_bonuses()
            .await
            .expect("Longevity processing should succeed");

        // No bonuses should be awarded (already paid)
        assert!(results.is_empty());

        // No ELO events created
        let events = elo_repo.find_by_agent(&agent.id).await.unwrap();
        assert!(events.is_empty());

        // Agent ELO unchanged
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1000);
    }

    #[tokio::test]
    async fn test_longevity_bonus_not_paid_for_reverted() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        // Create a reverted contribution from 31 days ago
        let mut contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::days(31));
        contribution.status = ContributionStatus::Reverted;
        contribution.reverted_at = Some(Utc::now() - Duration::days(20));

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo,
            Arc::new(InMemoryAgentReviewRepository::new()),
            Arc::new(InMemoryEloEventRepository::new()),
        );

        let results = service
            .process_longevity_bonuses()
            .await
            .expect("Longevity processing should succeed");

        // No bonuses for reverted contributions
        assert!(results.is_empty());
    }

    // ==========================================================================
    // Multiple dependent PRs tests
    // ==========================================================================

    #[tokio::test]
    async fn test_multiple_dependent_prs_accumulate() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        let contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::hours(1));

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo = Arc::new(
            InMemoryCodeContributionRepository::new().with_contribution(contribution.clone()),
        );
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo.clone(),
        );

        // First dependent PR
        let result1 = service
            .on_dependent_pr(&contribution)
            .await
            .expect("First dependent PR should succeed");
        assert_eq!(result1.new_elo, 1005);

        // Re-fetch contribution with updated count
        let updated_contrib = contrib_repo
            .find_by_id(&contribution.id)
            .await
            .unwrap()
            .unwrap();

        // Second dependent PR
        let result2 = service
            .on_dependent_pr(&updated_contrib)
            .await
            .expect("Second dependent PR should succeed");
        assert_eq!(result2.old_elo, 1005);
        assert_eq!(result2.new_elo, 1010); // +5 more

        // Verify dependent count
        let final_contrib = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(final_contrib[0].dependent_prs_count, 2);

        // Verify multiple ELO events
        let events = elo_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(events.len(), 2);
    }

    // ==========================================================================
    // Low peer review score tests
    // ==========================================================================

    #[tokio::test]
    async fn test_low_peer_review_score_deducts_elo() {
        let agent = test_agent_with_elo(1000);
        let (service, agent_repo, _, elo_repo) = create_service_with_agent(agent.clone());

        let result = service
            .on_low_peer_review_score(&agent.id, 42, "Multiple code quality issues")
            .await
            .expect("Low peer review score should succeed");

        assert_eq!(result.old_elo, 1000);
        assert_eq!(result.new_elo, 990); // -10
        assert_eq!(result.delta, ELO_LOW_PEER_REVIEW);
        assert_eq!(result.event_type, EloEventType::LowPeerReviewScore);

        // Verify agent updated
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 990);

        // Verify ELO event created
        let events = elo_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].details.as_ref().unwrap().contains("code quality"));
    }

    // ==========================================================================
    // Sequential ELO changes tests
    // ==========================================================================

    #[tokio::test]
    async fn test_multiple_sequential_elo_changes() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo = Arc::new(InMemoryCodeContributionRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo.clone(),
        );

        // PR merged: +15 -> 1015
        let result1 = service
            .on_pr_merged(&agent.id, &project.id, 1, "sha1")
            .await
            .expect("PR merge should succeed");
        assert_eq!(result1.new_elo, 1015);

        // Another PR merged: +15 -> 1030
        let result2 = service
            .on_pr_merged(&agent.id, &project.id, 2, "sha2")
            .await
            .expect("PR merge should succeed");
        assert_eq!(result2.old_elo, 1015);
        assert_eq!(result2.new_elo, 1030);

        // PR rejected: -5 -> 1025
        let result3 = service
            .on_pr_rejected(&agent.id, &project.id, 3)
            .await
            .expect("PR rejection should succeed");
        assert_eq!(result3.old_elo, 1030);
        assert_eq!(result3.new_elo, 1025);

        // Verify final ELO
        let final_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(final_agent.elo, 1025);

        // Verify audit trail
        let events = elo_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(events.len(), 3);

        // Verify ELO delta sum matches final ELO
        let sum = elo_repo.sum_delta_by_agent(&agent.id).await.unwrap();
        assert_eq!(sum, 25); // +15 +15 -5 = 25
    }

    // ==========================================================================
    // Edge case tests
    // ==========================================================================

    #[tokio::test]
    async fn test_multiple_elo_changes_ending_below_zero() {
        let agent = test_agent_with_elo(20); // Start low
        let project = test_project();

        // Create contribution
        let contribution =
            test_code_contribution_merged_at(agent.id, project.id, Utc::now() - Duration::hours(1));
        let commit_sha = contribution.commit_sha.clone();

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo =
            Arc::new(InMemoryCodeContributionRepository::new().with_contribution(contribution));

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo,
            Arc::new(InMemoryAgentReviewRepository::new()),
            Arc::new(InMemoryEloEventRepository::new()),
        );

        // Revert: -30, but clamped at 0
        let result = service
            .on_commit_reverted(&commit_sha, "revert1")
            .await
            .expect("Revert should succeed")
            .expect("Should return result");

        assert_eq!(result.old_elo, 20);
        assert_eq!(result.new_elo, 0); // Clamped at 0

        // PR rejected on agent at 0: stays at 0
        let result2 = service
            .on_pr_rejected(&agent.id, &project.id, 99)
            .await
            .expect("PR rejection should succeed");

        assert_eq!(result2.old_elo, 0);
        assert_eq!(result2.new_elo, 0); // Still 0, can't go negative
    }

    #[tokio::test]
    async fn test_nonexistent_agent_returns_error() {
        let service = create_test_service();
        let fake_agent_id = crate::domain::entities::AgentId::new();

        let result = service
            .apply_elo_change(
                &fake_agent_id,
                10,
                EloEventType::PrMerged,
                None,
                Some("test".to_string()),
            )
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[tokio::test]
    async fn test_bug_reference_unknown_pr_returns_none() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        let (service, agent_repo, _, _) = create_service_with_agent(agent.clone());

        // Reference a PR that doesn't exist
        let result = service
            .on_bug_referenced(&project.id, 999, "https://gitea.local/issues/1")
            .await
            .expect("Bug reference should succeed");

        assert!(result.is_none());

        // Agent ELO unchanged
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1000);
    }

    #[tokio::test]
    async fn test_revert_unknown_commit_returns_none() {
        let agent = test_agent_with_elo(1000);
        let (service, agent_repo, _, _) = create_service_with_agent(agent.clone());

        // Revert a commit that doesn't exist
        let result = service
            .on_commit_reverted("nonexistent_sha", "revert_sha")
            .await
            .expect("Revert should succeed");

        assert!(result.is_none());

        // Agent ELO unchanged
        let updated_agent = agent_repo.find_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(updated_agent.elo, 1000);
    }

    // ==========================================================================
    // Parser edge case tests
    // ==========================================================================

    #[test]
    fn parse_revert_commit_prefix_only() {
        let msg = "Revert abc1234567890";
        assert_eq!(parse_revert_commit(msg), Some("abc1234567890".to_string()));
    }

    #[test]
    fn parse_revert_commit_with_quotes() {
        let msg = "Revert \"Fix bug\"\n\nThis reverts commit deadbeef123.";
        assert_eq!(parse_revert_commit(msg), Some("deadbeef123".to_string()));
    }

    #[test]
    fn parse_revert_commit_case_sensitive() {
        // "revert" lowercase at start doesn't match
        let msg = "revert abc123def";
        assert_eq!(parse_revert_commit(msg), None);
    }

    #[test]
    fn parse_revert_commit_sha_with_non_hex() {
        // SHA can't contain non-hex chars
        let msg = "This reverts commit abc123xyz";
        // Should extract "abc123" (6 chars is < 7, so None)
        assert_eq!(parse_revert_commit(msg), None);
    }

    #[test]
    fn parse_bug_references_hash_only() {
        let body = "Bug introduced in #123";
        let refs = parse_bug_references(body);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, 123);
    }

    #[test]
    fn parse_bug_references_pr_prefix() {
        let body = "See PR #456 and PR#789";
        let refs = parse_bug_references(body);
        assert_eq!(refs.len(), 2);
        assert!(refs.iter().any(|(n, _)| *n == 456));
        assert!(refs.iter().any(|(n, _)| *n == 789));
    }

    #[test]
    fn parse_bug_references_empty_body() {
        let body = "";
        let refs = parse_bug_references(body);
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_bug_references_duplicates_kept() {
        // Same PR referenced twice
        let body = "Related to #10 and also #10";
        let refs = parse_bug_references(body);
        assert_eq!(refs.len(), 2); // Both occurrences captured
    }

    // ==========================================================================
    // ELO threshold boundary tests
    // ==========================================================================

    #[tokio::test]
    async fn test_peer_review_exactly_at_threshold() {
        use crate::app::elo_config::HIGH_ELO_THRESHOLD;

        let reviewer = test_agent_with_elo(HIGH_ELO_THRESHOLD); // Exactly at threshold
        let reviewed = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(
            InMemoryAgentRepository::new()
                .with_agent(reviewer.clone())
                .with_agent(reviewed.clone()),
        );

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            Arc::new(InMemoryCodeContributionRepository::new()),
            Arc::new(InMemoryAgentReviewRepository::new()),
            Arc::new(InMemoryEloEventRepository::new()),
        );

        let result = service
            .on_peer_review(
                42,
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::Approved,
            )
            .await
            .expect("Review should succeed");

        // Should get bonus at exactly threshold
        assert!(result.is_some());
        assert_eq!(result.unwrap().delta, ELO_HIGH_ELO_APPROVAL);
    }

    #[tokio::test]
    async fn test_peer_review_one_below_threshold() {
        use crate::app::elo_config::HIGH_ELO_THRESHOLD;

        let reviewer = test_agent_with_elo(HIGH_ELO_THRESHOLD - 1); // Just below
        let reviewed = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(
            InMemoryAgentRepository::new()
                .with_agent(reviewer.clone())
                .with_agent(reviewed.clone()),
        );

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            Arc::new(InMemoryCodeContributionRepository::new()),
            Arc::new(InMemoryAgentReviewRepository::new()),
            Arc::new(InMemoryEloEventRepository::new()),
        );

        let result = service
            .on_peer_review(
                42,
                &project.id,
                &reviewer.id,
                &reviewed.id,
                ReviewVerdict::Approved,
            )
            .await
            .expect("Review should succeed");

        // No bonus just below threshold
        assert!(result.is_none());
    }

    // ==========================================================================
    // Contribution lifecycle tests
    // ==========================================================================

    #[tokio::test]
    async fn test_full_contribution_lifecycle() {
        let agent = test_agent_with_elo(1000);
        let project = test_project();

        let agent_repo = Arc::new(InMemoryAgentRepository::new().with_agent(agent.clone()));
        let contrib_repo = Arc::new(InMemoryCodeContributionRepository::new());
        let elo_repo = Arc::new(InMemoryEloEventRepository::new());

        let service = ReactiveEloService::new(
            agent_repo.clone(),
            contrib_repo.clone(),
            Arc::new(InMemoryAgentReviewRepository::new()),
            elo_repo.clone(),
        );

        // 1. PR merged: +15 -> 1015
        let result1 = service
            .on_pr_merged(&agent.id, &project.id, 42, "commit_sha_123")
            .await
            .expect("PR merge should succeed");
        assert_eq!(result1.new_elo, 1015);

        // 2. Bug reported referencing this PR: -15 -> 1000
        let result2 = service
            .on_bug_referenced(&project.id, 42, "https://gitea.local/issues/99")
            .await
            .expect("Bug reference should succeed")
            .expect("Should return result");
        assert_eq!(result2.new_elo, 1000);

        // Verify contribution bug count
        let contribs = contrib_repo.find_by_pr(&project.id, 42).await.unwrap();
        assert_eq!(contribs.unwrap().bug_count, 1);

        // 3. Code reverted: -30 -> 970 (but clamped, stays at 970 since 1000 - 30 = 970)
        let result3 = service
            .on_commit_reverted("commit_sha_123", "revert_commit")
            .await
            .expect("Revert should succeed")
            .expect("Should return result");
        assert_eq!(result3.new_elo, 970);

        // Verify contribution status
        let contribs = contrib_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(contribs[0].status, ContributionStatus::Reverted);

        // Verify full audit trail
        let events = elo_repo.find_by_agent(&agent.id).await.unwrap();
        assert_eq!(events.len(), 3);

        // Net ELO change: +15 -15 -30 = -30
        let sum = elo_repo.sum_delta_by_agent(&agent.id).await.unwrap();
        assert_eq!(sum, -30);
    }
}
