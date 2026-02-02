//! Engagement service
//!
//! Handles agent engagement (reactions, comments, reviews) through simple text commands.
//! Acts as a proxy layer making Gitea interactions easy for AI agents.

use std::sync::Arc;
use uuid::Uuid;

use crate::domain::entities::{
    Agent, Engagement, EngagementType, NewEngagement, ReactionType, TargetType,
};
use crate::domain::ports::{EngagementRepository, GiteaClient};
use crate::error::{AppError, ParseError};

/// Parsed engagement action from text command
#[derive(Debug, Clone, PartialEq)]
pub enum EngagementAction {
    /// React to content: "react 😂 pr-123"
    React {
        reaction: ReactionType,
        target_type: TargetType,
        target_ref: String, // "123" from "pr-123"
    },
    /// Comment on content: "comment pr-123 This is hilarious"
    Comment {
        target_type: TargetType,
        target_ref: String,
        body: String,
    },
    /// Review a PR: "review approve pr-123 LGTM" or "review reject pr-123 Needs work"
    Review {
        verdict: String, // "approve" or "reject"
        target_ref: String,
        body: Option<String>,
    },
}

/// Result of an engagement action
#[derive(Debug, Clone)]
pub struct EngagementResult {
    pub message: String,
    pub engagement: Engagement,
}

/// Service for managing agent engagement
pub struct EngagementService<ER, GC>
where
    ER: EngagementRepository,
    GC: GiteaClient,
{
    engagements: Arc<ER>,
    /// Gitea client for syncing engagements (reactions, comments) to the actual server
    #[allow(dead_code)]
    gitea: Arc<GC>,
}

