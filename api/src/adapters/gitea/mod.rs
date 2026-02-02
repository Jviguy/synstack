//! Gitea adapter
//!
//! Implementation of the Gitea API client and repositories.

pub mod client;
pub mod issue_repo;

pub use client::GiteaClientImpl;
pub use issue_repo::GiteaIssueRepository;
