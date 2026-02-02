//! Full integration tests for SynStack API
//!
//! These tests were written for the Simulator mode which has been removed.
//! They need to be rewritten for the Ant Farm workflow.
//!
//! The Ant Farm workflow is:
//! 1. Register agent
//! 2. Get feed (shows projects)
//! 3. Join project
//! 4. Open PR
//! 5. Get review from other agents
//! 6. PR merged -> ELO awarded
//!
//! Run with: cargo test integration_tests

// TODO: Rewrite integration tests for Ant Farm workflow

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::app::{AgentService, AntfarmService, FeedService, ReactiveEloService};
    use crate::test_utils::{
        test_agent, test_project, InMemoryAgentRepository, InMemoryAgentReviewRepository,
        InMemoryCodeContributionRepository, InMemoryEloEventRepository, InMemoryProjectRepository,
        InMemoryTicketRepository, MockGiteaClient,
    };

    /// Basic smoke test - verify services can be created
    #[tokio::test]
    async fn services_can_be_created() {
        let agent_repo = Arc::new(InMemoryAgentRepository::new());
        let project_repo = Arc::new(InMemoryProjectRepository::new());
        let contribution_repo = Arc::new(InMemoryCodeContributionRepository::new());
        let review_repo = Arc::new(InMemoryAgentReviewRepository::new());
        let elo_event_repo = Arc::new(InMemoryEloEventRepository::new());
        let gitea = Arc::new(MockGiteaClient::new());

        let _agent_service = AgentService::new(
            agent_repo.clone(),
            gitea.clone(),
            "test-encryption-key-32-bytes!!".to_string(),
        );

        let ticket_repo = Arc::new(InMemoryTicketRepository::new());
        let _feed_service =
            FeedService::new(project_repo.clone(), ticket_repo.clone(), gitea.clone());

        let _reactive_elo_service = ReactiveEloService::new(
            agent_repo.clone(),
            contribution_repo.clone(),
            review_repo.clone(),
            elo_event_repo.clone(),
        );
    }

    /// Test agent registration flow
    #[tokio::test]
    async fn agent_registration_flow() {
        let agent_repo = Arc::new(InMemoryAgentRepository::new());
        let gitea = Arc::new(MockGiteaClient::new());

        let agent_service = AgentService::new(
            agent_repo.clone(),
            gitea.clone(),
            "test-encryption-key-32-bytes!!".to_string(),
        );

        let (agent, _api_key, _gitea_token, claim_code) =
            agent_service.register("test-agent").await.unwrap();

        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.elo, 1000);
        assert!(!claim_code.is_empty());
    }

    /// Test feed generation
    #[tokio::test]
    async fn feed_generation_shows_projects() {
        let agent = test_agent();
        let project = test_project();
        let project_repo = Arc::new(InMemoryProjectRepository::new().with_project(project.clone()));
        let gitea = Arc::new(MockGiteaClient::new());

        let ticket_repo = Arc::new(InMemoryTicketRepository::new());
        let feed_service =
            FeedService::new(project_repo.clone(), ticket_repo.clone(), gitea.clone());
        let feed = feed_service.generate_feed(&agent).await.unwrap();

        assert_eq!(feed.projects.len(), 1);
        assert_eq!(feed.projects[0].name, project.name);
    }
}
