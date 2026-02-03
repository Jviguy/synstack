//! Gitea API client implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use urlencoding::encode;

use crate::domain::ports::{
    GiteaBranch, GiteaClient, GiteaCombinedStatus, GiteaComment, GiteaCommit, GiteaIssue,
    GiteaIssueComment, GiteaLabel, GiteaOrg, GiteaPRBranch, GiteaPRReview, GiteaPullRequest,
    GiteaReaction, GiteaRepo, GiteaStatus, GiteaUser,
};
use crate::error::GiteaError;

/// Implementation of the Gitea API client
pub struct GiteaClientImpl {
    http: Client,
    base_url: String,
    admin_token: String,
}

impl GiteaClientImpl {
    pub fn new(base_url: String, admin_token: String) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            admin_token,
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.base_url, path)
    }

    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, GiteaError> {
        let status = response.status();

        if status.is_success() {
            response
                .json()
                .await
                .map_err(|e| GiteaError::Deserialization(e.to_string()))
        } else if status.as_u16() == 401 {
            Err(GiteaError::Unauthorized)
        } else if status.as_u16() == 429 {
            Err(GiteaError::RateLimited)
        } else {
            let message = response.text().await.unwrap_or_default();
            Err(GiteaError::Api {
                status: status.as_u16(),
                message,
            })
        }
    }

    async fn handle_empty_response(&self, response: reqwest::Response) -> Result<(), GiteaError> {
        let status = response.status();

        if status.is_success() {
            Ok(())
        } else if status.as_u16() == 401 {
            Err(GiteaError::Unauthorized)
        } else if status.as_u16() == 429 {
            Err(GiteaError::RateLimited)
        } else {
            let message = response.text().await.unwrap_or_default();
            Err(GiteaError::Api {
                status: status.as_u16(),
                message,
            })
        }
    }
}

/// Request types for Gitea API
#[derive(Serialize)]
struct CreateUserRequest<'a> {
    username: &'a str,
    email: &'a str,
    password: &'a str,
    must_change_password: bool,
}

#[derive(Serialize)]
struct CreateTokenRequest<'a> {
    name: &'a str,
    scopes: Vec<&'a str>,
}

#[derive(Deserialize)]
struct CreateTokenResponse {
    sha1: String,
}

#[derive(Serialize)]
struct CreateOrgRequest<'a> {
    username: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
}

#[derive(Serialize)]
struct CreateRepoRequest<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    private: bool,
    auto_init: bool,
}

#[derive(Serialize)]
struct ForkRepoRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    organization: Option<&'a str>,
}

#[derive(Serialize)]
struct CreatePRRequest<'a> {
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
    head: &'a str,
    base: &'a str,
}

#[derive(Serialize)]
struct MergePRRequest<'a> {
    #[serde(rename = "Do")]
    do_merge: &'a str,
}

#[derive(Serialize)]
struct CreateWebhookRequest<'a> {
    #[serde(rename = "type")]
    hook_type: &'a str,
    config: WebhookConfig<'a>,
    events: Vec<String>,
    active: bool,
}

#[derive(Serialize)]
struct WebhookConfig<'a> {
    url: &'a str,
    content_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret: Option<&'a str>,
}

#[derive(Deserialize)]
struct WebhookResponse {
    id: i64,
}

/// Gitea team response for org team operations
#[derive(Deserialize)]
struct GiteaTeamResponse {
    id: i64,
    name: String,
}

#[derive(Serialize)]
struct AddCollaboratorRequest<'a> {
    permission: &'a str,
}

/// Response types from Gitea API
#[derive(Deserialize)]
struct GiteaUserResponse {
    id: i64,
    login: String,
    email: String,
    full_name: Option<String>,
}

impl From<GiteaUserResponse> for GiteaUser {
    fn from(r: GiteaUserResponse) -> Self {
        GiteaUser {
            id: r.id,
            login: r.login,
            email: r.email,
            full_name: r.full_name,
        }
    }
}

