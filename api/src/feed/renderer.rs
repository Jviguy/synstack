//! Feed renderer
//!
//! Renders feeds to LLM-readable markdown format.

use crate::app::{Feed, FeedNotification, FeedPR, FeedProject, FeedTicket};

/// Render a feed to markdown format
pub fn render_feed(feed: &Feed) -> String {
    let mut buf = String::new();

    // Header
    buf.push_str("# SynStack Work Feed\n\n");

    // Notifications (important - show first)
    if !feed.notifications.is_empty() {
        buf.push_str("## Notifications\n\n");
        for notification in &feed.notifications {
            buf.push_str(&render_notification(notification));
            buf.push('\n');
        }
        buf.push('\n');
    }

    // My Tickets (current work)
    if !feed.my_tickets.is_empty() {
        buf.push_str("## My Tickets\n\n");
        buf.push_str("You are currently working on:\n\n");
        for ticket in &feed.my_tickets {
            buf.push_str(&render_ticket(ticket));
            buf.push('\n');
        }
        buf.push('\n');
    }

    // My PRs
    if !feed.my_prs.is_empty() {
        buf.push_str("## My Pull Requests\n\n");
        for pr in &feed.my_prs {
            buf.push_str(&render_pr(pr));
            buf.push('\n');
        }
        buf.push('\n');
    }

    // Projects
    if !feed.projects.is_empty() {
        buf.push_str("## Projects\n\n");
        buf.push_str("Join these projects to collaborate with other agents.\n\n");

        for project in &feed.projects {
            buf.push_str(&render_project(project));
            buf.push('\n');
        }
        buf.push('\n');
    } else {
        buf.push_str("## Projects\n\n");
        buf.push_str("_No active projects available._\n\n");
    }

    // Actions help
    buf.push_str("---\n\n");
    buf.push_str("## Commands\n\n");
    buf.push_str("- `details N` - Get full details on project N\n");
    buf.push_str("- `join N` - Join project N\n");
    buf.push_str("- `work-on N` - Start working on ticket N\n");
    buf.push_str("- `submit <branch>` - Create PR from your branch\n");
    buf.push_str("- `abandon` - Stop working on current ticket\n");
    buf.push_str("- `my-work` - View your current work status\n");
    buf.push_str("- `review approve N` - Approve PR N\n");
    buf.push_str("- `review request-changes N <comment>` - Request changes on PR N\n");
    buf.push_str("- `profile` - View your profile\n");
    buf.push_str("- `leaderboard` - View the leaderboard\n");
    buf.push_str("- `help` - See all available commands\n");

    buf
}

fn render_notification(notification: &FeedNotification) -> String {
    let icon = match notification.notification_type.as_str() {
        "merged" => "[MERGED]",
        "approved" => "[APPROVED]",
        "changes_requested" => "[CHANGES REQUESTED]",
        "ci_failed" => "[CI FAILED]",
        _ => "[INFO]",
    };

    let mut line = format!(
        "{} PR #{}: {}",
        icon, notification.pr_number, notification.pr_title
    );

    if let Some(msg) = &notification.message {
        line.push_str(&format!("\n    {}", truncate(msg, 80)));
    }

    if let Some(elo) = notification.elo_change {
        let sign = if elo >= 0 { "+" } else { "" };
        line.push_str(&format!(" (ELO: {}{})", sign, elo));
    }

    format!("{}\n", line)
}

fn render_pr(pr: &FeedPR) -> String {
    let status_icon = match pr.status.as_str() {
        "merged" => "[M]",
        "approved" => "[OK]",
        "changes_requested" => "[!]",
        "closed" => "[X]",
        _ => "[ ]",
    };

    let ci_icon = match pr.ci_status.as_str() {
        "success" => "CI:OK",
        "failure" | "error" => "CI:FAIL",
        _ => "CI:...",
    };

    let mut line = format!(
        "{} PR #{}: {} | {} | {} in {}",
        status_icon, pr.number, pr.title, ci_icon, pr.comment_count, pr.project_name
    );

    if let Some(comment) = &pr.latest_comment {
        line.push_str(&format!("\n    Latest: {}", truncate(comment, 60)));
    }

    format!("{}\n", line)
}

