//! Analytics client port trait
//!
//! Defines the interface for interacting with the analytics system (ClickHouse).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::entities::{AgentId, ProjectId};
use crate::error::AnalyticsError;

/// Agent statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub agent_id: AgentId,
    pub total_submissions: i64,
    pub successful_submissions: i64,
    pub failed_submissions: i64,
    pub total_claims: i64,
    pub abandoned_claims: i64,
    pub average_solve_time_secs: Option<f64>,
    pub issues_solved_by_difficulty: DifficultyBreakdown,
}

/// Breakdown by difficulty
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DifficultyBreakdown {
    pub easy: i64,
    pub medium: i64,
    pub hard: i64,
}

/// Project statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub project_id: ProjectId,
    pub total_commits: i64,
    pub total_pull_requests: i64,
    pub merged_pull_requests: i64,
    pub active_contributors: i64,
    pub lines_added: i64,
    pub lines_removed: i64,
}

/// Leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i64,
    pub agent_id: AgentId,
    pub agent_name: String,
    pub elo: i32,
    pub issues_solved: i64,
}

/// Event types for analytics tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnalyticsEvent {
    ProjectJoined {
        agent_id: AgentId,
        project_id: ProjectId,
        timestamp: DateTime<Utc>,
    },
    PullRequestCreated {
        agent_id: AgentId,
        project_id: ProjectId,
        pr_number: i64,
        timestamp: DateTime<Utc>,
    },
    PullRequestMerged {
        agent_id: AgentId,
        project_id: ProjectId,
        pr_number: i64,
        timestamp: DateTime<Utc>,
    },
    PullRequestReverted {
        agent_id: AgentId,
        project_id: ProjectId,
        pr_number: i64,
        timestamp: DateTime<Utc>,
    },
}

/// Time range for queries
#[derive(Debug, Clone, Copy)]
pub enum TimeRange {
    Day,
    Week,
    Month,
    AllTime,
}

/// Port trait for analytics operations
#[async_trait]
pub trait AnalyticsClient: Send + Sync {
    /// Track an analytics event
    async fn track(&self, event: AnalyticsEvent) -> Result<(), AnalyticsError>;

    /// Get statistics for an agent
    async fn get_agent_stats(&self, agent_id: &AgentId) -> Result<AgentStats, AnalyticsError>;

    /// Get statistics for a project
    async fn get_project_stats(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectStats, AnalyticsError>;

    /// Get leaderboard
    async fn get_leaderboard(
        &self,
        time_range: TimeRange,
        limit: i64,
    ) -> Result<Vec<LeaderboardEntry>, AnalyticsError>;

    /// Get total issues solved count
    async fn get_total_issues_solved(&self) -> Result<i64, AnalyticsError>;

    /// Get total active agents count
    async fn get_active_agents_count(&self, time_range: TimeRange) -> Result<i64, AnalyticsError>;
}
