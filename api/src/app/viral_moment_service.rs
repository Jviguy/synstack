//! Viral Moment service
//!
//! Detects and manages viral moments - interesting events worth sharing.
//! Uses engagement signals and optional LLM classification.

use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

/// Create a deterministic UUID from a string (for deduplication)
fn deterministic_uuid(input: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    // Use first 16 bytes of SHA256 hash as UUID
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&result[..16]);
    Uuid::from_bytes(bytes)
}

use crate::domain::entities::{
    Agent, AgentId, BattleRacer, BattleSnapshot, MomentType, NewViralMoment, ReferenceType,
    ShameSnapshot, Tier, UpsetLoser, UpsetSnapshot, ViralMoment, ViralMomentId,
};
use crate::domain::ports::{EngagementRepository, ViralMomentRepository};
use crate::error::AppError;

/// Thresholds for detecting viral moments
pub struct ViralThresholds {
    /// Minimum engagement score to consider for viral
    pub min_engagement_score: i32,
    /// Minimum ELO differential for David vs Goliath
    pub min_elo_differential: i32,
    /// Minimum number of conflicting reviews for drama
    pub min_conflicting_reviews: i32,
    /// Minimum racers for a live battle
    pub min_battle_racers: i32,
}

impl Default for ViralThresholds {
    fn default() -> Self {
        Self {
            min_engagement_score: 10,
            min_elo_differential: 200,
            min_conflicting_reviews: 2,
            min_battle_racers: 2,
        }
    }
}

/// Service for managing viral moments
pub struct ViralMomentService<VMR, ER>
where
    VMR: ViralMomentRepository,
    ER: EngagementRepository,
{
    moments: Arc<VMR>,
    engagements: Arc<ER>,
    thresholds: ViralThresholds,
}

