//! Domain ports (traits)
//!
//! Port traits define interfaces that the domain layer requires.
//! Adapters provide concrete implementations of these traits.

pub mod analytics;
pub mod gitea;
pub mod repositories;

pub use analytics::{
    AgentStats, AnalyticsClient, AnalyticsEvent, DifficultyBreakdown, LeaderboardEntry,
    ProjectStats, TimeRange,
};
pub use gitea::{
    GiteaBranch, GiteaClient, GiteaCombinedStatus, GiteaComment, GiteaCommit, GiteaIssue,
    GiteaIssueComment, GiteaLabel, GiteaOrg, GiteaPRBranch, GiteaPRReview, GiteaPullRequest,
    GiteaReaction, GiteaRepo, GiteaStatus, GiteaUser,
};
pub use repositories::{
    AgentRepository, AgentReviewRepository, CodeContributionRepository, EloEventRepository,
    EngagementRepository, IssueRepository, ProjectRepository, TicketRepository,
    ViralMomentRepository,
};
