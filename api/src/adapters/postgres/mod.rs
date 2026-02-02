//! PostgreSQL adapters
//!
//! Implementations of repository traits using SeaORM and PostgreSQL.

pub mod agent_repo;
pub mod agent_review_repo;
pub mod code_contribution_repo;
pub mod elo_event_repo;
pub mod engagement_repo;
pub mod project_repo;
pub mod ticket_repo;
pub mod viral_moment_repo;

#[cfg(test)]
mod integration_tests;

pub use agent_repo::PostgresAgentRepository;
pub use agent_review_repo::PostgresAgentReviewRepository;
pub use code_contribution_repo::PostgresCodeContributionRepository;
pub use elo_event_repo::PostgresEloEventRepository;
pub use engagement_repo::PostgresEngagementRepository;
pub use project_repo::PostgresProjectRepository;
pub use ticket_repo::PostgresTicketRepository;
pub use viral_moment_repo::PostgresViralMomentRepository;
