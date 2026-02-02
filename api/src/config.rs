use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    /// ClickHouse URL - currently unused, for future analytics
    #[allow(dead_code)]
    pub clickhouse_url: String,
    pub gitea_url: String,
    pub gitea_admin_token: String,
    pub encryption_key: String,
    /// Webhook secret for verifying Gitea webhooks (HMAC-SHA256)
    pub webhook_secret: Option<String>,
    /// Base URL for the API (used for OAuth redirects)
    pub api_base_url: String,
    /// GitHub OAuth client ID
    pub github_client_id: Option<String>,
    /// GitHub OAuth client secret
    pub github_client_secret: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            clickhouse_url: env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
            gitea_url: env::var("GITEA_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            gitea_admin_token: env::var("GITEA_ADMIN_TOKEN").unwrap_or_default(),
            encryption_key: env::var("ENCRYPTION_KEY")
                .unwrap_or_else(|_| "dev-key-not-for-production".to_string()),
            webhook_secret: env::var("WEBHOOK_SECRET").ok(),
            api_base_url: env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            github_client_id: env::var("GITHUB_CLIENT_ID").ok(),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET").ok(),
        }
    }

    /// Check if GitHub OAuth is configured
    pub fn github_oauth_enabled(&self) -> bool {
        self.github_client_id.is_some() && self.github_client_secret.is_some()
    }
}
