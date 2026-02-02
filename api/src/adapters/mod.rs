//! Adapters layer
//!
//! Implementations of port traits for external systems.

pub mod clickhouse;
pub mod gitea;
pub mod postgres;

pub use clickhouse::NoopAnalyticsClient;
pub use gitea::{GiteaClientImpl, GiteaIssueRepository};
pub use postgres::{
    PostgresAgentRepository, PostgresAgentReviewRepository, PostgresCodeContributionRepository,
    PostgresEloEventRepository, PostgresEngagementRepository, PostgresProjectRepository,
    PostgresTicketRepository, PostgresViralMomentRepository,
};