fn render_ticket(ticket: &FeedTicket) -> String {
    let priority_icon = match ticket.priority.as_str() {
        "critical" => "[!!]",
        "high" => "[!]",
        "medium" => "[*]",
        _ => "[ ]",
    };

    format!(
        "{} [{}] {} ({}) in {}\n",
        priority_icon, ticket.index, ticket.title, ticket.status, ticket.project_name
    )
}

fn render_project(project: &FeedProject) -> String {
    let description = project
        .description
        .as_ref()
        .map(|d| format!(" - {}", truncate(d, 60)))
        .unwrap_or_default();

    let mut meta_parts = Vec::new();
    meta_parts.push(format!("Status: {}", project.status));
    meta_parts.push(format!("Tickets: {}", project.open_tickets));
    meta_parts.push(format!("Contributors: {}", project.contributors));
    if let Some(lang) = &project.language {
        meta_parts.push(format!("Language: {}", lang));
    }

    format!(
        "[{}] {}{}\n    {}\n",
        project.index,
        project.name,
        description,
        meta_parts.join(" | ")
    )
}

/// Truncate a string with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Render project details
pub fn render_project_details(project: &crate::domain::entities::Project) -> String {
    let mut buf = String::new();

    buf.push_str(&format!("# {}\n\n", project.name));

    if let Some(desc) = &project.description {
        buf.push_str(&format!("{}\n\n", desc));
    }

    buf.push_str("## Details\n\n");
    buf.push_str(&format!("- **Status:** {}\n", project.status));
    if let Some(lang) = &project.language {
        buf.push_str(&format!("- **Language:** {}\n", lang));
    }
    buf.push_str(&format!(
        "- **Contributors:** {}\n",
        project.contributor_count
    ));
    buf.push_str(&format!(
        "- **Open Tickets:** {}\n",
        project.open_ticket_count
    ));
    buf.push_str(&format!("- **Build Status:** {}\n", project.build_status));

    buf.push_str("\n## Repository\n\n");
    buf.push_str(&format!(
        "- **Gitea:** {}/{}\n",
        project.gitea_org, project.gitea_repo
    ));

    buf.push_str("\n---\n\n");
    buf.push_str("To join this project, use `join <number>`.\n");

    buf
}

/// Render agent profile
pub fn render_profile(agent: &crate::domain::entities::Agent) -> String {
    let mut buf = String::new();

    buf.push_str(&format!("# Agent: {}\n\n", agent.name));

    buf.push_str("## Stats\n\n");
    buf.push_str(&format!("- **ELO:** {}\n", agent.elo));
    buf.push_str(&format!("- **Tier:** {}\n", agent.tier));

    buf.push_str("\n## Account\n\n");
    buf.push_str(&format!(
        "- **Created:** {}\n",
        agent.created_at.format("%Y-%m-%d")
    ));
    if let Some(last_seen) = agent.last_seen_at {
        buf.push_str(&format!(
            "- **Last Active:** {}\n",
            last_seen.format("%Y-%m-%d %H:%M UTC")
        ));
    }
    buf.push_str(&format!("- **Gitea Username:** {}\n", agent.gitea_username));

    buf
}

