//! Code contribution domain entity
//!
//! Tracks merged PRs and their "afterlife" - whether code remains healthy,
//! gets reverted, or gets replaced. Used for reactive ELO calculations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent::AgentId;
use super::project::ProjectId;

/// Unique identifier for a code contribution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodeContributionId(pub Uuid);

impl CodeContributionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CodeContributionId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for CodeContributionId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for CodeContributionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a code contribution over time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContributionStatus {
    /// Code is still in the codebase and functioning
    Healthy,
    /// Code was reverted
    Reverted,
    /// Code was replaced/overwritten within 7 days
    Replaced,
}

impl std::fmt::Display for ContributionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContributionStatus::Healthy => write!(f, "healthy"),
            ContributionStatus::Reverted => write!(f, "reverted"),
            ContributionStatus::Replaced => write!(f, "replaced"),
        }
    }
}

impl std::str::FromStr for ContributionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "healthy" => Ok(ContributionStatus::Healthy),
            "reverted" => Ok(ContributionStatus::Reverted),
            "replaced" => Ok(ContributionStatus::Replaced),
            _ => Err(format!("Unknown contribution status: {}", s)),
        }
    }
}

/// A code contribution (merged PR) tracked for reactive ELO
#[derive(Debug, Clone, Serialize)]
pub struct CodeContribution {
    pub id: CodeContributionId,
    pub agent_id: AgentId,
    pub project_id: ProjectId,
    pub pr_number: i64,
    pub commit_sha: String,
    pub status: ContributionStatus,
    /// Number of bugs that reference this contribution
    pub bug_count: i32,
    /// Whether the 30-day longevity bonus has been paid
    pub longevity_bonus_paid: bool,
    /// Number of PRs that build upon this contribution
    pub dependent_prs_count: i32,
    pub merged_at: DateTime<Utc>,
    pub reverted_at: Option<DateTime<Utc>>,
    pub replaced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl CodeContribution {
    /// Check if this contribution is eligible for the longevity bonus
    /// (survived 30 days without being reverted or replaced)
    pub fn is_eligible_for_longevity_bonus(&self, now: DateTime<Utc>) -> bool {
        if self.longevity_bonus_paid {
            return false;
        }
        if self.status != ContributionStatus::Healthy {
            return false;
        }
        let days_since_merge = (now - self.merged_at).num_days();
        days_since_merge >= 30
    }

    /// Check if this contribution was replaced within the penalty window (7 days)
    pub fn was_replaced_within_window(&self) -> bool {
        if self.status != ContributionStatus::Replaced {
            return false;
        }
        if let Some(replaced_at) = self.replaced_at {
            let days_since_merge = (replaced_at - self.merged_at).num_days();
            return days_since_merge <= 7;
        }
        false
    }
}

/// Data needed to create a new code contribution
#[derive(Debug, Clone)]
pub struct NewCodeContribution {
    pub agent_id: AgentId,
    pub project_id: ProjectId,
    pub pr_number: i64,
    pub commit_sha: String,
    pub merged_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_contribution(status: ContributionStatus, merged_at: DateTime<Utc>) -> CodeContribution {
        CodeContribution {
            id: CodeContributionId::new(),
            agent_id: AgentId::new(),
            project_id: ProjectId::new(),
            pr_number: 42,
            commit_sha: "abc123def456".to_string(),
            status,
            bug_count: 0,
            longevity_bonus_paid: false,
            dependent_prs_count: 0,
            merged_at,
            reverted_at: None,
            replaced_at: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn contribution_status_display() {
        assert_eq!(ContributionStatus::Healthy.to_string(), "healthy");
        assert_eq!(ContributionStatus::Reverted.to_string(), "reverted");
        assert_eq!(ContributionStatus::Replaced.to_string(), "replaced");
    }

    #[test]
    fn contribution_status_from_str() {
        assert_eq!(
            "healthy".parse::<ContributionStatus>().unwrap(),
            ContributionStatus::Healthy
        );
        assert_eq!(
            "REVERTED".parse::<ContributionStatus>().unwrap(),
            ContributionStatus::Reverted
        );
        assert_eq!(
            "Replaced".parse::<ContributionStatus>().unwrap(),
            ContributionStatus::Replaced
        );
        assert!("invalid".parse::<ContributionStatus>().is_err());
    }

    #[test]
    fn is_eligible_for_longevity_bonus_healthy_after_30_days() {
        let merged_at = Utc::now() - Duration::days(31);
        let contrib = make_contribution(ContributionStatus::Healthy, merged_at);
        assert!(contrib.is_eligible_for_longevity_bonus(Utc::now()));
    }

    #[test]
    fn is_eligible_for_longevity_bonus_healthy_before_30_days() {
        let merged_at = Utc::now() - Duration::days(15);
        let contrib = make_contribution(ContributionStatus::Healthy, merged_at);
        assert!(!contrib.is_eligible_for_longevity_bonus(Utc::now()));
    }

    #[test]
    fn is_eligible_for_longevity_bonus_already_paid() {
        let merged_at = Utc::now() - Duration::days(31);
        let mut contrib = make_contribution(ContributionStatus::Healthy, merged_at);
        contrib.longevity_bonus_paid = true;
        assert!(!contrib.is_eligible_for_longevity_bonus(Utc::now()));
    }

    #[test]
    fn is_eligible_for_longevity_bonus_reverted() {
        let merged_at = Utc::now() - Duration::days(31);
        let contrib = make_contribution(ContributionStatus::Reverted, merged_at);
        assert!(!contrib.is_eligible_for_longevity_bonus(Utc::now()));
    }

    #[test]
    fn was_replaced_within_window_true() {
        let merged_at = Utc::now() - Duration::days(10);
        let mut contrib = make_contribution(ContributionStatus::Replaced, merged_at);
        contrib.replaced_at = Some(merged_at + Duration::days(5));
        assert!(contrib.was_replaced_within_window());
    }

    #[test]
    fn was_replaced_within_window_false_outside_window() {
        let merged_at = Utc::now() - Duration::days(20);
        let mut contrib = make_contribution(ContributionStatus::Replaced, merged_at);
        contrib.replaced_at = Some(merged_at + Duration::days(10));
        assert!(!contrib.was_replaced_within_window());
    }

    #[test]
    fn was_replaced_within_window_false_healthy() {
        let merged_at = Utc::now() - Duration::days(10);
        let contrib = make_contribution(ContributionStatus::Healthy, merged_at);
        assert!(!contrib.was_replaced_within_window());
    }

    #[test]
    fn code_contribution_id_display() {
        let id = CodeContributionId(Uuid::nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }
}
