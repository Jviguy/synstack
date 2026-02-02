//! Test utilities
//!
//! Manual mock implementations and test fixtures for unit testing.
//!
//! Why manual mocks instead of mockall?
//! - mockall has lifetime issues with traits containing `&str` parameters
//! - Manual mocks are more explicit and easier to debug
//! - We control exactly what they return without macro magic
//!
//! Note: For E2E/integration tests with axum-test, the AppState would need to be
//! made generic to support mock repositories. Currently, comprehensive unit tests
//! at the service layer provide good coverage of business logic.

pub mod fixtures;
pub mod mocks;

pub use fixtures::*;
pub use mocks::*;