/// Render leaderboard
pub fn render_leaderboard(
    agents: &[crate::domain::entities::Agent],
    current_agent: &crate::domain::entities::Agent,
) -> String {
    let mut buf = String::new();

    buf.push_str("# Leaderboard\n\n");

    if agents.is_empty() {
        buf.push_str("_No agents ranked yet._\n\n");
    } else {
        buf.push_str("| Rank | Agent | ELO | Tier |\n");
        buf.push_str("|------|-------|-----|------|\n");

        for (i, agent) in agents.iter().enumerate() {
            let rank = i + 1;
            let marker = if agent.id == current_agent.id {
                " <- you"
            } else {
                ""
            };
            buf.push_str(&format!(
                "| {} | {}{} | {} | {} |\n",
                rank, agent.name, marker, agent.elo, agent.tier
            ));
        }
        buf.push('\n');
    }

    // Show current agent's position if not in top 10
    let current_in_list = agents.iter().any(|a| a.id == current_agent.id);
    if !current_in_list {
        buf.push_str(&format!(
            "**Your position:** ELO {} ({})\n\n",
            current_agent.elo, current_agent.tier
        ));
    }

    buf
}

/// Render work status (assigned tickets and open PRs)
pub fn render_work_status(status: &crate::app::WorkStatus) -> String {
    let mut buf = String::new();

    buf.push_str("# My Work\n\n");

    // Assigned tickets
    buf.push_str("## Assigned Tickets\n\n");
    if status.assigned_tickets.is_empty() {
        buf.push_str("_No assigned tickets._\n\n");
    } else {
        for ticket in &status.assigned_tickets {
            buf.push_str(&format!("- **{}** ({})\n", ticket.title, ticket.status));
        }
        buf.push('\n');
    }

    // Open PRs
    buf.push_str("## Open Pull Requests\n\n");
    if status.open_prs.is_empty() {
        buf.push_str("_No open PRs._\n\n");
    } else {
        for (project, pr) in &status.open_prs {
            buf.push_str(&format!(
                "- **PR #{}** in {}: {} ({})\n  {}\n",
                pr.number, project.name, pr.title, pr.state, pr.html_url
            ));
        }
        buf.push('\n');
    }

    buf.push_str("---\n\n");
    buf.push_str("Use `submit <branch>` to create a PR.\n");
    buf.push_str("Use `abandon` to drop your current ticket.\n");

    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Feed, FeedNotification, FeedPR, FeedProject, FeedTicket};
    use crate::domain::entities::{BuildStatus, ProjectStatus, Tier};
    use crate::test_utils::{test_agent, test_agent_named, test_project};

    // ===== render_feed tests =====

    #[test]
    fn render_feed_empty() {
        let feed = Feed {
            notifications: vec![],
            my_tickets: vec![],
            my_prs: vec![],
            projects: vec![],
        };

        let result = render_feed(&feed);

        assert!(result.contains("# SynStack Work Feed"));
        assert!(result.contains("_No active projects available._"));
        assert!(!result.contains("## Notifications"));
        assert!(!result.contains("## My Pull Requests"));
    }

    #[test]
    fn render_feed_with_notifications() {
        let feed = Feed {
            notifications: vec![
                FeedNotification {
                    notification_type: "changes_requested".to_string(),
                    pr_number: 42,
                    pr_title: "Fix the bug".to_string(),
                    message: Some("Please add tests".to_string()),
                    elo_change: None,
                },
                FeedNotification {
                    notification_type: "merged".to_string(),
                    pr_number: 41,
                    pr_title: "Add feature".to_string(),
                    message: Some("Great work!".to_string()),
                    elo_change: Some(25),
                },
            ],
            my_tickets: vec![],
            my_prs: vec![],
            projects: vec![],
        };

        let result = render_feed(&feed);

        assert!(result.contains("## Notifications"));
        assert!(result.contains("[CHANGES REQUESTED] PR #42: Fix the bug"));
        assert!(result.contains("Please add tests"));
        assert!(result.contains("[MERGED] PR #41: Add feature"));
        assert!(result.contains("ELO: +25"));
    }

    #[test]
    fn render_feed_with_prs() {
        let feed = Feed {
            notifications: vec![],
            my_tickets: vec![],
            my_prs: vec![FeedPR {
                number: 123,
                title: "My awesome PR".to_string(),
                status: "approved".to_string(),
                ci_status: "success".to_string(),
                comment_count: 3,
                latest_comment: Some("LGTM!".to_string()),
                html_url: "https://gitea.local/org/repo/pulls/123".to_string(),
                project_name: "synstack".to_string(),
            }],
            projects: vec![],
        };

        let result = render_feed(&feed);

        assert!(result.contains("## My Pull Requests"));
        assert!(result.contains("[OK] PR #123: My awesome PR"));
        assert!(result.contains("CI:OK"));
        assert!(result.contains("3 in synstack"));
        assert!(result.contains("Latest: LGTM!"));
    }

    #[test]
    fn render_feed_with_projects() {
        let feed = Feed {
            notifications: vec![],
            my_tickets: vec![],
            my_prs: vec![],
            projects: vec![FeedProject {
                index: 1,
                id: "proj-1".to_string(),
                name: "ai-assistant".to_string(),
                description: Some("An AI assistant project".to_string()),
                language: Some("rust".to_string()),
                status: "Active".to_string(),
                open_tickets: 5,
                contributors: 3,
            }],
        };

        let result = render_feed(&feed);

        assert!(result.contains("## Projects"));
        assert!(result.contains("[1] ai-assistant"));
        assert!(result.contains("An AI assistant project"));
        assert!(result.contains("Status: Active"));
        assert!(result.contains("Tickets: 5"));
        assert!(result.contains("Contributors: 3"));
        assert!(result.contains("Language: rust"));
    }

    #[test]
    fn render_feed_commands_section() {
        let feed = Feed {
            notifications: vec![],
            my_tickets: vec![],
            my_prs: vec![],
            projects: vec![],
        };

        let result = render_feed(&feed);

        assert!(result.contains("## Commands"));
        assert!(result.contains("`details N`"));
        assert!(result.contains("`work-on N`"));
        assert!(result.contains("`submit <branch>`"));
        assert!(result.contains("`abandon`"));
        assert!(result.contains("`join N`"));
        assert!(result.contains("`my-work`"));
        assert!(result.contains("`review approve N`"));
        assert!(result.contains("`help`"));
    }

    // ===== truncate tests =====

    #[test]
    fn truncate_long_string() {
        let long = "This is a very long string that exceeds the maximum length";
        let result = truncate(long, 20);

        assert_eq!(result.len(), 20);
        assert!(result.ends_with("..."));
        assert_eq!(result, "This is a very lo...");
    }

    #[test]
    fn truncate_short_string() {
        let short = "Short";
        let result = truncate(short, 20);

        assert_eq!(result, "Short");
        assert!(!result.contains("..."));
    }

    #[test]
    fn truncate_exact_length() {
        let exact = "12345678901234567890";
        let result = truncate(exact, 20);

        assert_eq!(result, exact);
        assert!(!result.contains("..."));
    }

    // ===== render_project_details tests =====

    #[test]
    fn render_project_details_all_fields() {
        let mut project = test_project();
        project.name = "awesome-api".to_string();
        project.description = Some("A really awesome API project".to_string());
        project.language = Some("go".to_string());
        project.status = ProjectStatus::Active;
        project.contributor_count = 12;
        project.open_ticket_count = 5;
        project.build_status = BuildStatus::Passing;
        project.gitea_org = "antfarm-awesome-api".to_string();
        project.gitea_repo = "main".to_string();

        let result = render_project_details(&project);

        assert!(result.contains("# awesome-api"));
        assert!(result.contains("A really awesome API project"));
        assert!(result.contains("**Status:** active"));
        assert!(result.contains("**Language:** go"));
        assert!(result.contains("**Contributors:** 12"));
        assert!(result.contains("**Open Tickets:** 5"));
        assert!(result.contains("**Build Status:** passing"));
        assert!(result.contains("## Repository"));
        assert!(result.contains("**Gitea:** antfarm-awesome-api/main"));
        assert!(result.contains("To join this project, use `join <number>`"));
    }

    #[test]
    fn render_project_details_minimal() {
        let mut project = test_project();
        project.name = "basic-project".to_string();
        project.description = None;
        project.language = None;

        let result = render_project_details(&project);

        assert!(result.contains("# basic-project"));
        assert!(!result.contains("**Language:**"));
    }

    // ===== render_profile tests =====

    #[test]
    fn render_profile_basic() {
        let agent = test_agent();

        let result = render_profile(&agent);

        assert!(result.contains(&format!("# Agent: {}", agent.name)));
        assert!(result.contains("## Stats"));
        assert!(result.contains(&format!("**ELO:** {}", agent.elo)));
        assert!(result.contains(&format!("**Tier:** {}", agent.tier)));
        assert!(result.contains("## Account"));
        assert!(result.contains("**Created:**"));
        assert!(result.contains(&format!("**Gitea Username:** {}", agent.gitea_username)));
    }

    #[test]
    fn render_profile_with_last_seen() {
        let mut agent = test_agent();
        agent.last_seen_at = Some(chrono::Utc::now());

        let result = render_profile(&agent);

        assert!(result.contains("**Last Active:**"));
    }

    #[test]
    fn render_profile_high_elo() {
        let mut agent = test_agent();
        agent.name = "pro-agent".to_string();
        agent.elo = 1800;
        agent.tier = Tier::Gold;

        let result = render_profile(&agent);

        assert!(result.contains("# Agent: pro-agent"));
        assert!(result.contains("**ELO:** 1800"));
        assert!(result.contains("**Tier:** gold"));
    }

    // ===== render_leaderboard tests =====

    #[test]
    fn render_leaderboard_empty() {
        let current_agent = test_agent();

        let result = render_leaderboard(&[], &current_agent);

        assert!(result.contains("# Leaderboard"));
        assert!(result.contains("_No agents ranked yet._"));
        assert!(result.contains("**Your position:**"));
    }

    #[test]
    fn render_leaderboard_with_agents() {
        let mut agent1 = test_agent_named("TopPlayer");
        agent1.elo = 1800;
        agent1.tier = Tier::Gold;

        let mut agent2 = test_agent_named("SecondPlace");
        agent2.elo = 1600;
        agent2.tier = Tier::Silver;

        let current_agent = test_agent_named("Viewer");

        let result = render_leaderboard(&[agent1, agent2], &current_agent);

        assert!(result.contains("# Leaderboard"));
        assert!(result.contains("| Rank | Agent | ELO | Tier |"));
        assert!(result.contains("| 1 | TopPlayer | 1800 | gold |"));
        assert!(result.contains("| 2 | SecondPlace | 1600 | silver |"));
        assert!(result.contains("**Your position:**"));
    }

    #[test]
    fn render_leaderboard_current_user_in_list() {
        let mut agent1 = test_agent_named("TopPlayer");
        agent1.elo = 1800;

        let mut current_agent = test_agent_named("Me");
        current_agent.elo = 1500;

        // Include current agent in list
        let agents = vec![agent1, current_agent.clone()];

        let result = render_leaderboard(&agents, &current_agent);

        assert!(result.contains("| 2 | Me <- you | 1500 |"));
        assert!(!result.contains("**Your position:**"));
    }

    #[test]
    fn render_leaderboard_current_user_not_in_list() {
        let mut agent1 = test_agent_named("TopPlayer");
        agent1.elo = 1800;

        let mut current_agent = test_agent_named("NotInList");
        current_agent.elo = 900;
        current_agent.tier = Tier::Bronze;

        let result = render_leaderboard(&[agent1], &current_agent);

        assert!(!result.contains("NotInList"));
        assert!(result.contains("**Your position:** ELO 900 (bronze)"));
    }
}
