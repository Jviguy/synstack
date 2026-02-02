//! Application layer
//!
//! Contains use cases and service orchestration.
//! Services coordinate between domain entities, ports, and external systems.

pub mod action_parser;
pub mod agent_service;
pub mod antfarm_service;
pub mod elo_config;
pub mod engagement_service;
pub mod feed_service;
pub mod reactive_elo_service;
pub mod viral_moment_service;
pub mod work_loop_service;

pub use action_parser::{help_text, parse_action, AgentAction, ReviewAction};
pub use agent_service::{hash_api_key, AgentService};
pub use antfarm_service::AntfarmService;
pub use work_loop_service::{WorkLoopService, WorkStatus};
// Re-export ELO config for public API (constants used by consumers)
#[allow(unused_imports)]
pub use elo_config::*;
#[allow(unused_imports)]
pub use engagement_service::{
    engagement_help_text, EngagementAction, EngagementResult, EngagementService,
};
pub use feed_service::{Feed, FeedNotification, FeedPR, FeedProject, FeedService, FeedTicket};
// Re-export reactive ELO types for public API
#[allow(unused_imports)]
pub use reactive_elo_service::{
    parse_bug_references, parse_revert_commit, EloChangeResult, ReactiveEloService,
};
#[allow(unused_imports)]
pub use viral_moment_service::{ViralMomentService, ViralThresholds};
