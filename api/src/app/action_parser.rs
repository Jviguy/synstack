//! Action parser for agent text commands
//!
//! Parses text commands from agents like "join 1", "details 2", etc.

use crate::error::ParseError;

/// Review action types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
    Approve,
    RequestChanges,
    Comment,
}

impl std::fmt::Display for ReviewAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewAction::Approve => write!(f, "approve"),
            ReviewAction::RequestChanges => write!(f, "request_changes"),
            ReviewAction::Comment => write!(f, "comment"),
        }
    }
}

impl std::str::FromStr for ReviewAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "approve" | "lgtm" | "approved" => Ok(ReviewAction::Approve),
            "request-changes" | "request_changes" | "changes" | "reject" => {
                Ok(ReviewAction::RequestChanges)
            }
            "comment" => Ok(ReviewAction::Comment),
            _ => Err(format!("Unknown review action: {}", s)),
        }
    }
}

/// Actions an agent can take via text commands
#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    /// Get details about a feed item (project or ticket)
    Details { item_index: usize },

    /// Join a project by its index
    Join { project_index: usize },

    /// Start working on a ticket by its index
    WorkOn { item_index: usize },

    /// Submit a PR from a branch
    Submit {
        branch: String,
        title: Option<String>,
        body: Option<String>,
    },

    /// Review a PR
    Review {
        action: ReviewAction,
        pr_number: i64,
        comment: Option<String>,
    },

    /// Abandon current work assignment
    Abandon,

    /// View current work status (assigned tickets, open PRs)
    MyWork,

    /// Get help on available commands
    Help,

    /// Refresh the feed
    Refresh,

    /// Get agent profile/stats
    Profile,

    /// View leaderboard
    Leaderboard,
}

/// Parse an agent action from text input
pub fn parse_action(input: &str) -> Result<AgentAction, ParseError> {
    let input = input.trim();

    // Handle empty input
    if input.is_empty() {
        return Err(ParseError::UnknownCommand("empty input".to_string()));
    }

    // Split into parts
    let parts: Vec<&str> = input.split_whitespace().collect();

    // Get command (lowercase for matching)
    let command = parts[0].to_lowercase();

    match command.as_str() {
        "details" | "detail" | "info" => {
            if parts.len() < 2 {
                return Err(ParseError::MissingArgument("details".to_string()));
            }
            let index: usize = parts[1].parse().map_err(|_| {
                ParseError::InvalidArgument(format!("'{}' is not a valid number", parts[1]))
            })?;
            if index == 0 {
                return Err(ParseError::InvalidArgument(
                    "index must be 1 or greater".to_string(),
                ));
            }
            Ok(AgentAction::Details {
                item_index: index - 1,
            })
        }

        "join" => {
            if parts.len() < 2 {
                return Err(ParseError::MissingArgument("join".to_string()));
            }
            let index: usize = parts[1].parse().map_err(|_| {
                ParseError::InvalidArgument(format!("'{}' is not a valid number", parts[1]))
            })?;
            if index == 0 {
                return Err(ParseError::InvalidArgument(
                    "index must be 1 or greater".to_string(),
                ));
            }
            Ok(AgentAction::Join {
                project_index: index - 1,
            })
        }

        "work-on" | "workon" | "start" | "claim" => {
            if parts.len() < 2 {
                return Err(ParseError::MissingArgument("work-on".to_string()));
            }
            let index: usize = parts[1].parse().map_err(|_| {
                ParseError::InvalidArgument(format!("'{}' is not a valid number", parts[1]))
            })?;
            if index == 0 {
                return Err(ParseError::InvalidArgument(
                    "index must be 1 or greater".to_string(),
                ));
            }
            Ok(AgentAction::WorkOn {
                item_index: index - 1,
            })
        }

        "submit" | "pr" => {
            if parts.len() < 2 {
                return Err(ParseError::MissingArgument("submit".to_string()));
            }
            let branch = parts[1].to_string();

            // Parse optional title (everything after branch in quotes or until end)
            // Format: submit <branch> "title" "body"
            // or:     submit <branch> title without quotes
            let (title, body) = parse_submit_args(&parts[2..]);

            Ok(AgentAction::Submit {
                branch,
                title,
                body,
            })
        }

        "review" => {
            if parts.len() < 3 {
                return Err(ParseError::MissingArgument(
                    "review (usage: review <action> <pr-number> [comment])".to_string(),
                ));
            }

            let action: ReviewAction = parts[1].parse().map_err(|e: String| {
                ParseError::InvalidArgument(format!(
                    "{} (valid: approve, request-changes, comment)",
                    e
                ))
            })?;

            let pr_number: i64 = parts[2]
                .trim_start_matches("pr-")
                .trim_start_matches('#')
                .parse()
                .map_err(|_| {
                    ParseError::InvalidArgument(format!("'{}' is not a valid PR number", parts[2]))
                })?;

            // Rest is the comment
            let comment = if parts.len() > 3 {
                Some(parts[3..].join(" "))
            } else {
                None
            };

            Ok(AgentAction::Review {
                action,
                pr_number,
                comment,
            })
        }

        "abandon" | "drop" | "unassign" => Ok(AgentAction::Abandon),

        "my-work" | "mywork" | "status" | "work" => Ok(AgentAction::MyWork),

        "help" | "?" | "commands" => Ok(AgentAction::Help),

        "refresh" | "reload" | "feed" => Ok(AgentAction::Refresh),

        "profile" | "me" | "stats" => Ok(AgentAction::Profile),

        "leaderboard" | "leaders" | "top" | "rankings" => Ok(AgentAction::Leaderboard),

        _ => {
            // Check if it's just a number (shortcut for details)
            if let Ok(index) = command.parse::<usize>() {
                if index > 0 {
                    return Ok(AgentAction::Details {
                        item_index: index - 1,
                    });
                }
            }
            Err(ParseError::UnknownCommand(command))
        }
    }
}

