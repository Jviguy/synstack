//! PostgreSQL integration tests
//!
//! These tests run against a real PostgreSQL database.
//! They are marked #[ignore] by default and should be run explicitly:
//!
//!   cargo test postgres_integration -- --ignored
//!
//! Requires:
//!   - PostgreSQL running on localhost:5432
//!   - Database 'synstack_test' with migrations applied
//!   - Environment variable TEST_DATABASE_URL or uses default

use sea_orm::{Database, DatabaseConnection};
use std::env;
use uuid::Uuid;

use super::*;
use crate::domain::entities::*;
use crate::domain::ports::*;

/// Get database connection for tests
async fn get_test_db() -> DatabaseConnection {
    let url = env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://synstack:REDACTED_PASSWORD@localhost:5432/synstack".to_string()
    });

    Database::connect(&url)
        .await
        .expect("Failed to connect to test database")
}

/// Generate a unique test name to avoid collisions
fn unique_name(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4().to_string()[..8].to_string())
}

// ============================================================================
// Agent Repository Tests
// ============================================================================

mod agent_repo_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn create_and_find_agent() {
        let db = get_test_db().await;
        let repo = PostgresAgentRepository::new(db);

        let name = unique_name("test-agent");
        let new_agent = NewAgent {
            name: name.clone(),
            api_key_hash: format!("hash-{}", Uuid::new_v4()),
            gitea_username: format!("agent-{}", name),
            gitea_token_encrypted: vec![1, 2, 3],
            claim_code: format!("claim-{}", Uuid::new_v4()),
        };

        // Create
        let agent = repo
            .create(&new_agent)
            .await
            .expect("Failed to create agent");
        assert_eq!(agent.name, name);
        assert_eq!(agent.elo, 1000); // default

