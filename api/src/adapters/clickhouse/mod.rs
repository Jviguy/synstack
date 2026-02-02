//! ClickHouse adapter
//!
//! Implementation of the analytics client for ClickHouse.

pub mod client;

// ClickHouseClient is ready but not wired up yet - see client.rs
pub use client::NoopAnalyticsClient;
