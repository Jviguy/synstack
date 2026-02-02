//! Test fixtures
//!
//! Factory functions for creating test data with sensible defaults.
//! Each fixture function creates a valid entity that can be customized.

use chrono::Utc;
use uuid::Uuid;

use crate::domain::entities::{
    Agent, AgentId, AgentReview, AgentReviewId, BuildStatus, CodeContribution, CodeContributionId,
    ContributionStatus, EloEvent, EloEventId, EloEventType, Issue, IssueId, IssueState, MemberRole,
    Project, ProjectId, ProjectMember, ProjectStatus, ReviewVerdict, Tier,
};

/// Create a test agent with default values
pub fn test_agent() -> Agent {
    Agent {
        id: AgentId(Uuid::new_v4()),
        name: "test-agent".to_string(),
        api_key_hash: "abc123hash".to_string(),
        gitea_username: "agent-test-agent".to_string(),
        elo: 1000,
        tier: Tier::Bronze,
        created_at: Utc::now(),
        last_seen_at: None,
        claim_code: Some("test-claim-code".to_string()),
        claimed_at: None,
        github_id: None,
        github_username: None,
        github_avatar_url: None,
    }
}

/// Create a test agent with a specific name
pub fn test_agent_named(name: &str) -> Agent {
    Agent {
        id: AgentId(Uuid::new_v4()),
        name: name.to_string(),
        api_key_hash: format!("hash-{}", name),
        gitea_username: format!("agent-{}", name.to_lowercase().replace(' ', "-")),
        elo: 1000,
        tier: Tier::Bronze,
        created_at: Utc::now(),
        last_seen_at: None,
        claim_code: Some(format!("claim-{}", name)),
        claimed_at: None,
        github_id: None,
        github_username: None,
        github_avatar_url: None,
    }
}

/// Create a test agent with a specific tier
pub fn test_agent_with_tier(tier: Tier) -> Agent {
    let elo = match tier {
        Tier::Bronze => 1000,
        Tier::Silver => 1400,
        Tier::Gold => 1800,
    };
    Agent {
        id: AgentId(Uuid::new_v4()),
        name: format!("{}-agent", tier),
        api_key_hash: "abc123hash".to_string(),
        gitea_username: format!("agent-{}", tier),
        elo,
        tier,
        created_at: Utc::now(),
        last_seen_at: None,
        claim_code: Some(format!("claim-{}", tier)),
        claimed_at: None,
        github_id: None,
        github_username: None,
        github_avatar_url: None,
    }
}

/// Create a test agent with specific ELO
pub fn test_agent_with_elo(elo: i32) -> Agent {
    Agent {
        id: AgentId(Uuid::new_v4()),
        name: format!("agent-elo-{}", elo),
        api_key_hash: format!("hash-{}", elo),
        gitea_username: format!("agent-elo-{}", elo),
        elo,
        tier: Tier::from_elo(elo),
        created_at: Utc::now(),
        last_seen_at: None,
        claim_code: Some(format!("claim-{}", elo)),
        claimed_at: None,
        github_id: None,
        github_username: None,
        github_avatar_url: None,
    }
}

/// Create a test issue with default values
pub fn test_issue(project_id: ProjectId) -> Issue {
    Issue {
        id: IssueId::new(project_id, 1),
        title: "Test Issue".to_string(),
        body: Some("This is a test issue body.".to_string()),
        state: IssueState::Open,
        url: "https://gitea.test/org/repo/issues/1".to_string(),
        labels: vec![],
        assignees: vec![],
    }
}

/// Create a test issue with specific state
pub fn test_issue_with_state(project_id: ProjectId, number: i64, state: IssueState) -> Issue {
    Issue {
        id: IssueId::new(project_id, number),
        title: format!("{:?} Issue #{}", state, number),
        body: Some("This is a test issue body.".to_string()),
        state,
        url: format!("https://gitea.test/org/repo/issues/{}", number),
        labels: vec![],
        assignees: vec![],
    }
}

/// Create a test project with default values
pub fn test_project() -> Project {
    Project {
        id: ProjectId(Uuid::new_v4()),
        name: "test-project".to_string(),
        description: Some("A test project".to_string()),
        gitea_org: "synstack".to_string(),
        gitea_repo: "test-project".to_string(),
        language: Some("rust".to_string()),
        status: ProjectStatus::Active,
        contributor_count: 0,
        open_ticket_count: 0,
        build_status: BuildStatus::Unknown,
        created_by: None,
        created_at: Utc::now(),
    }
}

/// Create a test project with specific status
pub fn test_project_with_status(status: ProjectStatus) -> Project {
    Project {
        id: ProjectId(Uuid::new_v4()),
        name: format!("{:?}-project", status),
        description: Some("A test project".to_string()),
        gitea_org: "synstack".to_string(),
        gitea_repo: format!("{:?}-project", status).to_lowercase(),
        language: Some("rust".to_string()),
        status,
        contributor_count: 0,
        open_ticket_count: 0,
        build_status: BuildStatus::Unknown,
        created_by: None,
        created_at: Utc::now(),
    }
}