        // Find by ID
        let found = repo
            .find_by_id(&agent.id)
            .await
            .expect("Failed to find agent");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, name);

        // Find by name
        let found = repo
            .find_by_name(&name)
            .await
            .expect("Failed to find by name");
        assert!(found.is_some());

        // Find by API key hash
        let found = repo
            .find_by_api_key_hash(&new_agent.api_key_hash)
            .await
            .expect("Failed to find by hash");
        assert!(found.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn update_elo() {
        let db = get_test_db().await;
        let repo = PostgresAgentRepository::new(db);

        let name = unique_name("elo-test");
        let new_agent = NewAgent {
            name: name.clone(),
            api_key_hash: format!("hash-{}", Uuid::new_v4()),
            gitea_username: format!("agent-{}", name),
            gitea_token_encrypted: vec![],
            claim_code: format!("claim-{}", Uuid::new_v4()),
        };

        let agent = repo.create(&new_agent).await.expect("Failed to create");

        // Update ELO
        repo.update_elo(&agent.id, 1500)
            .await
            .expect("Failed to update ELO");

        // Verify
        let updated = repo
            .find_by_id(&agent.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(updated.elo, 1500);
    }

    #[tokio::test]
    #[ignore]
    async fn update_last_seen() {
        let db = get_test_db().await;
        let repo = PostgresAgentRepository::new(db);

        let name = unique_name("seen-test");
        let new_agent = NewAgent {
            name: name.clone(),
            api_key_hash: format!("hash-{}", Uuid::new_v4()),
            gitea_username: format!("agent-{}", name),
            gitea_token_encrypted: vec![],
            claim_code: format!("claim-{}", Uuid::new_v4()),
        };

        let agent = repo.create(&new_agent).await.expect("Failed to create");
        assert!(agent.last_seen_at.is_none());

        // Update last seen
        repo.update_last_seen(&agent.id)
            .await
            .expect("Failed to update last seen");

        // Verify
        let updated = repo
            .find_by_id(&agent.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert!(updated.last_seen_at.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn leaderboard() {
        let db = get_test_db().await;
        let repo = PostgresAgentRepository::new(db);

        // Create agents with different ELOs
        for i in 0..3 {
            let name = unique_name(&format!("leaderboard-{}", i));
            let new_agent = NewAgent {
                name: name.clone(),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: format!("agent-{}", name),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            };
            let agent = repo.create(&new_agent).await.expect("Failed to create");
            repo.update_elo(&agent.id, 1000 + (i * 100))
                .await
                .expect("Failed to update ELO");
        }

        // Get leaderboard
        let top = repo
            .find_top_by_elo(10)
            .await
            .expect("Failed to get leaderboard");
        assert!(!top.is_empty());

        // Verify ordering (highest first)
        for i in 1..top.len() {
            assert!(top[i - 1].elo >= top[i].elo);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn find_by_claim_code() {
        let db = get_test_db().await;
        let repo = PostgresAgentRepository::new(db);

        let name = unique_name("claim-code-test");
        let claim_code = format!("claim-{}", Uuid::new_v4());
        let new_agent = NewAgent {
            name: name.clone(),
            api_key_hash: format!("hash-{}", Uuid::new_v4()),
            gitea_username: format!("agent-{}", name),
            gitea_token_encrypted: vec![],
            claim_code: claim_code.clone(),
        };

        let agent = repo.create(&new_agent).await.expect("Failed to create");

        // Find by claim code
        let found = repo
            .find_by_claim_code(&claim_code)
            .await
            .expect("Failed to find");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, agent.id);

        // Invalid code returns None
        let not_found = repo
            .find_by_claim_code("invalid-code")
            .await
            .expect("Failed to find");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn claim_agent() {
        let db = get_test_db().await;
        let repo = PostgresAgentRepository::new(db);

        let name = unique_name("claim-test");
        let claim_code = format!("claim-{}", Uuid::new_v4());
        let new_agent = NewAgent {
            name: name.clone(),
            api_key_hash: format!("hash-{}", Uuid::new_v4()),
            gitea_username: format!("agent-{}", name),
            gitea_token_encrypted: vec![],
            claim_code: claim_code.clone(),
        };

        let agent = repo.create(&new_agent).await.expect("Failed to create");
        assert!(agent.claimed_at.is_none());
        assert!(agent.github_id.is_none());

        // Claim the agent
        let github_id = rand::random::<i64>().abs();
        let claim = ClaimAgent {
            github_id,
            github_username: "test-user".to_string(),
            github_avatar_url: Some("https://github.com/avatar.png".to_string()),
        };

        repo.claim(&agent.id, &claim)
            .await
            .expect("Failed to claim");

        // Verify claim
        let claimed = repo
            .find_by_id(&agent.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert!(claimed.claimed_at.is_some());
        assert_eq!(claimed.github_id, Some(github_id));
        assert_eq!(claimed.github_username, Some("test-user".to_string()));
        assert!(claimed.claim_code.is_none()); // Should be cleared

        // Find by GitHub ID
        let by_github = repo
            .find_by_github_id(github_id)
            .await
            .expect("Failed to find");
        assert!(by_github.is_some());
        assert_eq!(by_github.unwrap().id, agent.id);

        // Old claim code no longer works
        let old_code = repo
            .find_by_claim_code(&claim_code)
            .await
            .expect("Failed to find");
        assert!(old_code.is_none());
    }
}

// ============================================================================
// Issue Repository Tests
// ============================================================================

// NOTE: Issue repository tests were removed - issues now live in Gitea (GiteaIssueRepository).
// NOTE: Claim and Submission repository tests were removed when Simulator mode was removed.
// Claims and Submissions were Simulator-only concepts.

// ============================================================================
// Code Contribution Repository Tests (Reactive ELO)
// ============================================================================

mod code_contribution_repo_tests {
    use super::*;
    use chrono::{Duration, Utc};

    async fn create_test_agent_and_project(
        db: &DatabaseConnection,
    ) -> (Agent, crate::domain::entities::Project) {
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let project_repo = PostgresProjectRepository::new(db.clone());

        let agent = agent_repo
            .create(&NewAgent {
                name: unique_name("contrib-agent"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent");

        let project = project_repo
            .create(&NewProject {
                name: unique_name("contrib-project"),
                description: Some("Test project".to_string()),
                gitea_org: unique_name("org"),
                gitea_repo: "repo".to_string(),
                language: Some("rust".to_string()),
                created_by: Some(agent.id),
            })
            .await
            .expect("Failed to create project");

        (agent, project)
    }

    #[tokio::test]
    #[ignore]
    async fn create_and_find_contribution() {
        let db = get_test_db().await;
        let (agent, project) = create_test_agent_and_project(&db).await;
        let repo = PostgresCodeContributionRepository::new(db);

        let new_contrib = NewCodeContribution {
            agent_id: agent.id,
            project_id: project.id,
            pr_number: 42,
            commit_sha: format!("sha-{}", Uuid::new_v4()),
            merged_at: Utc::now(),
        };

        // Create
        let contrib = repo
            .create(&new_contrib)
            .await
            .expect("Failed to create contribution");
        assert_eq!(contrib.status, ContributionStatus::Healthy);
        assert!(!contrib.longevity_bonus_paid);
        assert_eq!(contrib.bug_count, 0);
        assert_eq!(contrib.dependent_prs_count, 0);

        // Find by ID
        let found = repo
            .find_by_id(&contrib.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.pr_number, 42);

        // Find by commit SHA
        let found = repo
            .find_by_commit_sha(&new_contrib.commit_sha)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.id, contrib.id);

        // Find by PR
        let found = repo
            .find_by_pr(&project.id, 42)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.id, contrib.id);

        // Find by agent
        let by_agent = repo.find_by_agent(&agent.id).await.expect("Failed to find");
        assert!(!by_agent.is_empty());

        // Find by project
        let by_project = repo
            .find_by_project(&project.id)
            .await
            .expect("Failed to find");
        assert!(!by_project.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn update_contribution_status() {
        let db = get_test_db().await;
        let (agent, project) = create_test_agent_and_project(&db).await;
        let repo = PostgresCodeContributionRepository::new(db);

        let contrib = repo
            .create(&NewCodeContribution {
                agent_id: agent.id,
                project_id: project.id,
                pr_number: 99,
                commit_sha: format!("sha-{}", Uuid::new_v4()),
                merged_at: Utc::now(),
            })
            .await
            .expect("Failed to create");

        // Update to reverted
        repo.update_status(&contrib.id, ContributionStatus::Reverted, Utc::now())
            .await
            .expect("Failed to update");

        let updated = repo
            .find_by_id(&contrib.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(updated.status, ContributionStatus::Reverted);
        assert!(updated.reverted_at.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn mark_longevity_bonus_paid() {
        let db = get_test_db().await;
        let (agent, project) = create_test_agent_and_project(&db).await;
        let repo = PostgresCodeContributionRepository::new(db);

        let contrib = repo
            .create(&NewCodeContribution {
                agent_id: agent.id,
                project_id: project.id,
                pr_number: 100,
                commit_sha: format!("sha-{}", Uuid::new_v4()),
                merged_at: Utc::now() - Duration::days(31),
            })
            .await
            .expect("Failed to create");

        assert!(!contrib.longevity_bonus_paid);

        repo.mark_longevity_bonus_paid(&contrib.id)
            .await
            .expect("Failed to mark");

        let updated = repo
            .find_by_id(&contrib.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert!(updated.longevity_bonus_paid);
    }

    #[tokio::test]
    #[ignore]
    async fn increment_counters() {
        let db = get_test_db().await;
        let (agent, project) = create_test_agent_and_project(&db).await;
        let repo = PostgresCodeContributionRepository::new(db);

        let contrib = repo
            .create(&NewCodeContribution {
                agent_id: agent.id,
                project_id: project.id,
                pr_number: 101,
                commit_sha: format!("sha-{}", Uuid::new_v4()),
                merged_at: Utc::now(),
            })
            .await
            .expect("Failed to create");

        // Increment bug count
        repo.increment_bug_count(&contrib.id)
            .await
            .expect("Failed to increment bugs");
        repo.increment_bug_count(&contrib.id)
            .await
            .expect("Failed to increment bugs");

        let updated = repo
            .find_by_id(&contrib.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(updated.bug_count, 2);

        // Increment dependent PRs
        repo.increment_dependent_prs(&contrib.id)
            .await
            .expect("Failed to increment deps");

        let updated = repo
            .find_by_id(&contrib.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(updated.dependent_prs_count, 1);
    }

    #[tokio::test]
    #[ignore]
    async fn find_eligible_for_longevity_bonus() {
        let db = get_test_db().await;
        let (agent, project) = create_test_agent_and_project(&db).await;
        let repo = PostgresCodeContributionRepository::new(db);

        // Create old eligible contribution
        let _old = repo
            .create(&NewCodeContribution {
                agent_id: agent.id,
                project_id: project.id,
                pr_number: 200,
                commit_sha: format!("sha-old-{}", Uuid::new_v4()),
                merged_at: Utc::now() - Duration::days(35),
            })
            .await
            .expect("Failed to create");

        // Create recent non-eligible contribution
        let _recent = repo
            .create(&NewCodeContribution {
                agent_id: agent.id,
                project_id: project.id,
                pr_number: 201,
                commit_sha: format!("sha-recent-{}", Uuid::new_v4()),
                merged_at: Utc::now() - Duration::days(5),
            })
            .await
            .expect("Failed to create");

        // Find eligible (merged before 30 days ago)
        let threshold = Utc::now() - Duration::days(30);
        let eligible = repo
            .find_eligible_for_longevity_bonus(threshold)
            .await
            .expect("Failed to find");

        // At least the old one should be eligible
        assert!(eligible.iter().any(|c| c.pr_number == 200));
        // Recent one should not be eligible
        assert!(!eligible.iter().any(|c| c.pr_number == 201));
    }
}

// ============================================================================
// Agent Review Repository Tests (Reactive ELO)
// ============================================================================

mod agent_review_repo_tests {
    use super::*;
    use chrono::{Duration, Utc};

    async fn create_test_agents_and_project(
        db: &DatabaseConnection,
    ) -> (Agent, Agent, crate::domain::entities::Project) {
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let project_repo = PostgresProjectRepository::new(db.clone());

        let reviewer = agent_repo
            .create(&NewAgent {
                name: unique_name("reviewer"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea-r"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create reviewer");

        let reviewed = agent_repo
            .create(&NewAgent {
                name: unique_name("reviewed"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea-d"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create reviewed");

        let project = project_repo
            .create(&NewProject {
                name: unique_name("review-project"),
                description: Some("Test project".to_string()),
                gitea_org: unique_name("org"),
                gitea_repo: "repo".to_string(),
                language: Some("rust".to_string()),
                created_by: None,
            })
            .await
            .expect("Failed to create project");

        (reviewer, reviewed, project)
    }

    #[tokio::test]
    #[ignore]
    async fn create_and_find_review() {
        let db = get_test_db().await;
        let (reviewer, reviewed, project) = create_test_agents_and_project(&db).await;
        let repo = PostgresAgentReviewRepository::new(db);

        let new_review = NewAgentReview {
            pr_id: 42,
            project_id: project.id,
            reviewer_agent_id: reviewer.id,
            reviewed_agent_id: reviewed.id,
            verdict: ReviewVerdict::Approved,
            reviewer_elo_at_time: 1500,
        };

        // Create
        let review = repo
            .create(&new_review)
            .await
            .expect("Failed to create review");
        assert_eq!(review.verdict, ReviewVerdict::Approved);
        assert_eq!(review.reviewer_elo_at_time, 1500);

        // Find by ID
        let found = repo
            .find_by_id(&review.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.pr_id, 42);

        // Find by PR
        let by_pr = repo
            .find_by_pr(&project.id, 42)
            .await
            .expect("Failed to find");
        assert!(!by_pr.is_empty());

        // Find by reviewer
        let by_reviewer = repo
            .find_by_reviewer(&reviewer.id)
            .await
            .expect("Failed to find");
        assert!(!by_reviewer.is_empty());

        // Find by reviewed
        let by_reviewed = repo
            .find_by_reviewed(&reviewed.id)
            .await
            .expect("Failed to find");
        assert!(!by_reviewed.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn exists_for_pr_and_reviewer() {
        let db = get_test_db().await;
        let (reviewer, reviewed, project) = create_test_agents_and_project(&db).await;
        let repo = PostgresAgentReviewRepository::new(db);

        // Create a review
        repo.create(&NewAgentReview {
            pr_id: 100,
            project_id: project.id,
            reviewer_agent_id: reviewer.id,
            reviewed_agent_id: reviewed.id,
            verdict: ReviewVerdict::Approved,
            reviewer_elo_at_time: 1000,
        })
        .await
        .expect("Failed to create");

        // Check existence
        let exists = repo
            .exists_for_pr_and_reviewer(&project.id, 100, &reviewer.id)
            .await
            .expect("Failed to check");
        assert!(exists);

        // Check non-existent
        let not_exists = repo
            .exists_for_pr_and_reviewer(&project.id, 999, &reviewer.id)
            .await
            .expect("Failed to check");
        assert!(!not_exists);
    }

    #[tokio::test]
    #[ignore]
    async fn count_by_reviewer_since() {
        let db = get_test_db().await;
        let (reviewer, reviewed, project) = create_test_agents_and_project(&db).await;
        let repo = PostgresAgentReviewRepository::new(db);

        // Create a few reviews
        for i in 0..3 {
            repo.create(&NewAgentReview {
                pr_id: 200 + i,
                project_id: project.id,
                reviewer_agent_id: reviewer.id,
                reviewed_agent_id: reviewed.id,
                verdict: ReviewVerdict::Approved,
                reviewer_elo_at_time: 1000,
            })
            .await
            .expect("Failed to create");
        }

        // Count since 1 hour ago
        let count = repo
            .count_by_reviewer_since(&reviewer.id, Utc::now() - Duration::hours(1))
            .await
            .expect("Failed to count");
        assert!(count >= 3);

        // Count since 1 second ago (should catch all we just created)
        let count = repo
            .count_by_reviewer_since(&reviewer.id, Utc::now() - Duration::seconds(1))
            .await
            .expect("Failed to count");
        assert!(count >= 3);
    }

    #[tokio::test]
    #[ignore]
    async fn self_review_rejected() {
        let db = get_test_db().await;
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let project_repo = PostgresProjectRepository::new(db.clone());
        let review_repo = PostgresAgentReviewRepository::new(db);

        let agent = agent_repo
            .create(&NewAgent {
                name: unique_name("self-reviewer"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent");

        let project = project_repo
            .create(&NewProject {
                name: unique_name("self-review-project"),
                description: None,
                gitea_org: unique_name("org"),
                gitea_repo: "repo".to_string(),
                language: None,
                created_by: None,
            })
            .await
            .expect("Failed to create project");

        // Attempt self-review
        let result = review_repo
            .create(&NewAgentReview {
                pr_id: 999,
                project_id: project.id,
                reviewer_agent_id: agent.id,
                reviewed_agent_id: agent.id, // Same agent!
                verdict: ReviewVerdict::Approved,
                reviewer_elo_at_time: 1000,
            })
            .await;

        // Should fail due to DB constraint
        assert!(result.is_err());
    }
}

// ============================================================================
// ELO Event Repository Tests (Reactive ELO)
// ============================================================================

mod elo_event_repo_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn create_and_find_elo_event() {
        let db = get_test_db().await;
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let elo_repo = PostgresEloEventRepository::new(db);

        let agent = agent_repo
            .create(&NewAgent {
                name: unique_name("elo-event-agent"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent");

        let new_event = NewEloEvent {
            agent_id: agent.id,
            event_type: EloEventType::PrMerged,
            delta: 15,
            old_elo: 1000,
            new_elo: 1015,
            reference_id: Some(Uuid::new_v4()),
            details: Some("PR #42 merged".to_string()),
        };

        // Create
        let event = elo_repo
            .create(&new_event)
            .await
            .expect("Failed to create event");
        assert_eq!(event.delta, 15);
        assert_eq!(event.event_type, EloEventType::PrMerged);

        // Find by ID
        let found = elo_repo
            .find_by_id(&event.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.delta, 15);

        // Find by agent
        let by_agent = elo_repo
            .find_by_agent(&agent.id)
            .await
            .expect("Failed to find");
        assert!(!by_agent.is_empty());

        // Find by reference
        let by_ref = elo_repo
            .find_by_reference(new_event.reference_id.unwrap())
            .await
            .expect("Failed to find");
        assert!(!by_ref.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn find_by_agent_paginated() {
        let db = get_test_db().await;
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let elo_repo = PostgresEloEventRepository::new(db);

        let agent = agent_repo
            .create(&NewAgent {
                name: unique_name("paginated-agent"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent");

        // Create several events
        for i in 0..5 {
            elo_repo
                .create(&NewEloEvent {
                    agent_id: agent.id,
                    event_type: EloEventType::PrMerged,
                    delta: 15,
                    old_elo: 1000 + (i * 15),
                    new_elo: 1000 + ((i + 1) * 15),
                    reference_id: None,
                    details: Some(format!("Event {}", i)),
                })
                .await
                .expect("Failed to create");
        }

        // Get first page
        let page1 = elo_repo
            .find_by_agent_paginated(&agent.id, 2, 0)
            .await
            .expect("Failed to find");
        assert_eq!(page1.len(), 2);

        // Get second page
        let page2 = elo_repo
            .find_by_agent_paginated(&agent.id, 2, 2)
            .await
            .expect("Failed to find");
        assert_eq!(page2.len(), 2);

        // Events should be ordered by created_at descending (most recent first)
        assert!(page1[0].created_at >= page1[1].created_at);
    }

    #[tokio::test]
    #[ignore]
    async fn sum_delta_by_agent() {
        let db = get_test_db().await;
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let elo_repo = PostgresEloEventRepository::new(db);

        let agent = agent_repo
            .create(&NewAgent {
                name: unique_name("sum-agent"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent");

        // Create events with different deltas
        elo_repo
            .create(&NewEloEvent {
                agent_id: agent.id,
                event_type: EloEventType::PrMerged,
                delta: 15,
                old_elo: 1000,
                new_elo: 1015,
                reference_id: None,
                details: None,
            })
            .await
            .expect("Failed to create");

        elo_repo
            .create(&NewEloEvent {
                agent_id: agent.id,
                event_type: EloEventType::BugReferenced,
                delta: -15,
                old_elo: 1015,
                new_elo: 1000,
                reference_id: None,
                details: None,
            })
            .await
            .expect("Failed to create");

        elo_repo
            .create(&NewEloEvent {
                agent_id: agent.id,
                event_type: EloEventType::HighEloApproval,
                delta: 5,
                old_elo: 1000,
                new_elo: 1005,
                reference_id: None,
                details: None,
            })
            .await
            .expect("Failed to create");

        // Sum should be 15 - 15 + 5 = 5
        let sum = elo_repo
            .sum_delta_by_agent(&agent.id)
            .await
            .expect("Failed to sum");
        assert_eq!(sum, 5);
    }

    #[tokio::test]
    #[ignore]
    async fn all_event_types_persist_correctly() {
        let db = get_test_db().await;
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let elo_repo = PostgresEloEventRepository::new(db);

        let agent = agent_repo
            .create(&NewAgent {
                name: unique_name("all-types-agent"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent");

        let event_types = vec![
            (EloEventType::PrMerged, 15),
            (EloEventType::HighEloApproval, 5),
            (EloEventType::LongevityBonus, 10),
            (EloEventType::DependentPr, 5),
            (EloEventType::CommitReverted, -30),
            (EloEventType::BugReferenced, -15),
            (EloEventType::PrRejected, -5),
            (EloEventType::LowPeerReviewScore, -10),
            (EloEventType::CodeReplaced, -10),
        ];

        for (event_type, delta) in &event_types {
            let event = elo_repo
                .create(&NewEloEvent {
                    agent_id: agent.id,
                    event_type: *event_type,
                    delta: *delta,
                    old_elo: 1000,
                    new_elo: 1000 + delta,
                    reference_id: None,
                    details: Some(format!("Testing {:?}", event_type)),
                })
                .await
                .expect("Failed to create event");

            // Verify event type persisted and read back correctly
            let found = elo_repo
                .find_by_id(&event.id)
                .await
                .expect("Failed to find")
                .unwrap();
            assert_eq!(found.event_type, *event_type);
            assert_eq!(found.delta, *delta);
        }
    }
}

// ============================================================================
// Engagement Repository Tests
// ============================================================================

mod engagement_repo_tests {
    use super::*;

    async fn create_test_agent(db: &DatabaseConnection) -> Agent {
        let agent_repo = PostgresAgentRepository::new(db.clone());
        agent_repo
            .create(&NewAgent {
                name: unique_name("engage-agent"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent")
    }

    #[tokio::test]
    #[ignore]
    async fn create_and_find_engagement() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresEngagementRepository::new(db);

        let target_id = Uuid::new_v4();
        let new_engagement = NewEngagement {
            agent_id: agent.id,
            target_type: TargetType::Pr,
            target_id,
            engagement_type: EngagementType::Reaction,
            reaction: Some(ReactionType::Laugh),
            body: None,
        };

        // Create
        let engagement = repo
            .create(&new_engagement)
            .await
            .expect("Failed to create engagement");
        assert_eq!(engagement.target_type, TargetType::Pr);
        assert_eq!(engagement.reaction, Some(ReactionType::Laugh));
        assert!(!engagement.gitea_synced);

        // Find by ID
        let found = repo
            .find_by_id(&engagement.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.agent_id, agent.id);
        assert_eq!(found.target_id, target_id);
    }

    #[tokio::test]
    #[ignore]
    async fn find_by_target() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresEngagementRepository::new(db);

        let target_id = Uuid::new_v4();

        // Create multiple engagements on same target
        for reaction in [ReactionType::Laugh, ReactionType::Fire, ReactionType::Skull] {
            repo.create(&NewEngagement {
                agent_id: agent.id,
                target_type: TargetType::Pr,
                target_id,
                engagement_type: EngagementType::Reaction,
                reaction: Some(reaction),
                body: None,
            })
            .await
            .expect("Failed to create");
        }

        // Find by target
        let by_target = repo
            .find_by_target("pr", target_id, 10, 0)
            .await
            .expect("Failed to find");
        assert_eq!(by_target.len(), 3);
    }

    #[tokio::test]
    #[ignore]
    async fn find_by_agent() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresEngagementRepository::new(db);

        // Create engagements
        for i in 0..3 {
            repo.create(&NewEngagement {
                agent_id: agent.id,
                target_type: TargetType::ViralMoment,
                target_id: Uuid::new_v4(),
                engagement_type: EngagementType::Reaction,
                reaction: Some(ReactionType::Fire),
                body: None,
            })
            .await
            .expect("Failed to create");
        }

        // Find by agent
        let by_agent = repo
            .find_by_agent(&agent.id, 10, 0)
            .await
            .expect("Failed to find");
        assert!(by_agent.len() >= 3);
    }

    #[tokio::test]
    #[ignore]
    async fn has_reaction() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresEngagementRepository::new(db);

        let target_id = Uuid::new_v4();

        // Create a reaction
        repo.create(&NewEngagement {
            agent_id: agent.id,
            target_type: TargetType::Pr,
            target_id,
            engagement_type: EngagementType::Reaction,
            reaction: Some(ReactionType::Skull),
            body: None,
        })
        .await
        .expect("Failed to create");

        // Check has_reaction (using "pr" to match TargetType::Pr above)
        let has = repo
            .has_reaction(&agent.id, "pr", target_id, "skull")
            .await
            .expect("Failed to check");
        assert!(has);

        // Check for reaction that doesn't exist
        let not_has = repo
            .has_reaction(&agent.id, "pr", target_id, "laugh")
            .await
            .expect("Failed to check");
        assert!(!not_has);
    }

    #[tokio::test]
    #[ignore]
    async fn mark_synced() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresEngagementRepository::new(db);

        let engagement = repo
            .create(&NewEngagement {
                agent_id: agent.id,
                target_type: TargetType::Pr,
                target_id: Uuid::new_v4(),
                engagement_type: EngagementType::Reaction,
                reaction: Some(ReactionType::Heart),
                body: None,
            })
            .await
            .expect("Failed to create");

        assert!(!engagement.gitea_synced);
        assert!(engagement.gitea_id.is_none());

        // Mark synced
        repo.mark_synced(&engagement.id, 12345)
            .await
            .expect("Failed to mark synced");

        let updated = repo
            .find_by_id(&engagement.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert!(updated.gitea_synced);
        assert_eq!(updated.gitea_id, Some(12345));
    }

    #[tokio::test]
    #[ignore]
    async fn create_comment_engagement() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresEngagementRepository::new(db);

        let engagement = repo
            .create(&NewEngagement {
                agent_id: agent.id,
                target_type: TargetType::Pr,
                target_id: Uuid::new_v4(),
                engagement_type: EngagementType::Comment,
                reaction: None,
                body: Some("This is a great solution!".to_string()),
            })
            .await
            .expect("Failed to create");

        assert_eq!(engagement.engagement_type, EngagementType::Comment);
        assert!(engagement.reaction.is_none());
        assert_eq!(
            engagement.body,
            Some("This is a great solution!".to_string())
        );
    }

    #[tokio::test]
    #[ignore]
    async fn get_counts() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresEngagementRepository::new(db);

        let target_id = Uuid::new_v4();

        // Create various reactions
        repo.create(&NewEngagement {
            agent_id: agent.id,
            target_type: TargetType::ViralMoment,
            target_id,
            engagement_type: EngagementType::Reaction,
            reaction: Some(ReactionType::Laugh),
            body: None,
        })
        .await
        .expect("Failed to create");

        repo.create(&NewEngagement {
            agent_id: agent.id,
            target_type: TargetType::ViralMoment,
            target_id,
            engagement_type: EngagementType::Reaction,
            reaction: Some(ReactionType::Fire),
            body: None,
        })
        .await
        .expect("Failed to create");

        // Get counts (trigger updates via the DB trigger)
        let counts = repo
            .get_counts("viral_moment", target_id)
            .await
            .expect("Failed to get counts");

        // The counts are updated by DB trigger
        assert!(counts.laugh_count >= 1);
        assert!(counts.fire_count >= 1);
        assert!(counts.total_score > 0);
    }
}

// ============================================================================
// Viral Moment Repository Tests
// ============================================================================

mod viral_moment_repo_tests {
    use super::*;

    async fn create_test_agent(db: &DatabaseConnection) -> Agent {
        let agent_repo = PostgresAgentRepository::new(db.clone());
        agent_repo
            .create(&NewAgent {
                name: unique_name("viral-agent"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create agent")
    }

    #[tokio::test]
    #[ignore]
    async fn create_and_find_viral_moment() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        let reference_id = Uuid::new_v4();
        let snapshot = serde_json::json!({
            "agent_name": agent.name,
            "agent_elo": 1000,
            "stderr": "panic: stack overflow"
        });

        let new_moment = NewViralMoment {
            moment_type: MomentType::HallOfShame,
            title: "Agent discovers infinite recursion".to_string(),
            subtitle: Some("Exit code 137".to_string()),
            score: 50,
            agent_ids: vec![agent.id],
            reference_type: ReferenceType::PullRequest,
            reference_id,
            snapshot,
        };

        // Create
        let moment = repo
            .create(&new_moment)
            .await
            .expect("Failed to create moment");
        assert_eq!(moment.moment_type, MomentType::HallOfShame);
        assert_eq!(moment.title, "Agent discovers infinite recursion");
        assert_eq!(moment.score, 50);
        assert!(!moment.promoted);
        assert!(!moment.hidden);

        // Find by ID
        let found = repo
            .find_by_id(&moment.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.title, moment.title);
        assert_eq!(found.agent_ids.len(), 1);
        assert_eq!(found.agent_ids[0], agent.id);
    }

    #[tokio::test]
    #[ignore]
    async fn find_by_type() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        // Create moments of different types
        for (i, moment_type) in [
            MomentType::HallOfShame,
            MomentType::HallOfShame,
            MomentType::AgentDrama,
        ]
        .iter()
        .enumerate()
        {
            repo.create(&NewViralMoment {
                moment_type: *moment_type,
                title: format!("Test Moment {}", i),
                subtitle: None,
                score: (i * 10) as i32,
                agent_ids: vec![agent.id],
                reference_type: ReferenceType::PullRequest,
                reference_id: Uuid::new_v4(),
                snapshot: serde_json::json!({}),
            })
            .await
            .expect("Failed to create");
        }

        // Find by type
        let shame = repo
            .find_by_type(MomentType::HallOfShame, 10, 0)
            .await
            .expect("Failed to find");
        assert!(shame.len() >= 2);

        let drama = repo
            .find_by_type(MomentType::AgentDrama, 10, 0)
            .await
            .expect("Failed to find");
        assert!(drama.len() >= 1);

        // Verify ordering by score
        for i in 1..shame.len() {
            assert!(shame[i - 1].score >= shame[i].score);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn find_top() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        // Create moments with different scores
        for score in [100, 50, 200, 75] {
            repo.create(&NewViralMoment {
                moment_type: MomentType::DavidVsGoliath,
                title: format!("Moment with score {}", score),
                subtitle: None,
                score,
                agent_ids: vec![agent.id],
                reference_type: ReferenceType::Issue,
                reference_id: Uuid::new_v4(),
                snapshot: serde_json::json!({}),
            })
            .await
            .expect("Failed to create");
        }

        // Find top
        let top = repo.find_top(10).await.expect("Failed to find");
        assert!(!top.is_empty());

        // Verify ordering
        for i in 1..top.len() {
            assert!(top[i - 1].score >= top[i].score);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn exists_for_reference() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        let reference_id = Uuid::new_v4();

        // Initially doesn't exist
        let exists = repo
            .exists_for_reference("pull_request", reference_id)
            .await
            .expect("Failed to check");
        assert!(!exists);

        // Create moment
        repo.create(&NewViralMoment {
            moment_type: MomentType::HallOfShame,
            title: "Test".to_string(),
            subtitle: None,
            score: 10,
            agent_ids: vec![agent.id],
            reference_type: ReferenceType::PullRequest,
            reference_id,
            snapshot: serde_json::json!({}),
        })
        .await
        .expect("Failed to create");

        // Now exists (using "pull_request" to match ReferenceType::PullRequest)
        let exists = repo
            .exists_for_reference("pull_request", reference_id)
            .await
            .expect("Failed to check");
        assert!(exists);
    }

    #[tokio::test]
    #[ignore]
    async fn update_score() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        let moment = repo
            .create(&NewViralMoment {
                moment_type: MomentType::HallOfShame,
                title: "Score Test".to_string(),
                subtitle: None,
                score: 10,
                agent_ids: vec![agent.id],
                reference_type: ReferenceType::PullRequest,
                reference_id: Uuid::new_v4(),
                snapshot: serde_json::json!({}),
            })
            .await
            .expect("Failed to create");

        assert_eq!(moment.score, 10);

        // Update score
        repo.update_score(&moment.id, 100)
            .await
            .expect("Failed to update");

        let updated = repo
            .find_by_id(&moment.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(updated.score, 100);
    }

    #[tokio::test]
    #[ignore]
    async fn promote_and_hide() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        let moment = repo
            .create(&NewViralMoment {
                moment_type: MomentType::AgentDrama,
                title: "Moderation Test".to_string(),
                subtitle: None,
                score: 50,
                agent_ids: vec![agent.id],
                reference_type: ReferenceType::PullRequest,
                reference_id: Uuid::new_v4(),
                snapshot: serde_json::json!({}),
            })
            .await
            .expect("Failed to create");

        assert!(!moment.promoted);
        assert!(!moment.hidden);

        // Promote
        repo.set_promoted(&moment.id, true)
            .await
            .expect("Failed to promote");

        let updated = repo
            .find_by_id(&moment.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert!(updated.promoted);

        // Hide
        repo.set_hidden(&moment.id, true)
            .await
            .expect("Failed to hide");

        let updated = repo
            .find_by_id(&moment.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert!(updated.hidden);

        // Hidden moments shouldn't appear in feeds
        let feed = repo
            .find_by_type(MomentType::AgentDrama, 100, 0)
            .await
            .expect("Failed to find");
        assert!(!feed.iter().any(|m| m.id == moment.id));
    }

    #[tokio::test]
    #[ignore]
    async fn find_promoted() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        // Create a promoted moment
        let moment = repo
            .create(&NewViralMoment {
                moment_type: MomentType::LiveBattle,
                title: "Staff Pick".to_string(),
                subtitle: None,
                score: 200,
                agent_ids: vec![agent.id],
                reference_type: ReferenceType::Issue,
                reference_id: Uuid::new_v4(),
                snapshot: serde_json::json!({}),
            })
            .await
            .expect("Failed to create");

        repo.set_promoted(&moment.id, true)
            .await
            .expect("Failed to promote");

        // Find promoted
        let promoted = repo.find_promoted(10).await.expect("Failed to find");
        assert!(promoted.iter().any(|m| m.id == moment.id));
    }

    #[tokio::test]
    #[ignore]
    async fn update_llm_classification() {
        let db = get_test_db().await;
        let agent = create_test_agent(&db).await;
        let repo = PostgresViralMomentRepository::new(db);

        let moment = repo
            .create(&NewViralMoment {
                moment_type: MomentType::HallOfShame,
                title: "LLM Test".to_string(),
                subtitle: None,
                score: 10,
                agent_ids: vec![agent.id],
                reference_type: ReferenceType::PullRequest,
                reference_id: Uuid::new_v4(),
                snapshot: serde_json::json!({}),
            })
            .await
            .expect("Failed to create");

        assert!(!moment.llm_classified);
        assert!(moment.llm_classification.is_none());

        // Update classification
        let classification = serde_json::json!({
            "confidence": 0.95,
            "reasoning": "Clear stack overflow pattern",
            "generated_title": "Recursive nightmare"
        });

        repo.update_llm_classification(&moment.id, classification.clone())
            .await
            .expect("Failed to update");

        let updated = repo
            .find_by_id(&moment.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert!(updated.llm_classified);
        assert!(updated.llm_classification.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn multiple_agents_in_moment() {
        let db = get_test_db().await;
        let agent_repo = PostgresAgentRepository::new(db.clone());
        let repo = PostgresViralMomentRepository::new(db.clone());

        // Create multiple agents
        let agent1 = agent_repo
            .create(&NewAgent {
                name: unique_name("multi-1"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea-1"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create");

        let agent2 = agent_repo
            .create(&NewAgent {
                name: unique_name("multi-2"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea-2"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create");

        let agent3 = agent_repo
            .create(&NewAgent {
                name: unique_name("multi-3"),
                api_key_hash: format!("hash-{}", Uuid::new_v4()),
                gitea_username: unique_name("gitea-3"),
                gitea_token_encrypted: vec![],
                claim_code: format!("claim-{}", Uuid::new_v4()),
            })
            .await
            .expect("Failed to create");

        // Create moment with multiple agents
        let moment = repo
            .create(&NewViralMoment {
                moment_type: MomentType::LiveBattle,
                title: "3-way race!".to_string(),
                subtitle: None,
                score: 100,
                agent_ids: vec![agent1.id, agent2.id, agent3.id],
                reference_type: ReferenceType::Issue,
                reference_id: Uuid::new_v4(),
                snapshot: serde_json::json!({}),
            })
            .await
            .expect("Failed to create");

        let found = repo
            .find_by_id(&moment.id)
            .await
            .expect("Failed to find")
            .unwrap();
        assert_eq!(found.agent_ids.len(), 3);
        assert!(found.agent_ids.contains(&agent1.id));
        assert!(found.agent_ids.contains(&agent2.id));
        assert!(found.agent_ids.contains(&agent3.id));
    }
}
