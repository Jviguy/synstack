//! Viral Moment domain entity
//!
//! Represents interesting events worth sharing - failures, drama, upsets, live battles.
//! These are curated through engagement signals and LLM classification.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AgentId;

/// Unique identifier for a viral moment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViralMomentId(pub Uuid);

impl ViralMomentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ViralMomentId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ViralMomentId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ViralMomentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of viral moment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MomentType {
    /// Hall of Shame - hilarious agent failures
    HallOfShame,
    /// Agent Drama - PR review conflicts and debates
    AgentDrama,
    /// David vs Goliath - when bronze beats gold
    DavidVsGoliath,
    /// Live Battle - real-time race on an issue
    LiveBattle,
}

impl MomentType {
    /// Get a display name for the moment type
    pub fn display_name(&self) -> &'static str {
        match self {
            MomentType::HallOfShame => "Hall of Shame",
            MomentType::AgentDrama => "Agent Drama",
            MomentType::DavidVsGoliath => "David vs Goliath",
            MomentType::LiveBattle => "Live Battle",
        }
    }

    /// Get a short description
    pub fn description(&self) -> &'static str {
        match self {
            MomentType::HallOfShame => "When AI agents fail spectacularly",
            MomentType::AgentDrama => "PR review conflicts and heated debates",
            MomentType::DavidVsGoliath => "Underdog victories against the odds",
            MomentType::LiveBattle => "Real-time races to solve issues",
        }
    }
}

impl std::fmt::Display for MomentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MomentType::HallOfShame => write!(f, "hall_of_shame"),
            MomentType::AgentDrama => write!(f, "agent_drama"),
            MomentType::DavidVsGoliath => write!(f, "david_vs_goliath"),
            MomentType::LiveBattle => write!(f, "live_battle"),
        }
    }
}

impl std::str::FromStr for MomentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "hall_of_shame" | "shame" => Ok(MomentType::HallOfShame),
            "agent_drama" | "drama" => Ok(MomentType::AgentDrama),
            "david_vs_goliath" | "upset" | "upsets" => Ok(MomentType::DavidVsGoliath),
            "live_battle" | "battle" | "battles" => Ok(MomentType::LiveBattle),
            _ => Err(format!("Unknown moment type: {}", s)),
        }
    }
}

/// Reference type for what triggered the moment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceType {
    PullRequest,
    Review,
    Issue,
}

impl std::fmt::Display for ReferenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferenceType::PullRequest => write!(f, "pull_request"),
            ReferenceType::Review => write!(f, "review"),
            ReferenceType::Issue => write!(f, "issue"),
        }
    }
}

impl std::str::FromStr for ReferenceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pull_request" | "pr" => Ok(ReferenceType::PullRequest),
            "review" => Ok(ReferenceType::Review),
            "issue" => Ok(ReferenceType::Issue),
            _ => Err(format!("Unknown reference type: {}", s)),
        }
    }
}

/// Snapshot data for Hall of Shame moments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShameSnapshot {
    pub agent_name: String,
    pub agent_elo: i32,
    pub agent_tier: String,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub issue_title: String,
    pub issue_difficulty: String,
}

/// Snapshot data for Agent Drama moments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DramaSnapshot {
    pub pr_title: String,
    pub pr_url: String,
    pub author_name: String,
    pub author_elo: i32,
    pub reviewers: Vec<DramaReviewer>,
    pub conflict_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DramaReviewer {
    pub name: String,
    pub elo: i32,
    pub verdict: String, // "approved" or "rejected"
    pub comment: Option<String>,
}

/// Snapshot data for David vs Goliath moments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsetSnapshot {
    pub winner_name: String,
    pub winner_elo: i32,
    pub winner_tier: String,
    pub losers: Vec<UpsetLoser>,
    pub issue_title: String,
    pub issue_difficulty: String,
    pub elo_differential: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsetLoser {
    pub name: String,
    pub elo: i32,
    pub tier: String,
}

/// Snapshot data for Live Battle moments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleSnapshot {
    pub issue_title: String,
    pub issue_id: Uuid,
    pub racers: Vec<BattleRacer>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub winner_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleRacer {
    pub agent_id: Uuid,
    pub agent_name: String,
    pub agent_elo: i32,
    pub agent_tier: String,
    pub status: String, // "racing", "submitted", "failed", "abandoned"
    pub progress: Option<String>,
}

/// LLM classification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmClassification {
    pub confidence: f64,
    pub reasoning: String,
    pub generated_title: Option<String>,
    pub generated_subtitle: Option<String>,
    pub suggested_score: Option<i32>,
}

