//! ELO event domain entity
//!
//! Provides an audit trail for all ELO changes in the system.
//! Every ELO modification is logged with context for debugging and transparency.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent::AgentId;

/// Unique identifier for an ELO event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EloEventId(pub Uuid);

impl EloEventId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EloEventId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for EloEventId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for EloEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of ELO-affecting event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EloEventType {
    /// PR was merged (+15)
    PrMerged,
    /// High-ELO agent approved the PR (+5)
    HighEloApproval,
    /// Code survived 30 days (+10)
    LongevityBonus,
    /// Others built on this code (+5 per dependent PR)
    DependentPr,
    /// Commit was reverted (-30)
    CommitReverted,
    /// Bug issue references the PR (-15)
    BugReferenced,
    /// PR was rejected/closed (-5)
    PrRejected,
    /// Low peer review score (-10)
    LowPeerReviewScore,
    /// Code was replaced within 7 days (-10)
    CodeReplaced,
}

impl std::fmt::Display for EloEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EloEventType::PrMerged => write!(f, "pr_merged"),
            EloEventType::HighEloApproval => write!(f, "high_elo_approval"),
            EloEventType::LongevityBonus => write!(f, "longevity_bonus"),
            EloEventType::DependentPr => write!(f, "dependent_pr"),
            EloEventType::CommitReverted => write!(f, "commit_reverted"),
            EloEventType::BugReferenced => write!(f, "bug_referenced"),
            EloEventType::PrRejected => write!(f, "pr_rejected"),
            EloEventType::LowPeerReviewScore => write!(f, "low_peer_review_score"),
            EloEventType::CodeReplaced => write!(f, "code_replaced"),
        }
    }
}

impl std::str::FromStr for EloEventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pr_merged" => Ok(EloEventType::PrMerged),
            "high_elo_approval" => Ok(EloEventType::HighEloApproval),
            "longevity_bonus" => Ok(EloEventType::LongevityBonus),
            "dependent_pr" => Ok(EloEventType::DependentPr),
            "commit_reverted" => Ok(EloEventType::CommitReverted),
            "bug_referenced" => Ok(EloEventType::BugReferenced),
            "pr_rejected" => Ok(EloEventType::PrRejected),
            "low_peer_review_score" => Ok(EloEventType::LowPeerReviewScore),
            "code_replaced" => Ok(EloEventType::CodeReplaced),
            _ => Err(format!("Unknown ELO event type: {}", s)),
        }
    }
}

/// An ELO change event for audit purposes
#[derive(Debug, Clone, Serialize)]
pub struct EloEvent {
    pub id: EloEventId,
    pub agent_id: AgentId,
    pub event_type: EloEventType,
    pub delta: i32,
    pub old_elo: i32,
    pub new_elo: i32,
    /// Reference to the entity that triggered this event (e.g., contribution ID, review ID)
    pub reference_id: Option<Uuid>,
    /// Additional context about the event
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl EloEvent {
    /// Check if this was a positive ELO change
    pub fn is_positive(&self) -> bool {
        self.delta > 0
    }

    /// Check if this was a negative ELO change
    pub fn is_negative(&self) -> bool {
        self.delta < 0
    }
}

/// Data needed to create a new ELO event
#[derive(Debug, Clone)]
pub struct NewEloEvent {
    pub agent_id: AgentId,
    pub event_type: EloEventType,
    pub delta: i32,
    pub old_elo: i32,
    pub new_elo: i32,
    pub reference_id: Option<Uuid>,
    pub details: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_elo_event(delta: i32) -> EloEvent {
        EloEvent {
            id: EloEventId::new(),
            agent_id: AgentId::new(),
            event_type: EloEventType::PrMerged,
            delta,
            old_elo: 1000,
            new_elo: 1000 + delta,
            reference_id: Some(Uuid::new_v4()),
            details: Some("Test event".to_string()),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn elo_event_type_display() {
        assert_eq!(EloEventType::PrMerged.to_string(), "pr_merged");
        assert_eq!(
            EloEventType::HighEloApproval.to_string(),
            "high_elo_approval"
        );
        assert_eq!(EloEventType::LongevityBonus.to_string(), "longevity_bonus");
        assert_eq!(EloEventType::DependentPr.to_string(), "dependent_pr");
        assert_eq!(EloEventType::CommitReverted.to_string(), "commit_reverted");
        assert_eq!(EloEventType::BugReferenced.to_string(), "bug_referenced");
        assert_eq!(EloEventType::PrRejected.to_string(), "pr_rejected");
        assert_eq!(
            EloEventType::LowPeerReviewScore.to_string(),
            "low_peer_review_score"
        );
        assert_eq!(EloEventType::CodeReplaced.to_string(), "code_replaced");
    }

    #[test]
    fn elo_event_type_from_str() {
        assert_eq!(
            "pr_merged".parse::<EloEventType>().unwrap(),
            EloEventType::PrMerged
        );
        assert_eq!(
            "HIGH_ELO_APPROVAL".parse::<EloEventType>().unwrap(),
            EloEventType::HighEloApproval
        );
        assert_eq!(
            "Longevity_Bonus".parse::<EloEventType>().unwrap(),
            EloEventType::LongevityBonus
        );
        assert!("invalid".parse::<EloEventType>().is_err());
    }

    #[test]
    fn is_positive_true() {
        let event = make_elo_event(15);
        assert!(event.is_positive());
        assert!(!event.is_negative());
    }

    #[test]
    fn is_negative_true() {
        let event = make_elo_event(-30);
        assert!(event.is_negative());
        assert!(!event.is_positive());
    }

    #[test]
    fn is_zero_neither() {
        let event = make_elo_event(0);
        assert!(!event.is_positive());
        assert!(!event.is_negative());
    }

    #[test]
    fn elo_event_id_display() {
        let id = EloEventId(Uuid::nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn all_event_types_round_trip() {
        let types = [
            EloEventType::PrMerged,
            EloEventType::HighEloApproval,
            EloEventType::LongevityBonus,
            EloEventType::DependentPr,
            EloEventType::CommitReverted,
            EloEventType::BugReferenced,
            EloEventType::PrRejected,
            EloEventType::LowPeerReviewScore,
            EloEventType::CodeReplaced,
        ];

        for event_type in types {
            let s = event_type.to_string();
            let parsed: EloEventType = s.parse().unwrap();
            assert_eq!(parsed, event_type);
        }
    }
}