#[derive(Deserialize)]
struct GiteaOrgResponse {
    id: i64,
    name: String,
    full_name: Option<String>,
    description: Option<String>,
}

impl From<GiteaOrgResponse> for GiteaOrg {
    fn from(r: GiteaOrgResponse) -> Self {
        GiteaOrg {
            id: r.id,
            name: r.name,
            full_name: r.full_name,
            description: r.description,
        }
    }
}

#[derive(Deserialize)]
struct GiteaRepoResponse {
    id: i64,
    name: String,
    full_name: String,
    description: Option<String>,
    clone_url: String,
    ssh_url: String,
    html_url: String,
    default_branch: String,
    private: bool,
}

impl From<GiteaRepoResponse> for GiteaRepo {
    fn from(r: GiteaRepoResponse) -> Self {
        GiteaRepo {
            id: r.id,
            name: r.name,
            full_name: r.full_name,
            description: r.description,
            clone_url: r.clone_url,
            ssh_url: r.ssh_url,
            html_url: r.html_url,
            default_branch: r.default_branch,
            private: r.private,
        }
    }
}

#[derive(Deserialize)]
struct GiteaBranchResponse {
    name: String,
    commit: GiteaCommitResponse,
}

#[derive(Deserialize)]
struct GiteaCommitResponse {
    id: String,
    message: String,
}

impl From<GiteaBranchResponse> for GiteaBranch {
    fn from(r: GiteaBranchResponse) -> Self {
        GiteaBranch {
            name: r.name,
            commit: GiteaCommit {
                id: r.commit.id,
                message: r.commit.message,
            },
        }
    }
}

#[derive(Deserialize)]
struct GiteaPRResponse {
    id: i64,
    number: i64,
    title: String,
    body: Option<String>,
    state: String,
    html_url: String,
    head: GiteaPRBranchResponse,
    base: GiteaPRBranchResponse,
    merged: bool,
    user: Option<GiteaUserResponse>,
}

#[derive(Deserialize)]
struct GiteaPRBranchResponse {
    #[serde(rename = "ref")]
    ref_name: String,
    sha: String,
}

impl From<GiteaPRResponse> for GiteaPullRequest {
    fn from(r: GiteaPRResponse) -> Self {
        GiteaPullRequest {
            id: r.id,
            number: r.number,
            title: r.title,
            body: r.body,
            state: r.state,
            html_url: r.html_url,
            head: GiteaPRBranch {
                ref_name: r.head.ref_name,
                sha: r.head.sha,
            },
            base: GiteaPRBranch {
                ref_name: r.base.ref_name,
                sha: r.base.sha,
            },
            merged: r.merged,
            user: r.user.map(|u| u.into()),
        }
    }
}

#[derive(Deserialize)]
struct GiteaCommentResponse {
    id: i64,
    body: String,
    user: GiteaUserResponse,
    created_at: String,
    updated_at: String,
}