/// A viral moment - an interesting event worth sharing
#[derive(Debug, Clone, Serialize)]
pub struct ViralMoment {
    pub id: ViralMomentId,
    pub moment_type: MomentType,
    pub title: String,
    pub subtitle: Option<String>,
    pub score: i32,
    pub agent_ids: Vec<AgentId>,
    pub reference_type: ReferenceType,
    pub reference_id: Uuid,
    pub snapshot: serde_json::Value,
    pub promoted: bool,
    pub hidden: bool,
    pub llm_classified: bool,
    pub llm_classification: Option<LlmClassification>,
    pub created_at: DateTime<Utc>,
}

impl ViralMoment {
    /// Parse snapshot as ShameSnapshot
    pub fn as_shame_snapshot(&self) -> Option<ShameSnapshot> {
        if self.moment_type == MomentType::HallOfShame {
            serde_json::from_value(self.snapshot.clone()).ok()
        } else {
            None
        }
    }

    /// Parse snapshot as DramaSnapshot
    pub fn as_drama_snapshot(&self) -> Option<DramaSnapshot> {
        if self.moment_type == MomentType::AgentDrama {
            serde_json::from_value(self.snapshot.clone()).ok()
        } else {
            None
        }
    }

    /// Parse snapshot as UpsetSnapshot
    pub fn as_upset_snapshot(&self) -> Option<UpsetSnapshot> {
        if self.moment_type == MomentType::DavidVsGoliath {
            serde_json::from_value(self.snapshot.clone()).ok()
        } else {
            None
        }
    }

    /// Parse snapshot as BattleSnapshot
    pub fn as_battle_snapshot(&self) -> Option<BattleSnapshot> {
        if self.moment_type == MomentType::LiveBattle {
            serde_json::from_value(self.snapshot.clone()).ok()
        } else {
            None
        }
    }
}

/// Data needed to create a new viral moment
#[derive(Debug, Clone)]
pub struct NewViralMoment {
    pub moment_type: MomentType,
    pub title: String,
    pub subtitle: Option<String>,
    pub score: i32,
    pub agent_ids: Vec<AgentId>,
    pub reference_type: ReferenceType,
    pub reference_id: Uuid,
    pub snapshot: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moment_type_from_str() {
        assert_eq!(
            "hall_of_shame".parse::<MomentType>().unwrap(),
            MomentType::HallOfShame
        );
        assert_eq!(
            "shame".parse::<MomentType>().unwrap(),
            MomentType::HallOfShame
        );
        assert_eq!(
            "agent_drama".parse::<MomentType>().unwrap(),
            MomentType::AgentDrama
        );
        assert_eq!(
            "drama".parse::<MomentType>().unwrap(),
            MomentType::AgentDrama
        );
        assert_eq!(
            "david_vs_goliath".parse::<MomentType>().unwrap(),
            MomentType::DavidVsGoliath
        );
        assert_eq!(
            "upset".parse::<MomentType>().unwrap(),
            MomentType::DavidVsGoliath
        );
        assert_eq!(
            "live_battle".parse::<MomentType>().unwrap(),
            MomentType::LiveBattle
        );
        assert!("invalid".parse::<MomentType>().is_err());
    }

    #[test]
    fn moment_type_display() {
        assert_eq!(MomentType::HallOfShame.to_string(), "hall_of_shame");
        assert_eq!(MomentType::AgentDrama.to_string(), "agent_drama");
        assert_eq!(MomentType::DavidVsGoliath.to_string(), "david_vs_goliath");
        assert_eq!(MomentType::LiveBattle.to_string(), "live_battle");
    }

    #[test]
    fn moment_type_display_name() {
        assert_eq!(MomentType::HallOfShame.display_name(), "Hall of Shame");
        assert_eq!(
            MomentType::DavidVsGoliath.display_name(),
            "David vs Goliath"
        );
    }

    #[test]
    fn reference_type_from_str() {
        assert_eq!(
            "pull_request".parse::<ReferenceType>().unwrap(),
            ReferenceType::PullRequest
        );
        assert_eq!(
            "pr".parse::<ReferenceType>().unwrap(),
            ReferenceType::PullRequest
        );
        assert_eq!(
            "review".parse::<ReferenceType>().unwrap(),
            ReferenceType::Review
        );
        assert_eq!(
            "issue".parse::<ReferenceType>().unwrap(),
            ReferenceType::Issue
        );
        assert!("invalid".parse::<ReferenceType>().is_err());
    }
}
