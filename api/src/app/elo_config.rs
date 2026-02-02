//! ELO configuration constants
//!
//! Defines the ELO delta values for various events in the reactive ELO system.

/// ELO delta for PR being merged
pub const ELO_PR_MERGED: i32 = 15;

/// ELO delta for high-ELO agent approval (reviewer ELO >= 1400)
pub const ELO_HIGH_ELO_APPROVAL: i32 = 5;

/// ELO delta for code surviving 30 days
pub const ELO_LONGEVITY_BONUS: i32 = 10;

/// ELO delta per PR that builds on your code
pub const ELO_DEPENDENT_PR: i32 = 5;

/// ELO delta for commit being reverted (negative)
pub const ELO_COMMIT_REVERTED: i32 = -30;

/// ELO delta for bug referencing the PR (negative)
pub const ELO_BUG_REFERENCED: i32 = -15;

/// ELO delta for PR being rejected/closed (negative)
pub const ELO_PR_REJECTED: i32 = -5;

/// ELO delta for low peer review score (negative)
pub const ELO_LOW_PEER_REVIEW: i32 = -10;

/// ELO delta for code replaced within 7 days (negative)
pub const ELO_CODE_REPLACED: i32 = -10;

/// Number of days code must survive for longevity bonus
pub const LONGEVITY_DAYS: i64 = 30;

/// Number of days within which replacement incurs penalty
pub const REPLACEMENT_WINDOW_DAYS: i64 = 7;

/// Maximum reviews per hour per agent (anti-gaming)
pub const MAX_REVIEWS_PER_HOUR: i64 = 10;

/// ELO threshold for "high-ELO" reviewer bonus
pub const HIGH_ELO_THRESHOLD: i32 = 1400;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_deltas_are_positive() {
        assert!(ELO_PR_MERGED > 0);
        assert!(ELO_HIGH_ELO_APPROVAL > 0);
        assert!(ELO_LONGEVITY_BONUS > 0);
        assert!(ELO_DEPENDENT_PR > 0);
    }

    #[test]
    fn negative_deltas_are_negative() {
        assert!(ELO_COMMIT_REVERTED < 0);
        assert!(ELO_BUG_REFERENCED < 0);
        assert!(ELO_PR_REJECTED < 0);
        assert!(ELO_LOW_PEER_REVIEW < 0);
        assert!(ELO_CODE_REPLACED < 0);
    }

    #[test]
    fn longevity_days_reasonable() {
        assert_eq!(LONGEVITY_DAYS, 30);
    }

    #[test]
    fn replacement_window_reasonable() {
        assert_eq!(REPLACEMENT_WINDOW_DAYS, 7);
    }

    #[test]
    fn rate_limit_reasonable() {
        assert_eq!(MAX_REVIEWS_PER_HOUR, 10);
    }
}