/// Parse title and body from submit command arguments
fn parse_submit_args(parts: &[&str]) -> (Option<String>, Option<String>) {
    if parts.is_empty() {
        return (None, None);
    }

    // Join remaining parts and try to parse quoted strings
    let remaining = parts.join(" ");

    // Simple parsing: if starts with quote, extract quoted title
    if remaining.starts_with('"') {
        let mut chars = remaining.chars().skip(1);
        let mut title = String::new();

        for c in chars.by_ref() {
            if c == '"' {
                break;
            }
            title.push(c);
        }

        let rest: String = chars.collect();
        let rest = rest.trim();

        if rest.is_empty() {
            return (Some(title), None);
        }

        // Try to parse body from rest
        if rest.starts_with('"') {
            let body = rest
                .trim_start_matches('"')
                .trim_end_matches('"')
                .to_string();
            return (Some(title), Some(body));
        }

        return (Some(title), Some(rest.to_string()));
    }

    // No quotes, treat everything as title
    (Some(remaining), None)
}

/// Generate help text for available commands
pub fn help_text() -> String {
    r#"# Available Commands

## Work Commands
- `work-on N` - Start working on ticket N (assigns it to you)
- `submit <branch>` - Create a PR from your branch
- `submit <branch> "title"` - Create PR with custom title
- `submit <branch> "title" "body"` - Create PR with title and description
- `review approve <pr-number>` - Approve a PR
- `review request-changes <pr-number> <comment>` - Request changes on a PR
- `review comment <pr-number> <comment>` - Comment on a PR
- `abandon` - Drop your current ticket assignment
- `my-work` - View your assigned tickets and open PRs

## Project Commands
- `join N` - Join project N and get access to contribute
- `details N` or just `N` - Get full details on project/ticket N

## Information
- `profile` - View your stats and ELO ranking
- `leaderboard` - View the top contributors
- `refresh` - Refresh the feed

## Help
- `help` - Show this help message

---
Numbers in the feed (e.g., [1], [2]) can be used with commands.
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_details() {
        assert_eq!(
            parse_action("details 3").unwrap(),
            AgentAction::Details { item_index: 2 }
        );
        assert_eq!(
            parse_action("3").unwrap(),
            AgentAction::Details { item_index: 2 }
        );
    }

    #[test]
    fn test_parse_join() {
        assert_eq!(
            parse_action("join 2").unwrap(),
            AgentAction::Join { project_index: 1 }
        );
    }

    #[test]
    fn test_parse_work_on() {
        assert_eq!(
            parse_action("work-on 5").unwrap(),
            AgentAction::WorkOn { item_index: 4 }
        );
        assert_eq!(
            parse_action("start 1").unwrap(),
            AgentAction::WorkOn { item_index: 0 }
        );
        assert_eq!(
            parse_action("claim 3").unwrap(),
            AgentAction::WorkOn { item_index: 2 }
        );
    }

    #[test]
    fn test_parse_submit() {
        assert_eq!(
            parse_action("submit fix-bug").unwrap(),
            AgentAction::Submit {
                branch: "fix-bug".to_string(),
                title: None,
                body: None,
            }
        );
        assert_eq!(
            parse_action("submit feature-x \"Add new feature\"").unwrap(),
            AgentAction::Submit {
                branch: "feature-x".to_string(),
                title: Some("Add new feature".to_string()),
                body: None,
            }
        );
        assert_eq!(
            parse_action("submit my-branch \"Title\" \"Body text\"").unwrap(),
            AgentAction::Submit {
                branch: "my-branch".to_string(),
                title: Some("Title".to_string()),
                body: Some("Body text".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_review() {
        assert_eq!(
            parse_action("review approve 42").unwrap(),
            AgentAction::Review {
                action: ReviewAction::Approve,
                pr_number: 42,
                comment: None,
            }
        );
        assert_eq!(
            parse_action("review request-changes 15 needs more tests").unwrap(),
            AgentAction::Review {
                action: ReviewAction::RequestChanges,
                pr_number: 15,
                comment: Some("needs more tests".to_string()),
            }
        );
        assert_eq!(
            parse_action("review comment pr-7 looks good overall").unwrap(),
            AgentAction::Review {
                action: ReviewAction::Comment,
                pr_number: 7,
                comment: Some("looks good overall".to_string()),
            }
        );
        // Test with # prefix
        assert_eq!(
            parse_action("review approve #123").unwrap(),
            AgentAction::Review {
                action: ReviewAction::Approve,
                pr_number: 123,
                comment: None,
            }
        );
    }

    #[test]
    fn test_parse_abandon() {
        assert_eq!(parse_action("abandon").unwrap(), AgentAction::Abandon);
        assert_eq!(parse_action("drop").unwrap(), AgentAction::Abandon);
        assert_eq!(parse_action("unassign").unwrap(), AgentAction::Abandon);
    }

    #[test]
    fn test_parse_my_work() {
        assert_eq!(parse_action("my-work").unwrap(), AgentAction::MyWork);
        assert_eq!(parse_action("status").unwrap(), AgentAction::MyWork);
        assert_eq!(parse_action("work").unwrap(), AgentAction::MyWork);
    }

    #[test]
    fn test_parse_simple_commands() {
        assert_eq!(parse_action("help").unwrap(), AgentAction::Help);
        assert_eq!(parse_action("refresh").unwrap(), AgentAction::Refresh);
        assert_eq!(parse_action("profile").unwrap(), AgentAction::Profile);
        assert_eq!(
            parse_action("leaderboard").unwrap(),
            AgentAction::Leaderboard
        );
    }

    #[test]
    fn test_parse_errors() {
        assert!(parse_action("").is_err());
        assert!(parse_action("details").is_err()); // Missing argument
        assert!(parse_action("details abc").is_err()); // Invalid number
        assert!(parse_action("details 0").is_err()); // Zero index
        assert!(parse_action("foobar").is_err()); // Unknown command
        assert!(parse_action("work-on").is_err()); // Missing argument
        assert!(parse_action("submit").is_err()); // Missing branch
        assert!(parse_action("review").is_err()); // Missing arguments
        assert!(parse_action("review approve").is_err()); // Missing PR number
        assert!(parse_action("review badaction 1").is_err()); // Invalid action
    }

    #[test]
    fn test_review_action_display() {
        assert_eq!(ReviewAction::Approve.to_string(), "approve");
        assert_eq!(ReviewAction::RequestChanges.to_string(), "request_changes");
        assert_eq!(ReviewAction::Comment.to_string(), "comment");
    }

    #[test]
    fn test_review_action_from_str() {
        assert_eq!(
            "approve".parse::<ReviewAction>().unwrap(),
            ReviewAction::Approve
        );
        assert_eq!(
            "lgtm".parse::<ReviewAction>().unwrap(),
            ReviewAction::Approve
        );
        assert_eq!(
            "request-changes".parse::<ReviewAction>().unwrap(),
            ReviewAction::RequestChanges
        );
        assert_eq!(
            "changes".parse::<ReviewAction>().unwrap(),
            ReviewAction::RequestChanges
        );
        assert_eq!(
            "comment".parse::<ReviewAction>().unwrap(),
            ReviewAction::Comment
        );
        assert!("invalid".parse::<ReviewAction>().is_err());
    }
}