/// Create a test project member
pub fn test_project_member(
    project_id: ProjectId,
    agent_id: AgentId,
    role: MemberRole,
) -> ProjectMember {
    ProjectMember {
        project_id,
        agent_id,
        role,
        joined_at: Utc::now(),
    }
}

/// Create a test code contribution with default values
pub fn test_code_contribution(agent_id: AgentId, project_id: ProjectId) -> CodeContribution {
    CodeContribution {
        id: CodeContributionId::new(),
        agent_id,
        project_id,
        pr_number: 42,
        commit_sha: "abc123def456".to_string(),
        status: ContributionStatus::Healthy,
        bug_count: 0,
        longevity_bonus_paid: false,
        dependent_prs_count: 0,
        merged_at: Utc::now(),
        reverted_at: None,
        replaced_at: None,
        created_at: Utc::now(),
    }
}

/// Create a code contribution merged at a specific time (useful for longevity tests)
pub fn test_code_contribution_merged_at(
    agent_id: AgentId,
    project_id: ProjectId,
    merged_at: chrono::DateTime<Utc>,
) -> CodeContribution {
    CodeContribution {
        id: CodeContributionId::new(),
        agent_id,
        project_id,
        pr_number: 42,
        commit_sha: "abc123def456".to_string(),
        status: ContributionStatus::Healthy,
        bug_count: 0,
        longevity_bonus_paid: false,
        dependent_prs_count: 0,
        merged_at,
        reverted_at: None,
        replaced_at: None,
        created_at: merged_at,
    }
}

/// Create a reverted code contribution
pub fn test_reverted_contribution(agent_id: AgentId, project_id: ProjectId) -> CodeContribution {
    let merged_at = Utc::now() - chrono::Duration::hours(2);
    CodeContribution {
        id: CodeContributionId::new(),
        agent_id,
        project_id,
        pr_number: 42,
        commit_sha: "abc123def456".to_string(),
        status: ContributionStatus::Reverted,
        bug_count: 0,
        longevity_bonus_paid: false,
        dependent_prs_count: 0,
        merged_at,
        reverted_at: Some(Utc::now()),
        replaced_at: None,
        created_at: merged_at,
    }
}

/// Create a test agent review with default values
pub fn test_agent_review(
    reviewer_id: AgentId,
    reviewed_id: AgentId,
    project_id: ProjectId,
    verdict: ReviewVerdict,
) -> AgentReview {
    AgentReview {
        id: AgentReviewId::new(),
        pr_id: 123,
        project_id,
        reviewer_agent_id: reviewer_id,
        reviewed_agent_id: reviewed_id,
        verdict,
        reviewer_elo_at_time: 1000,
        created_at: Utc::now(),
    }
}

/// Create a high-ELO agent review (for bonus tests)
pub fn test_high_elo_review(
    reviewer_id: AgentId,
    reviewed_id: AgentId,
    project_id: ProjectId,
) -> AgentReview {
    AgentReview {
        id: AgentReviewId::new(),
        pr_id: 123,
        project_id,
        reviewer_agent_id: reviewer_id,
        reviewed_agent_id: reviewed_id,
        verdict: ReviewVerdict::Approved,
        reviewer_elo_at_time: 1500, // High ELO (>= 1400)
        created_at: Utc::now(),
    }
}

/// Create a test ELO event
pub fn test_elo_event(agent_id: AgentId, event_type: EloEventType, delta: i32) -> EloEvent {
    EloEvent {
        id: EloEventId::new(),
        agent_id,
        event_type,
        delta,
        old_elo: 1000,
        new_elo: 1000 + delta,
        reference_id: Some(Uuid::new_v4()),
        details: Some("Test ELO event".to_string()),
        created_at: Utc::now(),
    }
}

/// Create a test ticket with default values
pub fn test_ticket(project_id: ProjectId) -> crate::domain::entities::Ticket {
    crate::domain::entities::Ticket {
        id: crate::domain::entities::TicketId::new(),
        project_id,
        title: "Test Ticket".to_string(),
        body: Some("This is a test ticket body.".to_string()),
        gitea_issue_number: Some(1),
        gitea_issue_url: Some("https://gitea.test/org/repo/issues/1".to_string()),
        status: crate::domain::entities::TicketStatus::Open,
        priority: crate::domain::entities::TicketPriority::Medium,
        assigned_to: None,
        created_by: None,
        created_at: Utc::now(),
        closed_at: None,
    }
}

/// Create a test ticket assigned to an agent
pub fn test_ticket_assigned(
    project_id: ProjectId,
    agent_id: AgentId,
) -> crate::domain::entities::Ticket {
    crate::domain::entities::Ticket {
        id: crate::domain::entities::TicketId::new(),
        project_id,
        title: "Assigned Ticket".to_string(),
        body: Some("This ticket is assigned.".to_string()),
        gitea_issue_number: Some(2),
        gitea_issue_url: Some("https://gitea.test/org/repo/issues/2".to_string()),
        status: crate::domain::entities::TicketStatus::InProgress,
        priority: crate::domain::entities::TicketPriority::Medium,
        assigned_to: Some(agent_id),
        created_by: None,
        created_at: Utc::now(),
        closed_at: None,
    }
}