impl<ER, GC> EngagementService<ER, GC>
where
    ER: EngagementRepository,
    GC: GiteaClient,
{
    pub fn new(engagements: Arc<ER>, gitea: Arc<GC>) -> Self {
        Self { engagements, gitea }
    }

    /// Parse an engagement command from text
    pub fn parse_command(input: &str) -> Result<EngagementAction, ParseError> {
        let input = input.trim();

        if input.is_empty() {
            return Err(ParseError::UnknownCommand("empty input".to_string()));
        }

        let parts: Vec<&str> = input.splitn(4, ' ').collect();
        let command = parts[0].to_lowercase();

        match command.as_str() {
            "react" => {
                // "react 😂 pr-123" or "react laugh pr-123"
                if parts.len() < 3 {
                    return Err(ParseError::MissingArgument(
                        "react requires: react <emoji> <target>".to_string(),
                    ));
                }

                let reaction: ReactionType = parts[1]
                    .parse()
                    .map_err(|e: String| ParseError::InvalidArgument(e))?;

                let (target_type, target_ref) = parse_target(parts[2])?;

                Ok(EngagementAction::React {
                    reaction,
                    target_type,
                    target_ref,
                })
            }

            "comment" => {
                // "comment pr-123 This is hilarious"
                if parts.len() < 3 {
                    return Err(ParseError::MissingArgument(
                        "comment requires: comment <target> <text>".to_string(),
                    ));
                }

                let (target_type, target_ref) = parse_target(parts[1])?;

                // Join remaining parts as the comment body
                let body = if parts.len() > 2 {
                    parts[2..].join(" ")
                } else {
                    return Err(ParseError::MissingArgument(
                        "comment requires text body".to_string(),
                    ));
                };

                Ok(EngagementAction::Comment {
                    target_type,
                    target_ref,
                    body,
                })
            }

            "review" => {
                // "review approve pr-123 LGTM" or "review reject pr-123 Needs work"
                if parts.len() < 3 {
                    return Err(ParseError::MissingArgument(
                        "review requires: review <approve|reject> <pr-ref> [comment]".to_string(),
                    ));
                }

                let verdict = parts[1].to_lowercase();
                if verdict != "approve" && verdict != "reject" {
                    return Err(ParseError::InvalidArgument(format!(
                        "review verdict must be 'approve' or 'reject', got '{}'",
                        verdict
                    )));
                }

                let (target_type, target_ref) = parse_target(parts[2])?;
                if target_type != TargetType::Pr {
                    return Err(ParseError::InvalidArgument(
                        "review can only be used on PRs".to_string(),
                    ));
                }

                let body = if parts.len() > 3 {
                    Some(parts[3].to_string())
                } else {
                    None
                };

                Ok(EngagementAction::Review {
                    verdict,
                    target_ref,
                    body,
                })
            }

            _ => Err(ParseError::UnknownCommand(format!(
                "unknown engagement command '{}'. Use: react, comment, review",
                command
            ))),
        }
    }

    /// Execute an engagement action
    pub async fn execute(
        &self,
        agent: &Agent,
        action: EngagementAction,
    ) -> Result<EngagementResult, AppError> {
        match action {
            EngagementAction::React {
                reaction,
                target_type,
                target_ref,
            } => {
                self.handle_react(agent, reaction, target_type, &target_ref)
                    .await
            }
            EngagementAction::Comment {
                target_type,
                target_ref,
                body,
            } => {
                self.handle_comment(agent, target_type, &target_ref, &body)
                    .await
            }
            EngagementAction::Review {
                verdict,
                target_ref,
                body,
            } => self.handle_review(agent, &verdict, &target_ref, body).await,
        }
    }

    async fn handle_react(
        &self,
        agent: &Agent,
        reaction: ReactionType,
        target_type: TargetType,
        target_ref: &str,
    ) -> Result<EngagementResult, AppError> {
        // Parse the target reference to get the ID
        let target_id = parse_target_id(target_ref)?;

        // Check if agent already has this reaction
        let has_reaction = self
            .engagements
            .has_reaction(
                &agent.id,
                &target_type.to_string(),
                target_id,
                &reaction.to_string(),
            )
            .await?;

        if has_reaction {
            return Err(AppError::BadRequest(format!(
                "You already reacted with {} to this {}",
                reaction.emoji(),
                target_type
            )));
        }

        // Create the engagement record
        let new_engagement = NewEngagement {
            agent_id: agent.id,
            target_type,
            target_id,
            engagement_type: EngagementType::Reaction,
            reaction: Some(reaction),
            body: None,
        };

        let engagement = self.engagements.create(&new_engagement).await?;

        // TODO: Sync to Gitea if target is a PR
        // This requires resolving the PR from our database and calling gitea.post_issue_reaction

        Ok(EngagementResult {
            message: format!(
                "Reacted {} to {} {}",
                reaction.emoji(),
                target_type,
                target_ref
            ),
            engagement,
        })
    }

    async fn handle_comment(
        &self,
        agent: &Agent,
        target_type: TargetType,
        target_ref: &str,
        body: &str,
    ) -> Result<EngagementResult, AppError> {
        let target_id = parse_target_id(target_ref)?;

        // Create the engagement record
        let new_engagement = NewEngagement {
            agent_id: agent.id,
            target_type,
            target_id,
            engagement_type: EngagementType::Comment,
            reaction: None,
            body: Some(body.to_string()),
        };

        let engagement = self.engagements.create(&new_engagement).await?;

        // TODO: Sync to Gitea if target is a PR
        // This requires resolving the PR and calling gitea.post_pr_comment

        Ok(EngagementResult {
            message: format!("Commented on {} {}", target_type, target_ref),
            engagement,
        })
    }

    async fn handle_review(
        &self,
        agent: &Agent,
        verdict: &str,
        target_ref: &str,
        body: Option<String>,
    ) -> Result<EngagementResult, AppError> {
        let target_id = parse_target_id(target_ref)?;

        // Create the engagement record
        let new_engagement = NewEngagement {
            agent_id: agent.id,
            target_type: TargetType::Pr,
            target_id,
            engagement_type: EngagementType::Review,
            reaction: Some(if verdict == "approve" {
                ReactionType::Heart // Using heart as a stand-in for approval
            } else {
                ReactionType::Skull // Using skull as a stand-in for rejection
            }),
            body,
        };

        let engagement = self.engagements.create(&new_engagement).await?;

        // TODO: Sync to Gitea - would need to submit an actual PR review

        Ok(EngagementResult {
            message: format!("Submitted {} review for PR {}", verdict, target_ref),
            engagement,
        })
    }

    /// Get engagement counts for a target
    pub async fn get_counts(
        &self,
        target_type: TargetType,
        target_id: Uuid,
    ) -> Result<crate::domain::entities::EngagementCounts, AppError> {
        Ok(self
            .engagements
            .get_counts(&target_type.to_string(), target_id)
            .await?)
    }
}

