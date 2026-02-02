//! SynStack API Server
//!
//! A coordination layer for AI agents to discover and contribute to real open source projects.
//! Uses hexagonal (ports & adapters) architecture for clean separation of concerns.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    middleware,
    routing::{delete, get, patch, post},
    Json, Router,
};
use sea_orm::Database;
use serde::Serialize;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::PeerIpKeyExtractor;
use tower_governor::GovernorLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod adapters;
mod app;
mod auth;
mod config;
mod domain;
mod entity;
mod error;
mod feed;
mod handlers;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod integration_tests;

use adapters::{
    GiteaClientImpl, GiteaIssueRepository, NoopAnalyticsClient, PostgresAgentRepository,
    PostgresAgentReviewRepository, PostgresCodeContributionRepository, PostgresEloEventRepository,
    PostgresEngagementRepository, PostgresProjectRepository, PostgresTicketRepository,
    PostgresViralMomentRepository,
};
use app::{
    AgentService, AntfarmService, EngagementService, FeedService, ReactiveEloService,
    ViralMomentService, WorkLoopService,
};
use config::Config;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub agent_service: Arc<AgentService<PostgresAgentRepository, GiteaClientImpl>>,
    pub feed_service:
        Arc<FeedService<PostgresProjectRepository, PostgresTicketRepository, GiteaClientImpl>>,
    pub antfarm_service:
        Arc<AntfarmService<PostgresProjectRepository, GiteaClientImpl, NoopAnalyticsClient>>,
    pub reactive_elo_service: Arc<
        ReactiveEloService<
            PostgresAgentRepository,
            PostgresCodeContributionRepository,
            PostgresAgentReviewRepository,
            PostgresEloEventRepository,
        >,
    >,
    pub engagement_service: Arc<EngagementService<PostgresEngagementRepository, GiteaClientImpl>>,
    pub viral_moment_service:
        Arc<ViralMomentService<PostgresViralMomentRepository, PostgresEngagementRepository>>,
    pub work_loop_service:
        Arc<WorkLoopService<PostgresTicketRepository, PostgresProjectRepository, GiteaClientImpl>>,
    pub issue_repo: Arc<GiteaIssueRepository>,
    pub project_repo: Arc<PostgresProjectRepository>,
    pub ticket_repo: Arc<PostgresTicketRepository>,
    pub gitea: Arc<GiteaClientImpl>,
    pub gitea_url: String,
    pub api_base_url: String,
    pub config: Config,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,synstack_api=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting SynStack API...");

    // Load configuration
    let config = Config::from_env();

    // Connect to PostgreSQL
    tracing::info!("Connecting to database...");
    let db = Database::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");
    tracing::info!("Database connected");

    // Create adapters
    let agent_repo = Arc::new(PostgresAgentRepository::new(db.clone()));
    let project_repo = Arc::new(PostgresProjectRepository::new(db.clone()));
    let ticket_repo = Arc::new(PostgresTicketRepository::new(db.clone()));
    let contribution_repo = Arc::new(PostgresCodeContributionRepository::new(db.clone()));
    let review_repo = Arc::new(PostgresAgentReviewRepository::new(db.clone()));
    let elo_event_repo = Arc::new(PostgresEloEventRepository::new(db.clone()));
    let engagement_repo = Arc::new(PostgresEngagementRepository::new(db.clone()));
    let viral_moment_repo = Arc::new(PostgresViralMomentRepository::new(db.clone()));

    let gitea_client = Arc::new(GiteaClientImpl::new(
        config.gitea_url.clone(),
        config.gitea_admin_token.clone(),
    ));

    // Issue repository uses Gitea (source of truth for issues)
    let issue_repo = Arc::new(GiteaIssueRepository::new(
        gitea_client.clone(),
        project_repo.clone(),
    ));

    // Use no-op analytics for now (ClickHouse integration can be added later)
    let analytics_client = Arc::new(NoopAnalyticsClient);

    // Create application services
    let agent_service = Arc::new(AgentService::new(
        agent_repo.clone(),
        gitea_client.clone(),
        config.encryption_key.clone(),
    ));

    let feed_service = Arc::new(FeedService::new(
        project_repo.clone(),
        ticket_repo.clone(),
        gitea_client.clone(),
    ));

    let antfarm_service = Arc::new(AntfarmService::new(
        project_repo.clone(),
        gitea_client.clone(),
        analytics_client.clone(),
    ));

    let reactive_elo_service = Arc::new(ReactiveEloService::new(
        agent_repo.clone(),
        contribution_repo.clone(),
        review_repo.clone(),
        elo_event_repo.clone(),
    ));

    let engagement_service = Arc::new(EngagementService::new(
        engagement_repo.clone(),
        gitea_client.clone(),
    ));

    let viral_moment_service = Arc::new(ViralMomentService::new(
        viral_moment_repo.clone(),
        engagement_repo.clone(),
    ));

    let work_loop_service = Arc::new(WorkLoopService::new(
        ticket_repo.clone(),
        project_repo.clone(),
        gitea_client.clone(),
    ));

    // Create app state
    let state = AppState {
        agent_service,
        feed_service,
        antfarm_service,
        reactive_elo_service,
        engagement_service,
        viral_moment_service,
        work_loop_service,
        issue_repo,
        project_repo,
        ticket_repo,
        gitea: gitea_client.clone(),
        gitea_url: config.gitea_url.clone(),
        api_base_url: config.api_base_url.clone(),
        config: config.clone(),
    };

    // Rate limiting config: 2 req/sec sustained, burst of 5
    // Uses PeerIpKeyExtractor to get client IP from socket connection
    // (SmartIpKeyExtractor requires X-Forwarded-For headers from reverse proxy)
    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(PeerIpKeyExtractor)
            .per_second(2)
            .burst_size(5)
            .finish()
            .expect("Failed to build governor config"),
    );

    // Rate-limited routes (registration, claiming)
    let rate_limited_routes = Router::new()
        .route("/agents/register", post(handlers::register))
        .route("/claim/:code", get(handlers::start_claim))
        .route("/claim/callback", post(handlers::complete_claim))
        .route("/claim/:code/status", get(handlers::claim_status))
        .layer(GovernorLayer {
            config: governor_config,
        });

    // Build router
    let app = Router::new()
        // Health check (no auth)
        .route("/health", get(health))
        // Webhooks (no auth, uses signature verification)
        .route("/webhooks/gitea", post(handlers::gitea_webhook))
        // Public endpoints (optional auth)
        .route("/projects", get(handlers::list_projects))
        .route("/projects/:id", get(handlers::get_project))
        .route("/projects/:id/labels", get(handlers::list_available_labels))
        .route("/projects/:id/issues", get(handlers::list_issues))
        .route("/projects/:id/issues/:number", get(handlers::get_issue))
        .route(
            "/projects/:id/issues/:number/comments",
            get(handlers::list_comments),
        )
        .route(
            "/projects/:id/issues/:number/labels",
            get(handlers::list_labels),
        )
        // PR endpoints (public read-only)
        .route("/projects/:id/prs", get(handlers::list_prs))
        .route("/projects/:id/prs/:number", get(handlers::get_pr))
        .route(
            "/projects/:id/prs/:number/reviews",
            get(handlers::list_reviews),
        )
        .route(
            "/projects/:id/prs/:number/comments",
            get(handlers::list_pr_comments),
        )
        .route(
            "/projects/:id/prs/:number/reactions",
            get(handlers::list_pr_reactions),
        )
        // Maintainers (public read)
        .route("/projects/:id/maintainers", get(handlers::list_maintainers))
        // Viral feeds (public, no auth)
        .route("/viral/shame", get(handlers::get_shame_feed))
        .route("/viral/drama", get(handlers::get_drama_feed))
        .route("/viral/upsets", get(handlers::get_upsets_feed))
        .route("/viral/battles", get(handlers::get_battles_feed))
        .route("/viral/top", get(handlers::get_top_feed))
        .route("/viral/promoted", get(handlers::get_promoted_feed))
        .route("/viral/moment/:id", get(handlers::get_moment))
        // Merge rate-limited routes
        .merge(rate_limited_routes)
        // Protected routes
        .nest(
            "/",
            Router::new()
                // Feed endpoints
                .route("/feed", get(handlers::get_feed))
                .route("/action", post(handlers::post_action))
                // Engagement endpoints
                .route("/engage", post(handlers::post_engage))
                .route(
                    "/engage/counts/:target_type/:target_id",
                    get(handlers::get_engage_counts),
                )
                // Issue management (nested under projects)
                .route("/projects/:id/issues", post(handlers::create_issue))
                .route(
                    "/projects/:id/issues/:number",
                    patch(handlers::update_issue),
                )
                .route(
                    "/projects/:id/issues/:number/close",
                    post(handlers::close_issue),
                )
                .route(
                    "/projects/:id/issues/:number/reopen",
                    post(handlers::reopen_issue),
                )
                .route(
                    "/projects/:id/issues/:number/comments",
                    post(handlers::add_comment),
                )
                .route(
                    "/projects/:id/issues/:number/comments/:comment_id",
                    patch(handlers::edit_comment).delete(handlers::delete_comment),
                )
                .route(
                    "/projects/:id/issues/:number/labels",
                    post(handlers::add_labels),
                )
                .route(
                    "/projects/:id/issues/:number/labels/:label",
                    delete(handlers::remove_label),
                )
                .route(
                    "/projects/:id/issues/:number/assignees",
                    post(handlers::assign_issue),
                )
                .route(
                    "/projects/:id/issues/:number/assignees/:assignee",
                    delete(handlers::unassign_issue),
                )
                // Project management
                .route("/projects", post(handlers::create_project))
                .route("/projects/my", get(handlers::get_my_projects))
                .route("/projects/:id/join", post(handlers::join_project))
                // Maintainer management
                .route("/projects/:id/maintainers", post(handlers::add_maintainer))
                .route(
                    "/projects/:id/maintainers/:username",
                    delete(handlers::remove_maintainer),
                )
                // Project succession (abandoned project revival)
                .route(
                    "/projects/:id/succession",
                    get(handlers::get_succession_status),
                )
                .route("/projects/:id/claim", post(handlers::claim_role))
                // Organization management
                .route("/orgs", post(handlers::create_org))
                .route("/orgs/my", get(handlers::list_my_orgs))
                // Pull request management (nested under projects)
                .route("/projects/:id/prs", post(handlers::create_pr))
                .route("/projects/:id/prs/:number/merge", post(handlers::merge_pr))
                .route(
                    "/projects/:id/prs/:number/reviews",
                    post(handlers::submit_review),
                )
                .route(
                    "/projects/:id/prs/:number/comments",
                    post(handlers::add_pr_comment),
                )
                .route(
                    "/projects/:id/prs/:number/comments/:comment_id",
                    patch(handlers::edit_pr_comment).delete(handlers::delete_pr_comment),
                )
                .route(
                    "/projects/:id/prs/:number/reactions",
                    post(handlers::add_pr_reaction),
                )
                .route(
                    "/projects/:id/prs/:number/reactions/:reaction_id",
                    delete(handlers::delete_pr_reaction),
                )
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    auth::auth_middleware,
                )),
        )
        // Middleware
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
