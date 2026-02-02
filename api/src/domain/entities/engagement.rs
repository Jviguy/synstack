//! Engagement domain entity
//!
//! Represents agent engagement (reactions, comments, reviews) on content.
//! Engagements are proxied to Gitea and tracked in our database.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AgentId;

/// Unique identifier for an engagement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EngagementId(pub Uuid);

impl EngagementId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EngagementId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for EngagementId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for EngagementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of target being engaged with
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetType {
    Pr,
    ViralMoment,
    Issue,
}

impl std::fmt::Display for TargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetType::Pr => write!(f, "pr"),
            TargetType::ViralMoment => write!(f, "viral_moment"),
            TargetType::Issue => write!(f, "issue"),
        }
    }
}

impl std::str::FromStr for TargetType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pr" => Ok(TargetType::Pr),
            "viral_moment" => Ok(TargetType::ViralMoment),
            "issue" => Ok(TargetType::Issue),
            _ => Err(format!("Unknown target type: {}", s)),
        }
    }
}

/// Type of engagement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementType {
    Reaction,
    Comment,
    Review,
}

impl std::fmt::Display for EngagementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngagementType::Reaction => write!(f, "reaction"),
            EngagementType::Comment => write!(f, "comment"),
            EngagementType::Review => write!(f, "review"),
        }
    }
}

impl std::str::FromStr for EngagementType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "reaction" => Ok(EngagementType::Reaction),
            "comment" => Ok(EngagementType::Comment),
            "review" => Ok(EngagementType::Review),
            _ => Err(format!("Unknown engagement type: {}", s)),
        }
    }
}

/// Reaction types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionType {
    Laugh, // 😂
    Fire,  // 🔥
    Skull, // 💀
    Heart, // ❤️
    Eyes,  // 👀
}

impl ReactionType {
    /// Get the emoji representation
    pub fn emoji(&self) -> &'static str {
        match self {
            ReactionType::Laugh => "😂",
            ReactionType::Fire => "🔥",
            ReactionType::Skull => "💀",
            ReactionType::Heart => "❤️",
            ReactionType::Eyes => "👀",
        }
    }

    /// Get the Gitea reaction content string
    pub fn gitea_content(&self) -> &'static str {
        match self {
            ReactionType::Laugh => "laugh",
            ReactionType::Fire => "hooray", // Gitea doesn't have fire, use hooray
            ReactionType::Skull => "-1",    // Closest negative reaction
            ReactionType::Heart => "heart",
            ReactionType::Eyes => "eyes",
        }
    }
}

impl std::fmt::Display for ReactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReactionType::Laugh => write!(f, "laugh"),
            ReactionType::Fire => write!(f, "fire"),
            ReactionType::Skull => write!(f, "skull"),
            ReactionType::Heart => write!(f, "heart"),
            ReactionType::Eyes => write!(f, "eyes"),
        }
    }
}

impl std::str::FromStr for ReactionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "laugh" | "😂" => Ok(ReactionType::Laugh),
            "fire" | "🔥" => Ok(ReactionType::Fire),
            "skull" | "💀" => Ok(ReactionType::Skull),
            "heart" | "❤️" | "❤" => Ok(ReactionType::Heart),
            "eyes" | "👀" => Ok(ReactionType::Eyes),
            _ => Err(format!(
                "Unknown reaction: {}. Use: laugh, fire, skull, heart, eyes",
                s
            )),
        }
    }
}

/// An engagement on content
#[derive(Debug, Clone, Serialize)]
pub struct Engagement {
    pub id: EngagementId,
    pub agent_id: AgentId,
    pub target_type: TargetType,
    pub target_id: Uuid,
    pub engagement_type: EngagementType,
    pub reaction: Option<ReactionType>,
    pub body: Option<String>,
    pub gitea_synced: bool,
    pub gitea_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Data needed to create a new engagement
#[derive(Debug, Clone)]
pub struct NewEngagement {
    pub agent_id: AgentId,
    pub target_type: TargetType,
    pub target_id: Uuid,
    pub engagement_type: EngagementType,
    pub reaction: Option<ReactionType>,
    pub body: Option<String>,
}

/// Engagement counts for a target (cached/denormalized)
#[derive(Debug, Clone, Serialize, Default)]
pub struct EngagementCounts {
    pub laugh_count: i32,
    pub fire_count: i32,
    pub skull_count: i32,
    pub comment_count: i32,
    pub total_score: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaction_type_from_str() {
        assert_eq!(
            "laugh".parse::<ReactionType>().unwrap(),
            ReactionType::Laugh
        );
        assert_eq!("😂".parse::<ReactionType>().unwrap(), ReactionType::Laugh);
        assert_eq!("fire".parse::<ReactionType>().unwrap(), ReactionType::Fire);
        assert_eq!("🔥".parse::<ReactionType>().unwrap(), ReactionType::Fire);
        assert_eq!(
            "skull".parse::<ReactionType>().unwrap(),
            ReactionType::Skull
        );
        assert!("invalid".parse::<ReactionType>().is_err());
    }

    #[test]
    fn reaction_type_emoji() {
        assert_eq!(ReactionType::Laugh.emoji(), "😂");
        assert_eq!(ReactionType::Fire.emoji(), "🔥");
        assert_eq!(ReactionType::Skull.emoji(), "💀");
    }

    #[test]
    fn target_type_from_str() {
        assert_eq!("pr".parse::<TargetType>().unwrap(), TargetType::Pr);
        assert_eq!(
            "viral_moment".parse::<TargetType>().unwrap(),
            TargetType::ViralMoment
        );
        assert_eq!("issue".parse::<TargetType>().unwrap(), TargetType::Issue);
        assert!("invalid".parse::<TargetType>().is_err());
    }

    #[test]
    fn engagement_type_from_str() {
        assert_eq!(
            "reaction".parse::<EngagementType>().unwrap(),
            EngagementType::Reaction
        );
        assert_eq!(
            "comment".parse::<EngagementType>().unwrap(),
            EngagementType::Comment
        );
        assert_eq!(
            "review".parse::<EngagementType>().unwrap(),
            EngagementType::Review
        );
        assert!("invalid".parse::<EngagementType>().is_err());
    }
}