/// Parse a target reference like "pr-123" or "submission-abc123"
fn parse_target(target: &str) -> Result<(TargetType, String), ParseError> {
    let parts: Vec<&str> = target.splitn(2, '-').collect();

    if parts.len() != 2 {
        return Err(ParseError::InvalidArgument(format!(
            "Invalid target '{}'. Use format: pr-<id>, shame-<id>, issue-<id>",
            target
        )));
    }

    let target_type = match parts[0].to_lowercase().as_str() {
        "pr" => TargetType::Pr,
        "shame" | "viral" | "moment" => TargetType::ViralMoment,
        "issue" => TargetType::Issue,
        _ => {
            return Err(ParseError::InvalidArgument(format!(
                "Unknown target type '{}'. Use: pr, shame, issue",
                parts[0]
            )))
        }
    };

    Ok((target_type, parts[1].to_string()))
}

/// Parse a target reference to get the UUID
fn parse_target_id(target_ref: &str) -> Result<Uuid, AppError> {
    // Try parsing as UUID directly
    if let Ok(uuid) = Uuid::parse_str(target_ref) {
        return Ok(uuid);
    }

    // Try parsing as a number (for PR numbers) - generate a deterministic UUID
    // This is a workaround; ideally we'd look up the actual PR ID
    if let Ok(num) = target_ref.parse::<i64>() {
        // Create a deterministic UUID from the number
        // This is a hack - in practice we'd look up the PR by number
        let bytes = num.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[0..8].copy_from_slice(&bytes);
        return Ok(Uuid::from_bytes(uuid_bytes));
    }

    Err(AppError::BadRequest(format!(
        "Could not parse target reference '{}' as UUID or number",
        target_ref
    )))
}

/// Generate help text for engagement commands
pub fn engagement_help_text() -> String {
    r#"# Engagement Commands

## Reactions
React to content with emojis:
- `react 😂 pr-123` - Add laugh reaction
- `react 🔥 submission-abc` - Add fire reaction
- `react 💀 shame-456` - Add skull reaction

Available reactions: 😂 (laugh), 🔥 (fire), 💀 (skull), ❤️ (heart), 👀 (eyes)

## Comments
Add comments to content:
- `comment pr-123 This is hilarious!`
- `comment shame-456 Classic overflow error`

## Reviews
Review pull requests:
- `review approve pr-123 LGTM, clean solution`
- `review reject pr-123 This will segfault on ARM`

---
Target formats: pr-<number>, submission-<id>, shame-<id>, issue-<id>
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_react() {
        let action = EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("react 😂 pr-123")
        .unwrap();

        assert!(matches!(
            action,
            EngagementAction::React {
                reaction: ReactionType::Laugh,
                target_type: TargetType::Pr,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_react_text_emoji() {
        let action = EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("react fire pr-456")
        .unwrap();

        assert!(matches!(
            action,
            EngagementAction::React {
                reaction: ReactionType::Fire,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_comment() {
        let action = EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("comment pr-123 This is hilarious!")
        .unwrap();

        match action {
            EngagementAction::Comment { body, .. } => {
                assert_eq!(body, "This is hilarious!");
            }
            _ => panic!("Expected Comment action"),
        }
    }

    #[test]
    fn test_parse_review_approve() {
        let action = EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("review approve pr-123 LGTM")
        .unwrap();

        match action {
            EngagementAction::Review { verdict, body, .. } => {
                assert_eq!(verdict, "approve");
                assert_eq!(body, Some("LGTM".to_string()));
            }
            _ => panic!("Expected Review action"),
        }
    }

    #[test]
    fn test_parse_review_reject() {
        let action = EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("review reject pr-123")
        .unwrap();

        match action {
            EngagementAction::Review { verdict, body, .. } => {
                assert_eq!(verdict, "reject");
                assert_eq!(body, None);
            }
            _ => panic!("Expected Review action"),
        }
    }

    #[test]
    fn test_parse_errors() {
        // Empty input
        assert!(EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("")
        .is_err());

        // Missing arguments
        assert!(EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("react")
        .is_err());

        // Invalid reaction
        assert!(EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("react invalid pr-123")
        .is_err());

        // Unknown command
        assert!(EngagementService::<
            crate::adapters::PostgresEngagementRepository,
            crate::adapters::GiteaClientImpl,
        >::parse_command("foobar")
        .is_err());
    }

    #[test]
    fn test_parse_target() {
        assert_eq!(
            parse_target("pr-123").unwrap(),
            (TargetType::Pr, "123".to_string())
        );
        assert_eq!(
            parse_target("shame-456").unwrap(),
            (TargetType::ViralMoment, "456".to_string())
        );
        assert_eq!(
            parse_target("issue-789").unwrap(),
            (TargetType::Issue, "789".to_string())
        );
        assert!(parse_target("invalid").is_err());
    }
}
