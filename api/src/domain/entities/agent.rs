//! Agent domain entity
//!
//! Represents an AI agent that participates in the SynStack platform.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub Uuid);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for AgentId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Agent tier based on performance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Bronze,
    Silver,
    Gold,
}

impl Tier {
    /// Get tier from ELO rating
    pub fn from_elo(elo: i32) -> Self {
        match elo {
            ..=1199 => Tier::Bronze,
            1200..=1599 => Tier::Silver,
            _ => Tier::Gold,
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tier::Bronze => write!(f, "bronze"),
            Tier::Silver => write!(f, "silver"),
            Tier::Gold => write!(f, "gold"),
        }
    }
}

impl std::str::FromStr for Tier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bronze" => Ok(Tier::Bronze),
            "silver" => Ok(Tier::Silver),
            "gold" => Ok(Tier::Gold),
            _ => Err(format!("Unknown tier: {}", s)),
        }
    }
}

/// An AI agent that can contribute to projects
#[derive(Debug, Clone, Serialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub api_key_hash: String,
    pub gitea_username: String,
    pub elo: i32,
    pub tier: Tier,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    /// Claim code for human verification (only shown once at registration)
    #[serde(skip_serializing)]
    pub claim_code: Option<String>,
    /// When the agent was claimed by a human
    pub claimed_at: Option<DateTime<Utc>>,
    /// GitHub user ID of the claiming human
    pub github_id: Option<i64>,
    /// GitHub username of the claiming human
    pub github_username: Option<String>,
    /// GitHub avatar URL
    pub github_avatar_url: Option<String>,
}

impl Agent {
    /// Update tier based on current ELO rating
    pub fn update_tier(&mut self) {
        self.tier = Tier::from_elo(self.elo);
    }

    /// Check if this agent has been claimed by a human
    pub fn is_claimed(&self) -> bool {
        self.claimed_at.is_some()
    }
}

/// Data needed to create a new agent
#[derive(Debug, Clone)]
pub struct NewAgent {
    pub name: String,
    pub api_key_hash: String,
    pub gitea_username: String,
    pub gitea_token_encrypted: Vec<u8>,
    pub claim_code: String,
}

/// Data for claiming an agent via GitHub OAuth
#[derive(Debug, Clone)]
pub struct ClaimAgent {
    pub github_id: i64,
    pub github_username: String,
    pub github_avatar_url: Option<String>,
}

/// Agent with decrypted Gitea token (for internal use only)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AgentWithToken {
    pub agent: Agent,
    pub gitea_token: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_from_elo_bronze() {
        assert_eq!(Tier::from_elo(0), Tier::Bronze);
        assert_eq!(Tier::from_elo(500), Tier::Bronze);
        assert_eq!(Tier::from_elo(1000), Tier::Bronze);
        assert_eq!(Tier::from_elo(1199), Tier::Bronze);
    }

    #[test]
    fn tier_from_elo_silver() {
        assert_eq!(Tier::from_elo(1200), Tier::Silver);
        assert_eq!(Tier::from_elo(1400), Tier::Silver);
        assert_eq!(Tier::from_elo(1599), Tier::Silver);
    }

    #[test]
    fn tier_from_elo_gold() {
        assert_eq!(Tier::from_elo(1600), Tier::Gold);
        assert_eq!(Tier::from_elo(1800), Tier::Gold);
        assert_eq!(Tier::from_elo(2000), Tier::Gold);
        assert_eq!(Tier::from_elo(3000), Tier::Gold);
    }

    #[test]
    fn tier_from_elo_negative() {
        // Edge case: negative ELO should still be Bronze
        assert_eq!(Tier::from_elo(-100), Tier::Bronze);
    }

    #[test]
    fn tier_display() {
        assert_eq!(Tier::Bronze.to_string(), "bronze");
        assert_eq!(Tier::Silver.to_string(), "silver");
        assert_eq!(Tier::Gold.to_string(), "gold");
    }

    #[test]
    fn tier_from_str() {
        assert_eq!("bronze".parse::<Tier>().unwrap(), Tier::Bronze);
        assert_eq!("SILVER".parse::<Tier>().unwrap(), Tier::Silver);
        assert_eq!("Gold".parse::<Tier>().unwrap(), Tier::Gold);
        assert!("invalid".parse::<Tier>().is_err());
    }

    #[test]
    fn agent_update_tier() {
        let mut agent = Agent {
            id: AgentId::new(),
            name: "test".to_string(),
            api_key_hash: "hash".to_string(),
            gitea_username: "test".to_string(),
            elo: 1500,
            tier: Tier::Bronze,
            created_at: Utc::now(),
            last_seen_at: None,
            claim_code: None,
            claimed_at: None,
            github_id: None,
            github_username: None,
            github_avatar_url: None,
        };

        agent.update_tier();

        assert_eq!(agent.tier, Tier::Silver);
    }

    #[test]
    fn agent_is_claimed() {
        let mut agent = Agent {
            id: AgentId::new(),
            name: "test".to_string(),
            api_key_hash: "hash".to_string(),
            gitea_username: "test".to_string(),
            elo: 1000,
            tier: Tier::Bronze,
            created_at: Utc::now(),
            last_seen_at: None,
            claim_code: Some("abc123".to_string()),
            claimed_at: None,
            github_id: None,
            github_username: None,
            github_avatar_url: None,
        };

        assert!(!agent.is_claimed());

        agent.claimed_at = Some(Utc::now());
        agent.github_id = Some(12345);
        agent.github_username = Some("testuser".to_string());

        assert!(agent.is_claimed());
    }

    #[test]
    fn agent_id_display() {
        let id = AgentId(Uuid::nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }
}
