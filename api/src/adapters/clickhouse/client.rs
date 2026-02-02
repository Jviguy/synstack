//! ClickHouse analytics client implementation
//!
//! The ClickHouseClient is ready to use but not wired up yet.
//! Currently using NoopAnalyticsClient until ClickHouse is deployed.

use async_trait::async_trait;
use reqwest::Client;

use crate::domain::entities::{AgentId, ProjectId};
use crate::domain::ports::{
    AgentStats, AnalyticsClient, AnalyticsEvent, DifficultyBreakdown, LeaderboardEntry,
    ProjectStats, TimeRange,
};
use crate::error::AnalyticsError;

/// Implementation of the ClickHouse analytics client
///
/// Currently unused - will be enabled when ClickHouse is deployed.
#[allow(dead_code)]
pub struct ClickHouseClient {
    http: Client,
    base_url: String,
}

#[allow(dead_code)]
impl ClickHouseClient {
    pub fn new(base_url: String) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn execute_query(&self, query: &str) -> Result<String, AnalyticsError> {
        let resp = self
            .http
            .post(&self.base_url)
            .body(query.to_string())
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.text().await.unwrap_or_default())
        } else {
            let msg = resp.text().await.unwrap_or_default();
            Err(AnalyticsError::Query(msg))
        }
    }

    fn time_range_to_interval(range: TimeRange) -> &'static str {
        match range {
            TimeRange::Day => "1 DAY",
            TimeRange::Week => "7 DAY",
            TimeRange::Month => "30 DAY",
            TimeRange::AllTime => "100 YEAR",
        }
    }
}

#[async_trait]
impl AnalyticsClient for ClickHouseClient {
    async fn track(&self, event: AnalyticsEvent) -> Result<(), AnalyticsError> {
        let (event_type, agent_id, timestamp) = match &event {
            AnalyticsEvent::ProjectJoined {
                agent_id,
                timestamp,
                ..
            } => ("project_joined", agent_id.0, *timestamp),
            AnalyticsEvent::PullRequestCreated {
                agent_id,
                timestamp,
                ..
            } => ("pr_created", agent_id.0, *timestamp),
            AnalyticsEvent::PullRequestMerged {
                agent_id,
                timestamp,
                ..
            } => ("pr_merged", agent_id.0, *timestamp),
            AnalyticsEvent::PullRequestReverted {
                agent_id,
                timestamp,
                ..
            } => ("pr_reverted", agent_id.0, *timestamp),
        };

        let event_json =
            serde_json::to_string(&event).map_err(|e| AnalyticsError::Query(e.to_string()))?;

        let query = format!(
            "INSERT INTO events (event_type, agent_id, event_data, timestamp) VALUES ('{}', '{}', '{}', '{}')",
            event_type,
            agent_id,
            event_json.replace('\'', "''"),
            timestamp.format("%Y-%m-%d %H:%M:%S")
        );

        self.execute_query(&query).await?;
        Ok(())
    }

    async fn get_agent_stats(&self, agent_id: &AgentId) -> Result<AgentStats, AnalyticsError> {
        // For now, return default stats
        // In production, this would query ClickHouse
        Ok(AgentStats {
            agent_id: *agent_id,
            total_submissions: 0,
            successful_submissions: 0,
            failed_submissions: 0,
            total_claims: 0,
            abandoned_claims: 0,
            average_solve_time_secs: None,
            issues_solved_by_difficulty: DifficultyBreakdown::default(),
        })
    }

    async fn get_project_stats(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectStats, AnalyticsError> {
        // For now, return default stats
        Ok(ProjectStats {
            project_id: *project_id,
            total_commits: 0,
            total_pull_requests: 0,
            merged_pull_requests: 0,
            active_contributors: 0,
            lines_added: 0,
            lines_removed: 0,
        })
    }

    async fn get_leaderboard(
        &self,
        _time_range: TimeRange,
        _limit: i64,
    ) -> Result<Vec<LeaderboardEntry>, AnalyticsError> {
        // For now, return empty leaderboard
        // In production, this would query ClickHouse and join with PostgreSQL
        Ok(Vec::new())
    }

    async fn get_total_issues_solved(&self) -> Result<i64, AnalyticsError> {
        Ok(0)
    }

    async fn get_active_agents_count(&self, _time_range: TimeRange) -> Result<i64, AnalyticsError> {
        Ok(0)
    }
}

/// A no-op analytics client for testing or when ClickHouse is not available
pub struct NoopAnalyticsClient;

#[async_trait]
impl AnalyticsClient for NoopAnalyticsClient {
    async fn track(&self, _event: AnalyticsEvent) -> Result<(), AnalyticsError> {
        Ok(())
    }

    async fn get_agent_stats(&self, agent_id: &AgentId) -> Result<AgentStats, AnalyticsError> {
        Ok(AgentStats {
            agent_id: *agent_id,
            total_submissions: 0,
            successful_submissions: 0,
            failed_submissions: 0,
            total_claims: 0,
            abandoned_claims: 0,
            average_solve_time_secs: None,
            issues_solved_by_difficulty: DifficultyBreakdown::default(),
        })
    }

    async fn get_project_stats(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectStats, AnalyticsError> {
        Ok(ProjectStats {
            project_id: *project_id,
            total_commits: 0,
            total_pull_requests: 0,
            merged_pull_requests: 0,
            active_contributors: 0,
            lines_added: 0,
            lines_removed: 0,
        })
    }

    async fn get_leaderboard(
        &self,
        _time_range: TimeRange,
        _limit: i64,
    ) -> Result<Vec<LeaderboardEntry>, AnalyticsError> {
        Ok(Vec::new())
    }

    async fn get_total_issues_solved(&self) -> Result<i64, AnalyticsError> {
        Ok(0)
    }

    async fn get_active_agents_count(&self, _time_range: TimeRange) -> Result<i64, AnalyticsError> {
        Ok(0)
    }
}
