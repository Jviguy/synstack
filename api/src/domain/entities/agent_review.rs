//! Agent review domain entity
//!
//! Represents peer reviews between agents on PRs. Higher-ELO reviewers
//! have more weight in the reactive ELO system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent::AgentId;
use super::project::ProjectId;

/// Unique identifier for an agent review
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentReviewId(pub Uuid);

impl AgentReviewId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AgentReviewId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for AgentReviewId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for AgentReviewId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The verdict of a peer review
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewVerdict {
    /// Review approved the changes
    Approved,
    /// Review requested changes
    ChangesRequested,
}

impl std::fmt::Display for ReviewVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewVerdict::Approved => write!(f, "approved"),
            ReviewVerdict::ChangesRequested => write!(f, "changes_requested"),
        }
    }
}

impl std::str::FromStr for ReviewVerdict {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "approved" => Ok(ReviewVerdict::Approved),
            "changes_requested" => Ok(ReviewVerdict::ChangesRequested),
            _ => Err(format!("Unknown review verdict: {}", s)),
        }
    }
}

/// ELO threshold for "high-ELO" reviewer bonus
pub const HIGH_ELO_THRESHOLD: i32 = 1400;

/// A peer review from one agent on another agent's PR
#[derive(Debug, Clone, Serialize)]
pub struct AgentReview {
    pub id: AgentReviewId,
    pub pr_id: i64,
    pub project_id: ProjectId,
    pub reviewer_agent_id: AgentId,
    pub reviewed_agent_id: AgentId,
    pub verdict: ReviewVerdict,
    /// Snapshot of reviewer's ELO at time of review (for weighting)
    pub reviewer_elo_at_time: i32,
    pub created_at: DateTime<Utc>,
}

impl AgentReview {
    /// Check if this is a high-ELO approval (reviewer ELO >= 1400)
    pub fn is_high_elo_approval(&self) -> bool {
        self.verdict == ReviewVerdict::Approved && self.reviewer_elo_at_time >= HIGH_ELO_THRESHOLD
    }

    /// Check if this would be a self-review (invalid)
    pub fn is_self_review(&self) -> bool {
        self.reviewer_agent_id == self.reviewed_agent_id
    }
}

/// Data needed to create a new agent review
#[derive(Debug, Clone)]
pub struct NewAgentReview {
    pub pr_id: i64,
    pub project_id: ProjectId,
    pub reviewer_agent_id: AgentId,
    pub reviewed_agent_id: AgentId,
    pub verdict: ReviewVerdict,
    pub reviewer_elo_at_time: i32,
}

impl NewAgentReview {
    /// Check if this would be a self-review (invalid)
    pub fn is_self_review(&self) -> bool {
        self.reviewer_agent_id == self.reviewed_agent_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_review(verdict: ReviewVerdict, reviewer_elo: i32, same_agent: bool) -> AgentReview {
        let reviewer_id = AgentId::new();
        let reviewed_id = if same_agent {
            reviewer_id
        } else {
            AgentId::new()
        };

        AgentReview {
            id: AgentReviewId::new(),
            pr_id: 123,
            project_id: ProjectId::new(),
            reviewer_agent_id: reviewer_id,
            reviewed_agent_id: reviewed_id,
            verdict,
            reviewer_elo_at_time: reviewer_elo,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn review_verdict_display() {
        assert_eq!(ReviewVerdict::Approved.to_string(), "approved");
        assert_eq!(
            ReviewVerdict::ChangesRequested.to_string(),
            "changes_requested"
        );
    }

    #[test]
    fn review_verdict_from_str() {
        assert_eq!(
            "approved".parse::<ReviewVerdict>().unwrap(),
            ReviewVerdict::Approved
        );
        assert_eq!(
            "CHANGES_REQUESTED".parse::<ReviewVerdict>().unwrap(),
            ReviewVerdict::ChangesRequested
        );
        assert!("invalid".parse::<ReviewVerdict>().is_err());
    }

    #[test]
    fn is_high_elo_approval_true() {
        let review = make_review(ReviewVerdict::Approved, 1500, false);
        assert!(review.is_high_elo_approval());
    }

    #[test]
    fn is_high_elo_approval_false_low_elo() {
        let review = make_review(ReviewVerdict::Approved, 1200, false);
        assert!(!review.is_high_elo_approval());
    }

    #[test]
    fn is_high_elo_approval_false_changes_requested() {
        let review = make_review(ReviewVerdict::ChangesRequested, 1500, false);
        assert!(!review.is_high_elo_approval());
    }

    #[test]
    fn is_high_elo_approval_boundary() {
        // Exactly at threshold should qualify
        let review = make_review(ReviewVerdict::Approved, HIGH_ELO_THRESHOLD, false);
        assert!(review.is_high_elo_approval());

        // One below threshold should not
        let review = make_review(ReviewVerdict::Approved, HIGH_ELO_THRESHOLD - 1, false);
        assert!(!review.is_high_elo_approval());
    }

    #[test]
    fn is_self_review_true() {
        let review = make_review(ReviewVerdict::Approved, 1400, true);
        assert!(review.is_self_review());
    }

    #[test]
    fn is_self_review_false() {
        let review = make_review(ReviewVerdict::Approved, 1400, false);
        assert!(!review.is_self_review());
    }

    #[test]
    fn new_agent_review_is_self_review() {
        let agent_id = AgentId::new();
        let new_review = NewAgentReview {
            pr_id: 123,
            project_id: ProjectId::new(),
            reviewer_agent_id: agent_id,
            reviewed_agent_id: agent_id,
            verdict: ReviewVerdict::Approved,
            reviewer_elo_at_time: 1400,
        };
        assert!(new_review.is_self_review());
    }

    #[test]
    fn agent_review_id_display() {
        let id = AgentReviewId(Uuid::nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }
}
