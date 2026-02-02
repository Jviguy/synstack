//! Domain entities
//!
//! Pure domain models representing core business concepts.
//! These are separate from the SeaORM entities in the `entity` module.

pub mod agent;
pub mod agent_review;
pub mod code_contribution;
pub mod elo_event;
pub mod engagement;
pub mod issue;
pub mod project;
pub mod ticket;
pub mod viral_moment;

pub use agent::{Agent, AgentId, ClaimAgent, NewAgent, Tier};
// Re-export agent review types including threshold for domain consumers
#[allow(unused_imports)]
pub use agent_review::{
    AgentReview, AgentReviewId, NewAgentReview, ReviewVerdict, HIGH_ELO_THRESHOLD,
};
pub use code_contribution::{
    CodeContribution, CodeContributionId, ContributionStatus, NewCodeContribution,
};
pub use elo_event::{EloEvent, EloEventId, EloEventType, NewEloEvent};
pub use engagement::{
    Engagement, EngagementCounts, EngagementId, EngagementType, NewEngagement, ReactionType,
    TargetType,
};
pub use issue::{Issue, IssueComment, IssueId, IssueState, Label, NewIssue};
pub use project::{
    BuildStatus, MemberRole, NewProject, Project, ProjectId, ProjectMember, ProjectStatus,
};
pub use ticket::{NewTicket, Ticket, TicketId, TicketPriority, TicketStatus};
#[allow(unused_imports)]
pub use viral_moment::{
    BattleRacer, BattleSnapshot, DramaReviewer, DramaSnapshot, LlmClassification, MomentType,
    NewViralMoment, ReferenceType, ShameSnapshot, UpsetLoser, UpsetSnapshot, ViralMoment,
    ViralMomentId,
};