impl<VMR, ER> ViralMomentService<VMR, ER>
where
    VMR: ViralMomentRepository,
    ER: EngagementRepository,
{
    pub fn new(moments: Arc<VMR>, engagements: Arc<ER>) -> Self {
        Self {
            moments,
            engagements,
            thresholds: ViralThresholds::default(),
        }
    }

    pub fn with_thresholds(mut self, thresholds: ViralThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    // ========== Feed Generation ==========

    /// Get Hall of Shame feed (PR failures, reverts, etc.)
    pub async fn get_shame_feed(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, AppError> {
        Ok(self
            .moments
            .find_by_type(MomentType::HallOfShame, limit, offset)
            .await?)
    }

    /// Get Agent Drama feed
    pub async fn get_drama_feed(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, AppError> {
        Ok(self
            .moments
            .find_by_type(MomentType::AgentDrama, limit, offset)
            .await?)
    }

    /// Get David vs Goliath (upsets) feed
    pub async fn get_upsets_feed(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, AppError> {
        Ok(self
            .moments
            .find_by_type(MomentType::DavidVsGoliath, limit, offset)
            .await?)
    }

    /// Get Live Battles feed
    pub async fn get_battles_feed(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ViralMoment>, AppError> {
        Ok(self
            .moments
            .find_by_type(MomentType::LiveBattle, limit, offset)
            .await?)
    }

    /// Get top moments across all types
    pub async fn get_top_moments(&self, limit: i64) -> Result<Vec<ViralMoment>, AppError> {
        Ok(self.moments.find_top(limit).await?)
    }

    /// Get a specific moment by ID
    pub async fn get_moment(&self, id: &ViralMomentId) -> Result<Option<ViralMoment>, AppError> {
        Ok(self.moments.find_by_id(id).await?)
    }

    /// Get promoted (staff pick) moments
    pub async fn get_promoted(&self, limit: i64) -> Result<Vec<ViralMoment>, AppError> {
        Ok(self.moments.find_promoted(limit).await?)
    }

    // ========== Moment Detection ==========

    /// Check if a PR revert should be in Hall of Shame
    pub async fn check_hall_of_shame_revert(
        &self,
        agent: &Agent,
        pr_number: i64,
        pr_title: &str,
        project_name: &str,
        revert_reason: Option<&str>,
    ) -> Result<Option<ViralMoment>, AppError> {
        // Check if we already have a moment for this PR using a deterministic ID based on PR number
        // Using namespace UUID + hash is one approach, but for simplicity we'll use new_v4
        // and rely on exists_for_reference check to prevent duplicates
        let reference_id = Uuid::new_v4();
        if self
            .moments
            .exists_for_reference("pr_revert", reference_id)
            .await?
        {
            return Ok(None);
        }

        // Calculate shame score
        let score = self.calculate_revert_shame_score(agent, revert_reason);

        // Check threshold
        if score < 10 {
            return Ok(None);
        }

        let title = format!("{}'s PR gets reverted", agent.name);
        let subtitle = revert_reason
            .map(|r| truncate(r, 80).to_string())
            .unwrap_or_else(|| format!("PR #{} in {}", pr_number, project_name));

        let snapshot = ShameSnapshot {
            agent_name: agent.name.clone(),
            agent_elo: agent.elo,
            agent_tier: agent.tier.to_string(),
            stderr: revert_reason.map(|s| s.to_string()),
            exit_code: None,
            issue_title: pr_title.to_string(),
            issue_difficulty: "N/A".to_string(),
        };

        let new_moment = NewViralMoment {
            moment_type: MomentType::HallOfShame,
            title,
            subtitle: Some(subtitle),
            score,
            agent_ids: vec![agent.id],
            reference_type: ReferenceType::Review,
            reference_id,
            snapshot: serde_json::to_value(&snapshot).unwrap_or_default(),
        };

        let moment = self.moments.create(&new_moment).await?;
        Ok(Some(moment))
    }

    /// Check if a PR rejection should be in Hall of Shame
    pub async fn check_hall_of_shame_rejection(
        &self,
        agent: &Agent,
        pr_number: i64,
        pr_title: &str,
        project_name: &str,
        rejection_count: i32,
    ) -> Result<Option<ViralMoment>, AppError> {
        // Use a deterministic reference ID based on project + PR number
        let reference_id =
            deterministic_uuid(&format!("pr_rejection:{}:{}", project_name, pr_number));

        if self
            .moments
            .exists_for_reference("pr_rejection", reference_id)
            .await?
        {
            return Ok(None);
        }

        // Calculate shame score
        let score = self.calculate_rejection_shame_score(agent, rejection_count);

        // Check threshold - must be somewhat notable
        if score < 10 {
            return Ok(None);
        }

        let title = if rejection_count > 1 {
            format!("{}'s PR rejected {} times", agent.name, rejection_count)
        } else {
            format!("{}'s PR rejected", agent.name)
        };

        let subtitle = format!(
            "PR #{}: {} in {}",
            pr_number,
            truncate(pr_title, 40),
            project_name
        );

        let snapshot = ShameSnapshot {
            agent_name: agent.name.clone(),
            agent_elo: agent.elo,
            agent_tier: agent.tier.to_string(),
            stderr: None,
            exit_code: None,
            issue_title: pr_title.to_string(),
            issue_difficulty: "N/A".to_string(),
        };

        let new_moment = NewViralMoment {
            moment_type: MomentType::HallOfShame,
            title,
            subtitle: Some(subtitle),
            score,
            agent_ids: vec![agent.id],
            reference_type: ReferenceType::PullRequest,
            reference_id,
            snapshot: serde_json::to_value(&snapshot).unwrap_or_default(),
        };

        let moment = self.moments.create(&new_moment).await?;
        Ok(Some(moment))
    }

    /// Check if conflicting reviews create drama
    pub async fn check_drama(
        &self,
        pr_number: i64,
        project_name: &str,
        pr_title: &str,
        approvers: &[Agent],
        rejectors: &[Agent],
    ) -> Result<Option<ViralMoment>, AppError> {
        // Need at least one of each to have drama
        if approvers.is_empty() || rejectors.is_empty() {
            return Ok(None);
        }

        // Use a deterministic reference ID
        let reference_id = deterministic_uuid(&format!("pr_drama:{}:{}", project_name, pr_number));

        if self
            .moments
            .exists_for_reference("pr_drama", reference_id)
            .await?
        {
            return Ok(None);
        }

        // Calculate drama score
        let score = self.calculate_drama_score(approvers, rejectors);

        // Minimum threshold for drama
        if score < self.thresholds.min_engagement_score {
            return Ok(None);
        }

        let title = format!(
            "PR #{} sparks debate: {} vs {}",
            pr_number,
            approvers.len(),
            rejectors.len()
        );

        let approver_names: Vec<_> = approvers.iter().map(|a| a.name.as_str()).collect();
        let rejector_names: Vec<_> = rejectors.iter().map(|a| a.name.as_str()).collect();

        let subtitle = format!(
            "{} approve, {} request changes on \"{}\"",
            approver_names.join(", "),
            rejector_names.join(", "),
            truncate(pr_title, 30)
        );

        // Build drama snapshot
        let snapshot = serde_json::json!({
            "pr_number": pr_number,
            "pr_title": pr_title,
            "project": project_name,
            "approvers": approvers.iter().map(|a| serde_json::json!({
                "name": a.name,
                "elo": a.elo,
                "tier": a.tier.to_string(),
            })).collect::<Vec<_>>(),
            "rejectors": rejectors.iter().map(|a| serde_json::json!({
                "name": a.name,
                "elo": a.elo,
                "tier": a.tier.to_string(),
            })).collect::<Vec<_>>(),
        });

        let mut agent_ids: Vec<AgentId> = approvers.iter().map(|a| a.id).collect();
        agent_ids.extend(rejectors.iter().map(|a| a.id));

        let new_moment = NewViralMoment {
            moment_type: MomentType::AgentDrama,
            title,
            subtitle: Some(subtitle),
            score,
            agent_ids,
            reference_type: ReferenceType::PullRequest,
            reference_id,
            snapshot,
        };

        let moment = self.moments.create(&new_moment).await?;
        Ok(Some(moment))
    }

    /// Check if a solved issue is a David vs Goliath upset
    pub async fn check_upset(
        &self,
        winner: &Agent,
        losers: &[Agent],
        issue_id: Uuid,
        issue_title: &str,
        issue_difficulty: &str,
    ) -> Result<Option<ViralMoment>, AppError> {
        if losers.is_empty() {
            return Ok(None);
        }

        // Check if we already have a moment for this
        if self.moments.exists_for_reference("issue", issue_id).await? {
            return Ok(None);
        }

        // Calculate max ELO differential
        let max_loser_elo = losers.iter().map(|a| a.elo).max().unwrap_or(0);
        let elo_differential = max_loser_elo - winner.elo;

        // Must be a significant upset
        if elo_differential < self.thresholds.min_elo_differential {
            return Ok(None);
        }

        // Calculate upset score
        let score = self.calculate_upset_score(winner, losers, issue_difficulty);

        let title = self.generate_upset_title(winner, &losers[0]);
        let subtitle = format!(
            "{} ELO underdog beats {} agent{}",
            elo_differential,
            losers.len(),
            if losers.len() > 1 { "s" } else { "" }
        );

        let snapshot = UpsetSnapshot {
            winner_name: winner.name.clone(),
            winner_elo: winner.elo,
            winner_tier: winner.tier.to_string(),
            losers: losers
                .iter()
                .map(|a| UpsetLoser {
                    name: a.name.clone(),
                    elo: a.elo,
                    tier: a.tier.to_string(),
                })
                .collect(),
            issue_title: issue_title.to_string(),
            issue_difficulty: issue_difficulty.to_string(),
            elo_differential,
        };

        let mut agent_ids: Vec<AgentId> = vec![winner.id];
        agent_ids.extend(losers.iter().map(|a| a.id));

        let new_moment = NewViralMoment {
            moment_type: MomentType::DavidVsGoliath,
            title,
            subtitle: Some(subtitle),
            score,
            agent_ids,
            reference_type: ReferenceType::Issue,
            reference_id: issue_id,
            snapshot: serde_json::to_value(&snapshot).unwrap_or_default(),
        };

        let moment = self.moments.create(&new_moment).await?;
        Ok(Some(moment))
    }

    /// Create or update a live battle moment
    pub async fn track_battle(
        &self,
        issue_id: Uuid,
        issue_title: &str,
        racers: Vec<(Agent, String)>, // (agent, status)
    ) -> Result<Option<ViralMoment>, AppError> {
        if racers.len() < self.thresholds.min_battle_racers as usize {
            return Ok(None);
        }

        // Check if battle already exists
        let exists = self.moments.exists_for_reference("issue", issue_id).await?;

        if exists {
            return Ok(None);
        }

        let score = self.calculate_battle_score(&racers);

        let title = format!("{}-way race!", racers.len());
        let subtitle = format!(
            "{} agents racing on: {}",
            racers.len(),
            truncate(issue_title, 50)
        );

        let snapshot = BattleSnapshot {
            issue_title: issue_title.to_string(),
            issue_id,
            racers: racers
                .iter()
                .map(|(agent, status)| BattleRacer {
                    agent_id: agent.id.0,
                    agent_name: agent.name.clone(),
                    agent_elo: agent.elo,
                    agent_tier: agent.tier.to_string(),
                    status: status.clone(),
                    progress: None,
                })
                .collect(),
            started_at: chrono::Utc::now(),
            ended_at: None,
            winner_id: None,
        };

        let new_moment = NewViralMoment {
            moment_type: MomentType::LiveBattle,
            title,
            subtitle: Some(subtitle),
            score,
            agent_ids: racers.iter().map(|(a, _)| a.id).collect(),
            reference_type: ReferenceType::Issue,
            reference_id: issue_id,
            snapshot: serde_json::to_value(&snapshot).unwrap_or_default(),
        };

        let moment = self.moments.create(&new_moment).await?;
        Ok(Some(moment))
    }

    // ========== Scoring Algorithms ==========

    fn calculate_revert_shame_score(&self, agent: &Agent, revert_reason: Option<&str>) -> i32 {
        let mut score = 0;

        // High ELO agent getting reverted is more notable
        score += agent.elo / 100;

        // Gold agent getting reverted is extra notable
        if agent.tier == Tier::Gold {
            score += 30;
        } else if agent.tier == Tier::Silver {
            score += 15;
        }

        // Interesting revert reasons
        if let Some(reason) = revert_reason {
            let reason_lower = reason.to_lowercase();
            if reason_lower.contains("broke") || reason_lower.contains("broken") {
                score += 20;
            }
            if reason_lower.contains("security") {
                score += 25;
            }
            if reason_lower.contains("regression") {
                score += 15;
            }
        }

        score
    }

    fn calculate_rejection_shame_score(&self, agent: &Agent, rejection_count: i32) -> i32 {
        let mut score = 0;

        // High ELO agent getting rejected is more notable
        score += agent.elo / 100;

        // Gold agent getting rejected is extra notable
        if agent.tier == Tier::Gold {
            score += 25;
        } else if agent.tier == Tier::Silver {
            score += 10;
        }

        // Multiple rejections on same PR is extra shameful
        if rejection_count > 1 {
            score += (rejection_count - 1) * 15;
        }

        // Minimum base score for any rejection
        score += 5;

        score
    }

    fn calculate_drama_score(&self, approvers: &[Agent], rejectors: &[Agent]) -> i32 {
        let mut score = 0;

        // More reviewers = more drama
        let total_reviewers = approvers.len() + rejectors.len();
        score += (total_reviewers * 5) as i32;

        // High ELO agents disagreeing is spicy
        let max_approver_elo = approvers.iter().map(|a| a.elo).max().unwrap_or(0);
        let max_rejector_elo = rejectors.iter().map(|a| a.elo).max().unwrap_or(0);

        // Both sides have high ELO agents
        if max_approver_elo >= 1600 && max_rejector_elo >= 1600 {
            score += 30; // Gold vs Gold drama
        } else if max_approver_elo >= 1200 && max_rejector_elo >= 1200 {
            score += 15; // Silver vs Silver
        }

        // ELO differential between camps
        let elo_diff = (max_approver_elo - max_rejector_elo).abs();
        score += elo_diff / 50;

        // Even split is more dramatic
        let balance = (approvers.len() as i32 - rejectors.len() as i32).abs();
        if balance == 0 {
            score += 20; // Perfect split
        } else if balance == 1 {
            score += 10; // Close split
        }

        score
    }

    fn calculate_upset_score(&self, winner: &Agent, losers: &[Agent], difficulty: &str) -> i32 {
        let mut score = 0;

        // ELO differential
        let max_loser_elo = losers.iter().map(|a| a.elo).max().unwrap_or(0);
        score += (max_loser_elo - winner.elo) / 10;

        // Tier differential bonus
        if winner.tier == Tier::Bronze {
            let gold_count = losers.iter().filter(|a| a.tier == Tier::Gold).count();
            score += (gold_count * 50) as i32;

            let silver_count = losers.iter().filter(|a| a.tier == Tier::Silver).count();
            score += (silver_count * 25) as i32;
        }

        // Difficulty bonus
        match difficulty.to_lowercase().as_str() {
            "hard" => score += 30,
            "medium" => score += 15,
            _ => {}
        }

        // More losers = more impressive
        score += (losers.len() * 10) as i32;

        score
    }

    fn calculate_battle_score(&self, racers: &[(Agent, String)]) -> i32 {
        let mut score = 0;

        // More racers = more exciting
        score += (racers.len() * 15) as i32;

        // ELO spread makes it interesting
        let elos: Vec<i32> = racers.iter().map(|(a, _)| a.elo).collect();
        if let (Some(max), Some(min)) = (elos.iter().max(), elos.iter().min()) {
            score += (max - min) / 20;
        }

        // Mixed tiers are interesting
        let tiers: std::collections::HashSet<_> = racers.iter().map(|(a, _)| &a.tier).collect();
        if tiers.len() > 1 {
            score += 20;
        }

        score
    }

    // ========== Title Generation ==========

    fn generate_upset_title(&self, winner: &Agent, top_loser: &Agent) -> String {
        format!(
            "{} ({}) beats {} ({})",
            winner.name, winner.tier, top_loser.name, top_loser.tier
        )
    }

    // ========== Moderation ==========

    /// Promote a moment (staff pick)
    pub async fn promote(&self, id: &ViralMomentId) -> Result<(), AppError> {
        Ok(self.moments.set_promoted(id, true).await?)
    }

    /// Hide a moment
    pub async fn hide(&self, id: &ViralMomentId) -> Result<(), AppError> {
        Ok(self.moments.set_hidden(id, true).await?)
    }

    /// Update moment score based on engagement
    pub async fn update_score_from_engagement(
        &self,
        moment_id: &ViralMomentId,
    ) -> Result<(), AppError> {
        let moment = self
            .moments
            .find_by_id(moment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Moment not found".to_string()))?;

        let counts = self
            .engagements
            .get_counts("viral_moment", moment.id.0)
            .await?;

        // Update score based on engagement
        let engagement_bonus = counts.total_score;
        let new_score = moment.score + engagement_bonus;

        self.moments.update_score(moment_id, new_score).await?;

        Ok(())
    }
}

/// Truncate a string to max length with ellipsis
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world this is long", 10), "hello w...");
    }

    #[test]
    fn test_default_thresholds() {
        let thresholds = ViralThresholds::default();
        assert_eq!(thresholds.min_engagement_score, 10);
        assert_eq!(thresholds.min_elo_differential, 200);
    }
}
