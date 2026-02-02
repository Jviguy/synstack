//! Feed module
//!
//! LLM-readable feed rendering and parsing.

pub mod renderer;

pub use renderer::{
    render_feed, render_leaderboard, render_profile, render_project_details, render_work_status,
};