impl From<GiteaCommentResponse> for GiteaComment {
    fn from(r: GiteaCommentResponse) -> Self {
        GiteaComment {
            id: r.id,
            body: r.body,
            user: r.user.into(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Deserialize)]
struct GiteaPRReviewResponse {
    id: i64,
    user: GiteaUserResponse,
    state: String,
    body: Option<String>,
    submitted_at: Option<String>,
}

impl From<GiteaPRReviewResponse> for GiteaPRReview {
    fn from(r: GiteaPRReviewResponse) -> Self {
        GiteaPRReview {
            id: r.id,
            user: r.user.into(),
            state: r.state,
            body: r.body,
            submitted_at: r.submitted_at,
        }
    }
}

#[derive(Deserialize)]
struct GiteaCombinedStatusResponse {
    state: String,
    statuses: Vec<GiteaStatusResponse>,
}

#[derive(Deserialize)]
struct GiteaStatusResponse {
    state: String,
    context: String,
    description: Option<String>,
    target_url: Option<String>,
}

impl From<GiteaCombinedStatusResponse> for GiteaCombinedStatus {
    fn from(r: GiteaCombinedStatusResponse) -> Self {
        GiteaCombinedStatus {
            state: r.state,
            statuses: r
                .statuses
                .into_iter()
                .map(|s| GiteaStatus {
                    state: s.state,
                    context: s.context,
                    description: s.description,
                    target_url: s.target_url,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct CreateCommentRequest<'a> {
    body: &'a str,
}

#[derive(Serialize)]
struct SubmitReviewRequest<'a> {
    event: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
}

#[derive(Serialize)]
struct CreateReactionRequest<'a> {
    content: &'a str,
}

#[derive(Deserialize)]
struct GiteaReactionResponse {
    #[serde(default)]
    id: Option<i64>,
    user: GiteaUserResponse,
    content: String,
    created_at: String,
}

impl From<GiteaReactionResponse> for GiteaReaction {
    fn from(r: GiteaReactionResponse) -> Self {
        GiteaReaction {
            // Gitea reactions don't have IDs - use 0 as placeholder
            id: r.id.unwrap_or(0),
            user: r.user.into(),
            content: r.content,
            created_at: r.created_at,
        }
    }
}

#[async_trait]
impl GiteaClient for GiteaClientImpl {
    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<GiteaUser, GiteaError> {
        let resp = self
            .http
            .post(self.api_url("/admin/users"))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&CreateUserRequest {
                username,
                email,
                password,
                must_change_password: false,
            })
            .send()
            .await?;

        let user: GiteaUserResponse = self.handle_response(resp).await?;
        Ok(user.into())
    }

    async fn get_user(&self, username: &str) -> Result<GiteaUser, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/users/{}", username)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Err(GiteaError::UserNotFound(username.to_string()));
        }

        let user: GiteaUserResponse = self.handle_response(resp).await?;
        Ok(user.into())
    }

    async fn create_access_token(
        &self,
        username: &str,
        password: &str,
        token_name: &str,
    ) -> Result<String, GiteaError> {
        // Gitea requires basic auth with user's credentials to create tokens
        let resp = self
            .http
            .post(self.api_url(&format!("/users/{}/tokens", username)))
            .basic_auth(username, Some(password))
            .json(&CreateTokenRequest {
                name: token_name,
                scopes: vec![
                    "write:repository",
                    "write:user",
                    "write:issue",
                    "write:organization",
                ],
            })
            .send()
            .await?;

        let token: CreateTokenResponse = self.handle_response(resp).await?;
        Ok(token.sha1)
    }

    async fn delete_access_token(
        &self,
        username: &str,
        token_name: &str,
    ) -> Result<(), GiteaError> {
        let resp = self
            .http
            .delete(self.api_url(&format!("/users/{}/tokens/{}", username, token_name)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn create_org(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<GiteaOrg, GiteaError> {
        let resp = self
            .http
            .post(self.api_url("/orgs"))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&CreateOrgRequest {
                username: name,
                description,
            })
            .send()
            .await?;

        let org: GiteaOrgResponse = self.handle_response(resp).await?;
        Ok(org.into())
    }

    async fn get_org(&self, name: &str) -> Result<GiteaOrg, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/orgs/{}", name)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Err(GiteaError::OrgNotFound(name.to_string()));
        }

        let org: GiteaOrgResponse = self.handle_response(resp).await?;
        Ok(org.into())
    }

    async fn add_org_member(&self, org: &str, username: &str) -> Result<(), GiteaError> {
        let resp = self
            .http
            .put(self.api_url(&format!("/orgs/{}/members/{}", org, username)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn add_org_owner(&self, org: &str, username: &str) -> Result<(), GiteaError> {
        // Get the org's teams to find the "Owners" team
        let resp = self
            .http
            .get(self.api_url(&format!("/orgs/{}/teams", org)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let teams: Vec<GiteaTeamResponse> = self.handle_response(resp).await?;

        // Find the Owners team
        let owners_team =
            teams
                .iter()
                .find(|t| t.name == "Owners")
                .ok_or_else(|| GiteaError::Api {
                    status: 404,
                    message: format!("Owners team not found for org {}", org),
                })?;

        // Add user to the Owners team
        let resp = self
            .http
            .put(self.api_url(&format!("/teams/{}/members/{}", owners_team.id, username)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn create_team(
        &self,
        org: &str,
        name: &str,
        description: Option<&str>,
        permission: &str,
    ) -> Result<i64, GiteaError> {
        #[derive(Serialize)]
        struct CreateTeamRequest<'a> {
            name: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<&'a str>,
            permission: &'a str,
            includes_all_repositories: bool,
            /// Required by Gitea - list of units the team has access to
            units: Vec<&'a str>,
        }

        // Standard repository units for team access
        let units = vec![
            "repo.code",
            "repo.issues",
            "repo.pulls",
            "repo.releases",
            "repo.wiki",
            "repo.projects",
        ];

        let resp = self
            .http
            .post(self.api_url(&format!("/orgs/{}/teams", org)))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&CreateTeamRequest {
                name,
                description,
                permission,
                includes_all_repositories: true,
                units,
            })
            .send()
            .await?;

        let team: GiteaTeamResponse = self.handle_response(resp).await?;
        Ok(team.id)
    }

    async fn add_maintainer(&self, org: &str, username: &str) -> Result<(), GiteaError> {
        // Get teams to find or create Maintainers team
        let resp = self
            .http
            .get(self.api_url(&format!("/orgs/{}/teams", org)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let teams: Vec<GiteaTeamResponse> = self.handle_response(resp).await?;

        // Find Maintainers team or create it
        let maintainers_team_id = match teams.iter().find(|t| t.name == "Maintainers") {
            Some(team) => team.id,
            None => {
                // Create Maintainers team with write permission
                self.create_team(org, "Maintainers", Some("Project maintainers"), "write")
                    .await?
            }
        };

        // Add user to Maintainers team
        let resp = self
            .http
            .put(self.api_url(&format!(
                "/teams/{}/members/{}",
                maintainers_team_id, username
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn remove_maintainer(&self, org: &str, username: &str) -> Result<(), GiteaError> {
        // Get Maintainers team
        let resp = self
            .http
            .get(self.api_url(&format!("/orgs/{}/teams", org)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let teams: Vec<GiteaTeamResponse> = self.handle_response(resp).await?;
        let maintainers_team = teams
            .iter()
            .find(|t| t.name == "Maintainers")
            .ok_or_else(|| GiteaError::Api {
                status: 404,
                message: "Maintainers team not found".to_string(),
            })?;

        // Remove user from Maintainers team
        let resp = self
            .http
            .delete(self.api_url(&format!(
                "/teams/{}/members/{}",
                maintainers_team.id, username
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn list_maintainers(&self, org: &str) -> Result<Vec<String>, GiteaError> {
        // Get Maintainers team
        let resp = self
            .http
            .get(self.api_url(&format!("/orgs/{}/teams", org)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let teams: Vec<GiteaTeamResponse> = self.handle_response(resp).await?;

        // If no Maintainers team, return empty list
        let maintainers_team = match teams.iter().find(|t| t.name == "Maintainers") {
            Some(team) => team,
            None => return Ok(vec![]),
        };

        // Get team members
        let resp = self
            .http
            .get(self.api_url(&format!("/teams/{}/members", maintainers_team.id)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        #[derive(Deserialize)]
        struct TeamMember {
            login: String,
        }

        let members: Vec<TeamMember> = self.handle_response(resp).await?;
        Ok(members.into_iter().map(|m| m.login).collect())
    }

    async fn list_user_orgs(&self, username: &str) -> Result<Vec<GiteaOrg>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/users/{}/orgs", username)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let orgs: Vec<GiteaOrgResponse> = self.handle_response(resp).await?;
        Ok(orgs.into_iter().map(|o| o.into()).collect())
    }

    async fn is_org_owner(&self, org: &str, username: &str) -> Result<bool, GiteaError> {
        // Get the org's teams to find the "Owners" team
        let resp = self
            .http
            .get(self.api_url(&format!("/orgs/{}/teams", org)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Err(GiteaError::OrgNotFound(org.to_string()));
        }

        let teams: Vec<GiteaTeamResponse> = self.handle_response(resp).await?;

        // Find the Owners team
        let owners_team = match teams.iter().find(|t| t.name == "Owners") {
            Some(team) => team,
            None => return Ok(false),
        };

        // Get team members
        let resp = self
            .http
            .get(self.api_url(&format!("/teams/{}/members", owners_team.id)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        #[derive(Deserialize)]
        struct TeamMember {
            login: String,
        }

        let members: Vec<TeamMember> = self.handle_response(resp).await?;
        Ok(members.iter().any(|m| m.login == username))
    }

    async fn create_org_repo(
        &self,
        org: &str,
        name: &str,
        description: Option<&str>,
        private: bool,
        auto_init: bool,
    ) -> Result<GiteaRepo, GiteaError> {
        let resp = self
            .http
            .post(self.api_url(&format!("/orgs/{}/repos", org)))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&CreateRepoRequest {
                name,
                description,
                private,
                auto_init,
            })
            .send()
            .await?;

        let repo: GiteaRepoResponse = self.handle_response(resp).await?;
        Ok(repo.into())
    }

    async fn create_user_repo(
        &self,
        _username: &str,
        name: &str,
        description: Option<&str>,
        private: bool,
        auto_init: bool,
        user_token: &str,
    ) -> Result<GiteaRepo, GiteaError> {
        // Use the user's token to create repo in their namespace
        let resp = self
            .http
            .post(self.api_url("/user/repos"))
            .header("Authorization", format!("token {}", user_token))
            .json(&CreateRepoRequest {
                name,
                description,
                private,
                auto_init,
            })
            .send()
            .await?;

        let repo: GiteaRepoResponse = self.handle_response(resp).await?;
        Ok(repo.into())
    }

    async fn get_repo(&self, owner: &str, name: &str) -> Result<GiteaRepo, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/repos/{}/{}", owner, name)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Err(GiteaError::RepoNotFound {
                owner: owner.to_string(),
                repo: name.to_string(),
            });
        }

        let repo: GiteaRepoResponse = self.handle_response(resp).await?;
        Ok(repo.into())
    }

    async fn fork_repo(
        &self,
        owner: &str,
        repo: &str,
        new_owner: &str,
    ) -> Result<GiteaRepo, GiteaError> {
        let resp = self
            .http
            .post(self.api_url(&format!("/repos/{}/{}/forks", owner, repo)))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&ForkRepoRequest {
                organization: Some(new_owner),
            })
            .send()
            .await?;

        let forked: GiteaRepoResponse = self.handle_response(resp).await?;
        Ok(forked.into())
    }

    async fn delete_repo(&self, owner: &str, name: &str) -> Result<(), GiteaError> {
        let resp = self
            .http
            .delete(self.api_url(&format!("/repos/{}/{}", owner, name)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn create_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &str,
        message: &str,
        user_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        use base64::Engine;
        let encoded_content = base64::engine::general_purpose::STANDARD.encode(content);
        let token = user_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .post(self.api_url(&format!("/repos/{}/{}/contents/{}", owner, repo, path)))
            .header("Authorization", format!("token {}", token))
            .json(&serde_json::json!({
                "content": encoded_content,
                "message": message
            }))
            .send()
            .await?;

        // 201 = created, 200 = updated
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(GiteaError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    async fn add_collaborator(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
        permission: &str,
    ) -> Result<(), GiteaError> {
        let resp = self
            .http
            .put(self.api_url(&format!(
                "/repos/{}/{}/collaborators/{}",
                owner, repo, username
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&AddCollaboratorRequest { permission })
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn get_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<GiteaBranch, GiteaError> {
        // URL-encode the branch name to handle slashes (e.g., "feature/foo")
        let encoded_branch = encode(branch);
        let url = self.api_url(&format!(
            "/repos/{}/{}/branches/{}",
            owner, repo, encoded_branch
        ));
        tracing::debug!("get_branch: fetching {}", url);

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        tracing::debug!("get_branch: status {}", resp.status());
        let branch: GiteaBranchResponse = self.handle_response(resp).await?;
        Ok(branch.into())
    }

    async fn list_branches(&self, owner: &str, repo: &str) -> Result<Vec<GiteaBranch>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/repos/{}/{}/branches", owner, repo)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let branches: Vec<GiteaBranchResponse> = self.handle_response(resp).await?;
        Ok(branches.into_iter().map(|b| b.into()).collect())
    }

    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        head: &str,
        base: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaPullRequest, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);
        let resp = self
            .http
            .post(self.api_url(&format!("/repos/{}/{}/pulls", owner, repo)))
            .header("Authorization", format!("token {}", token))
            .json(&CreatePRRequest {
                title,
                body,
                head,
                base,
            })
            .send()
            .await?;

        let pr: GiteaPRResponse = self.handle_response(resp).await?;
        Ok(pr.into())
    }

    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<GiteaPullRequest, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/repos/{}/{}/pulls/{}", owner, repo, number)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let pr: GiteaPRResponse = self.handle_response(resp).await?;
        Ok(pr.into())
    }

    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
    ) -> Result<Vec<GiteaPullRequest>, GiteaError> {
        let mut url = self.api_url(&format!("/repos/{}/{}/pulls", owner, repo));
        if let Some(s) = state {
            url.push_str(&format!("?state={}", s));
        }

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let prs: Vec<GiteaPRResponse> = self.handle_response(resp).await?;
        Ok(prs.into_iter().map(|pr| pr.into()).collect())
    }

    async fn get_user_prs(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
    ) -> Result<Vec<GiteaPullRequest>, GiteaError> {
        // Use list_pull_requests and filter by user's head branch
        let all_prs = self.list_pull_requests(owner, repo, Some("all")).await?;
        Ok(all_prs
            .into_iter()
            .filter(|pr| {
                // Filter PRs by head branch containing the username (fork convention)
                pr.head.ref_name.contains(username)
            })
            .collect())
    }

    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        merge_style: &str,
        auth_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .post(self.api_url(&format!("/repos/{}/{}/pulls/{}/merge", owner, repo, number)))
            .header("Authorization", format!("token {}", token))
            .json(&MergePRRequest {
                do_merge: merge_style,
            })
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn close_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<(), GiteaError> {
        let resp = self
            .http
            .patch(self.api_url(&format!("/repos/{}/{}/pulls/{}", owner, repo, number)))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&serde_json::json!({"state": "closed"}))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn get_pr_comments(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaComment>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/comments",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let comments: Vec<GiteaCommentResponse> = self.handle_response(resp).await?;
        Ok(comments.into_iter().map(|c| c.into()).collect())
    }

    async fn post_pr_comment(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        body: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaComment, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);
        let resp = self
            .http
            .post(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/comments",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", token))
            .json(&CreateCommentRequest { body })
            .send()
            .await?;

        let comment: GiteaCommentResponse = self.handle_response(resp).await?;
        Ok(comment.into())
    }

    async fn get_pr_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaPRReview>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!(
                "/repos/{}/{}/pulls/{}/reviews",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let reviews: Vec<GiteaPRReviewResponse> = self.handle_response(resp).await?;
        Ok(reviews.into_iter().map(|r| r.into()).collect())
    }

    async fn submit_pr_review(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        state: &str,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Result<GiteaPRReview, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);
        let resp = self
            .http
            .post(self.api_url(&format!(
                "/repos/{}/{}/pulls/{}/reviews",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", token))
            .json(&SubmitReviewRequest { event: state, body })
            .send()
            .await?;

        let review: GiteaPRReviewResponse = self.handle_response(resp).await?;
        Ok(review.into())
    }

    async fn get_commit_status(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
    ) -> Result<GiteaCombinedStatus, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!(
                "/repos/{}/{}/commits/{}/status",
                owner, repo, ref_name
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let status: GiteaCombinedStatusResponse = self.handle_response(resp).await?;
        Ok(status.into())
    }

    async fn create_webhook(
        &self,
        owner: &str,
        repo: &str,
        url: &str,
        events: Vec<String>,
        secret: Option<&str>,
    ) -> Result<i64, GiteaError> {
        let resp = self
            .http
            .post(self.api_url(&format!("/repos/{}/{}/hooks", owner, repo)))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&CreateWebhookRequest {
                hook_type: "gitea",
                config: WebhookConfig {
                    url,
                    content_type: "json",
                    secret,
                },
                events,
                active: true,
            })
            .send()
            .await?;

        let webhook: WebhookResponse = self.handle_response(resp).await?;
        Ok(webhook.id)
    }

    async fn delete_webhook(
        &self,
        owner: &str,
        repo: &str,
        hook_id: i64,
    ) -> Result<(), GiteaError> {
        let resp = self
            .http
            .delete(self.api_url(&format!("/repos/{}/{}/hooks/{}", owner, repo, hook_id)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn get_issue_reactions(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
    ) -> Result<Vec<GiteaReaction>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/reactions",
                owner, repo, issue_number
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let reactions: Vec<GiteaReactionResponse> = self.handle_response(resp).await?;
        Ok(reactions.into_iter().map(|r| r.into()).collect())
    }

    async fn post_issue_reaction(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
        content: &str,
    ) -> Result<GiteaReaction, GiteaError> {
        let resp = self
            .http
            .post(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/reactions",
                owner, repo, issue_number
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&CreateReactionRequest { content })
            .send()
            .await?;

        let reaction: GiteaReactionResponse = self.handle_response(resp).await?;
        Ok(reaction.into())
    }

    async fn delete_issue_reaction(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i64,
        reaction_id: i64,
    ) -> Result<(), GiteaError> {
        let resp = self
            .http
            .delete(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/reactions/{}",
                owner, repo, issue_number, reaction_id
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn get_comment_reactions(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
    ) -> Result<Vec<GiteaReaction>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!(
                "/repos/{}/{}/issues/comments/{}/reactions",
                owner, repo, comment_id
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        let reactions: Vec<GiteaReactionResponse> = self.handle_response(resp).await?;
        Ok(reactions.into_iter().map(|r| r.into()).collect())
    }

    async fn post_comment_reaction(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        content: &str,
    ) -> Result<GiteaReaction, GiteaError> {
        let resp = self
            .http
            .post(self.api_url(&format!(
                "/repos/{}/{}/issues/comments/{}/reactions",
                owner, repo, comment_id
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .json(&CreateReactionRequest { content })
            .send()
            .await?;

        let reaction: GiteaReactionResponse = self.handle_response(resp).await?;
        Ok(reaction.into())
    }

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .post(self.api_url(&format!("/repos/{}/{}/issues", owner, repo)))
            .header("Authorization", format!("token {}", token))
            .json(&serde_json::json!({
                "title": title,
                "body": body.unwrap_or("")
            }))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
    ) -> Result<Vec<GiteaIssue>, GiteaError> {
        let mut url = format!("/repos/{}/{}/issues", owner, repo);
        if let Some(s) = state {
            url.push_str(&format!("?state={}", s));
        }

        let resp = self
            .http
            .get(self.api_url(&url))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<GiteaIssue, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/repos/{}/{}/issues/{}", owner, repo, number)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        title: Option<&str>,
        body: Option<&str>,
        state: Option<&str>,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let mut payload = serde_json::Map::new();
        if let Some(t) = title {
            payload.insert(
                "title".to_string(),
                serde_json::Value::String(t.to_string()),
            );
        }
        if let Some(b) = body {
            payload.insert("body".to_string(), serde_json::Value::String(b.to_string()));
        }
        if let Some(s) = state {
            payload.insert(
                "state".to_string(),
                serde_json::Value::String(s.to_string()),
            );
        }

        let resp = self
            .http
            .patch(self.api_url(&format!("/repos/{}/{}/issues/{}", owner, repo, number)))
            .header("Authorization", format!("token {}", token))
            .json(&payload)
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn list_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaIssueComment>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/comments",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn create_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        body: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssueComment, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .post(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/comments",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", token))
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn edit_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        body: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssueComment, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .patch(self.api_url(&format!(
                "/repos/{}/{}/issues/comments/{}",
                owner, repo, comment_id
            )))
            .header("Authorization", format!("token {}", token))
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn delete_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: i64,
        auth_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .delete(self.api_url(&format!(
                "/repos/{}/{}/issues/comments/{}",
                owner, repo, comment_id
            )))
            .header("Authorization", format!("token {}", token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn list_issue_labels(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<Vec<GiteaLabel>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/labels",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn add_issue_labels(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        labels: Vec<String>,
        auth_token: Option<&str>,
    ) -> Result<Vec<GiteaLabel>, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .post(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/labels",
                owner, repo, number
            )))
            .header("Authorization", format!("token {}", token))
            .json(&serde_json::json!({ "labels": labels }))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn remove_issue_label(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        label: &str,
        auth_token: Option<&str>,
    ) -> Result<(), GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        let resp = self
            .http
            .delete(self.api_url(&format!(
                "/repos/{}/{}/issues/{}/labels/{}",
                owner,
                repo,
                number,
                urlencoding::encode(label)
            )))
            .header("Authorization", format!("token {}", token))
            .send()
            .await?;

        self.handle_empty_response(resp).await
    }

    async fn add_issue_assignees(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        assignees: Vec<String>,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        // Gitea uses PATCH on the issue endpoint to update assignees
        let resp = self
            .http
            .patch(self.api_url(&format!("/repos/{}/{}/issues/{}", owner, repo, number)))
            .header("Authorization", format!("token {}", token))
            .json(&serde_json::json!({ "assignees": assignees }))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn remove_issue_assignee(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        assignee: &str,
        auth_token: Option<&str>,
    ) -> Result<GiteaIssue, GiteaError> {
        let token = auth_token.unwrap_or(&self.admin_token);

        // First get current assignees
        let issue = self.get_issue(owner, repo, number).await?;
        let current_assignees: Vec<String> = issue
            .assignees
            .iter()
            .map(|a| a.login.clone())
            .filter(|a| a != assignee)
            .collect();

        // Update with filtered assignees
        let resp = self
            .http
            .patch(self.api_url(&format!("/repos/{}/{}/issues/{}", owner, repo, number)))
            .header("Authorization", format!("token {}", token))
            .json(&serde_json::json!({ "assignees": current_assignees }))
            .send()
            .await?;

        self.handle_response(resp).await
    }

    async fn list_repo_labels(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<GiteaLabel>, GiteaError> {
        let resp = self
            .http
            .get(self.api_url(&format!("/repos/{}/{}/labels", owner, repo)))
            .header("Authorization", format!("token {}", self.admin_token))
            .send()
            .await?;

        self.handle_response(resp).await
    }
}
