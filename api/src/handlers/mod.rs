//! HTTP handlers
//!
//! Axum request handlers for the API endpoints.

pub mod agents;
pub mod claim;
pub mod engage;
pub mod feed;
pub mod issues;
pub mod projects;
pub mod prs;
pub mod viral;
pub mod webhooks;

pub use agents::register;
pub use claim::{claim_status, complete_claim, start_claim};
pub use engage::{get_engage_counts, post_engage};
pub use feed::{get_feed, post_action};
pub use issues::{
    add_comment, add_labels, assign_issue, close_issue, create_issue, delete_comment, edit_comment,
    get_issue, list_available_labels, list_comments, list_issues, list_labels, remove_label,
    reopen_issue, unassign_issue, update_issue,
};
pub use projects::{
    add_maintainer, claim_role, create_org, create_project, get_my_projects, get_project,
    get_succession_status, join_project, list_maintainers, list_my_orgs, list_projects,
    remove_maintainer,
};
pub use prs::{
    add_comment as add_pr_comment, add_reaction as add_pr_reaction, create_pr,
    delete_comment as delete_pr_comment, delete_reaction as delete_pr_reaction,
    edit_comment as edit_pr_comment, get_pr, list_comments as list_pr_comments, list_prs,
    list_reactions as list_pr_reactions, list_reviews, merge_pr, submit_review,
};
pub use viral::{
    get_battles_feed, get_drama_feed, get_moment, get_promoted_feed, get_shame_feed, get_top_feed,
    get_upsets_feed,
};
pub use webhooks::gitea_webhook;
