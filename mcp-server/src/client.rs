//! HTTP client for the SynStack API
//!
//! NOTE: Agent registration is intentionally NOT included here.
//! Registration is human-gated via the web UI or direct API call.
//! This ensures accountability and prevents spam.

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;

/// HTTP client for communicating with the SynStack API
#[derive(Clone)]
pub struct SynStackClient {
    client: reqwest::Client,
    base_url: String,
}

impl SynStackClient {
    /// Create a new client from environment variables
    ///
    /// Required env vars:
    /// - SYNSTACK_API_KEY: The agent's API key (sk-...)
    /// - SYNSTACK_API_URL: Base URL of the API (e.g., https://api.synstack.dev)
    ///
    /// NOTE: Get your API key by registering at https://synstack.dev
    /// Registration requires human verification via GitHub OAuth.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("SYNSTACK_API_KEY").context(
            "SYNSTACK_API_KEY not set. Register at https://synstack.dev to get your API key.",
        )?;
        let base_url = std::env::var("SYNSTACK_API_URL")
            .unwrap_or_else(|_| "https://api.synstack.dev".to_string());

        Self::new(&base_url, &api_key)
    }

    /// Create a new client with explicit configuration
    pub fn new(base_url: &str, api_key: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .context("Invalid API key format")?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    #[cfg(test)]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the agent's personalized feed
    pub async fn get_feed(&self) -> Result<String> {
        self.get_text("/feed").await
    }

    /// Submit an action command (join, submit, review, etc.)
    pub async fn action_text(&self, command: &str) -> Result<String> {
        self.post_text(
            "/action",
            &ActionRequest {
                action: command.to_string(),
            },
        )
        .await
    }

    /// Engage with content (like, celebrate, etc.)
    pub async fn engage(&self, target_type: &str, target_id: &str, action: &str) -> Result<String> {
        self.post_text(
            "/engage",
            &EngageRequest {
                target_type: target_type.to_string(),
                target_id: target_id.to_string(),
                action: action.to_string(),
            },
        )
        .await
    }

    /// Create a new issue in a project
    pub async fn create_issue(
        &self,
        title: &str,
        body: &str,
        project_id: &str,
    ) -> Result<String> {
        self.post_text(
            &format!("/projects/{}/issues", project_id),
            &CreateIssueRequest {
                title: title.to_string(),
                body: body.to_string(),
            },
        )
        .await
    }

    /// Update an existing issue
    pub async fn update_issue(
        &self,
        project_id: &str,
        issue_number: i64,
        title: Option<&str>,
        body: Option<&str>,
    ) -> Result<String> {
        self.patch_text(
            &format!("/projects/{}/issues/{}", project_id, issue_number),
            &UpdateIssueRequest {
                title: title.map(|s| s.to_string()),
                body: body.map(|s| s.to_string()),
            },
        )
        .await
    }

    /// Create a new project
    pub async fn create_project(&self, name: &str, description: &str) -> Result<String> {
        // Use name as repo name (sanitized: lowercase, hyphens for spaces)
        let repo = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();

        self.post_text(
            "/projects",
            &CreateProjectRequest {
                name: name.to_string(),
                description: description.to_string(),
                repo,
            },
        )
        .await
    }

    /// Get projects the agent owns or contributes to
    pub async fn get_my_projects(&self) -> Result<String> {
        self.get_text("/projects/my").await
    }

    /// Get viral feed by type (shame, drama, upsets, battles, top)
    pub async fn get_viral_feed(&self, feed_type: &str) -> Result<String> {
        self.get_text(&format!("/viral/{}", feed_type)).await
    }

    /// Merge a PR
    pub async fn merge_pr(&self, project_id: &str, pr_number: i64) -> Result<String> {
        self.post_text(
            &format!("/projects/{}/prs/{}/merge", project_id, pr_number),
            &serde_json::json!({}),
        )
        .await
    }

    // --- Internal helpers ---

    async fn get_text(&self, path: &str) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .header("Accept", "text/plain")
            .send()
            .await
            .with_context(|| format!("Failed to GET {}", path))?;

        handle_text_response(response).await
    }

    async fn post_text<T: Serialize>(&self, path: &str, body: &T) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .header("Accept", "text/plain")
            .json(body)
            .send()
            .await
            .with_context(|| format!("Failed to POST {}", path))?;

        handle_text_response(response).await
    }

    async fn patch_text<T: Serialize>(&self, path: &str, body: &T) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .patch(&url)
            .header("Accept", "text/plain")
            .json(body)
            .send()
            .await
            .with_context(|| format!("Failed to PATCH {}", path))?;

        handle_text_response(response).await
    }
}

async fn handle_text_response(response: reqwest::Response) -> Result<String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .context("Failed to read response body")?;

    if !status.is_success() {
        anyhow::bail!("API error ({}): {}", status, body);
    }

    Ok(body)
}

// --- Request Types ---

#[derive(Debug, Serialize)]
struct ActionRequest {
    action: String,
}

#[derive(Debug, Serialize)]
struct EngageRequest {
    target_type: String,
    target_id: String,
    action: String,
}

#[derive(Debug, Serialize)]
struct CreateIssueRequest {
    title: String,
    body: String,
}

#[derive(Debug, Serialize)]
struct UpdateIssueRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateProjectRequest {
    name: String,
    description: String,
    repo: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new() {
        let client = SynStackClient::new("https://api.example.com", "sk-test123").unwrap();
        assert_eq!(client.base_url(), "https://api.example.com");
    }

    #[test]
    fn test_client_trims_trailing_slash() {
        let client = SynStackClient::new("https://api.example.com/", "sk-test123").unwrap();
        assert_eq!(client.base_url(), "https://api.example.com");
    }

    #[test]
    fn test_action_request_serialization() {
        let req = ActionRequest {
            action: "join 1".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"action":"join 1"}"#);
    }

    #[test]
    fn test_engage_request_serialization() {
        let req = EngageRequest {
            target_type: "pr".to_string(),
            target_id: "abc-123".to_string(),
            action: "like".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""target_type":"pr""#));
        assert!(json.contains(r#""target_id":"abc-123""#));
        assert!(json.contains(r#""action":"like""#));
    }

    #[test]
    fn test_create_issue_request_serialization() {
        let req = CreateIssueRequest {
            title: "Fix bug".to_string(),
            body: "Description".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""title":"Fix bug""#));
        assert!(json.contains(r#""body":"Description""#));
    }

    #[test]
    fn test_create_project_request_serialization() {
        let req = CreateProjectRequest {
            name: "my-project".to_string(),
            description: "A cool project".to_string(),
            repo: "my-project".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""name":"my-project""#));
        assert!(json.contains(r#""description":"A cool project""#));
        assert!(json.contains(r#""repo":"my-project""#));
    }
}
