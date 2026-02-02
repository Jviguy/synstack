//! Project domain entity
//!
//! Represents an Ant Farm project where agents collaborate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent::AgentId;

/// Unique identifier for a project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub Uuid);

impl ProjectId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ProjectId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Project status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    /// Project is active and accepting contributions
    Active,
    /// Project is paused
    Paused,
    /// Project is completed
    Completed,
    /// Project is archived
    Archived,
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStatus::Active => write!(f, "active"),
            ProjectStatus::Paused => write!(f, "paused"),
            ProjectStatus::Completed => write!(f, "completed"),
            ProjectStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for ProjectStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(ProjectStatus::Active),
            "paused" => Ok(ProjectStatus::Paused),
            "completed" => Ok(ProjectStatus::Completed),
            "archived" => Ok(ProjectStatus::Archived),
            _ => Err(format!("Unknown project status: {}", s)),
        }
    }
}

/// Build status for the project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    Unknown,
    Passing,
    Failing,
}

impl std::fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildStatus::Unknown => write!(f, "unknown"),
            BuildStatus::Passing => write!(f, "passing"),
            BuildStatus::Failing => write!(f, "failing"),
        }
    }
}

impl std::str::FromStr for BuildStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unknown" => Ok(BuildStatus::Unknown),
            "passing" => Ok(BuildStatus::Passing),
            "failing" => Ok(BuildStatus::Failing),
            _ => Err(format!("Unknown build status: {}", s)),
        }
    }
}

/// An Ant Farm project where agents collaborate
#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub description: Option<String>,
    pub gitea_org: String,
    pub gitea_repo: String,
    pub language: Option<String>,
    pub status: ProjectStatus,
    pub contributor_count: i32,
    pub open_ticket_count: i32,
    pub build_status: BuildStatus,
    pub created_by: Option<AgentId>,
    pub created_at: DateTime<Utc>,
}

impl Project {
    /// Get the full Gitea repository path
    pub fn gitea_path(&self) -> String {
        format!("{}/{}", self.gitea_org, self.gitea_repo)
    }

    /// Check if the project is accepting new contributors
    pub fn is_joinable(&self) -> bool {
        self.status == ProjectStatus::Active
    }
}

/// Data needed to create a new project
#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub description: Option<String>,
    pub gitea_org: String,
    pub gitea_repo: String,
    pub language: Option<String>,
    pub created_by: Option<AgentId>,
}

/// Role of a member in a project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    Owner,
    Maintainer,
    Contributor,
}

impl std::fmt::Display for MemberRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemberRole::Owner => write!(f, "owner"),
            MemberRole::Maintainer => write!(f, "maintainer"),
            MemberRole::Contributor => write!(f, "contributor"),
        }
    }
}

impl std::str::FromStr for MemberRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(MemberRole::Owner),
            "maintainer" => Ok(MemberRole::Maintainer),
            "contributor" => Ok(MemberRole::Contributor),
            _ => Err(format!("Unknown member role: {}", s)),
        }
    }
}

/// A project member
#[derive(Debug, Clone, Serialize)]
pub struct ProjectMember {
    pub project_id: ProjectId,
    pub agent_id: AgentId,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_project(status: ProjectStatus) -> Project {
        Project {
            id: ProjectId::new(),
            name: "test-project".to_string(),
            description: Some("A test project".to_string()),
            gitea_org: "antfarm-test".to_string(),
            gitea_repo: "main".to_string(),
            language: Some("rust".to_string()),
            status,
            contributor_count: 5,
            open_ticket_count: 10,
            build_status: BuildStatus::Passing,
            created_by: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn project_gitea_path() {
        let project = make_project(ProjectStatus::Active);
        assert_eq!(project.gitea_path(), "antfarm-test/main");
    }

    #[test]
    fn project_is_joinable_active() {
        let project = make_project(ProjectStatus::Active);
        assert!(project.is_joinable());
    }

    #[test]
    fn project_is_joinable_paused() {
        let project = make_project(ProjectStatus::Paused);
        assert!(!project.is_joinable());
    }

    #[test]
    fn project_is_joinable_completed() {
        let project = make_project(ProjectStatus::Completed);
        assert!(!project.is_joinable());
    }

    #[test]
    fn project_is_joinable_archived() {
        let project = make_project(ProjectStatus::Archived);
        assert!(!project.is_joinable());
    }

    #[test]
    fn project_status_display() {
        assert_eq!(ProjectStatus::Active.to_string(), "active");
        assert_eq!(ProjectStatus::Paused.to_string(), "paused");
        assert_eq!(ProjectStatus::Completed.to_string(), "completed");
        assert_eq!(ProjectStatus::Archived.to_string(), "archived");
    }

    #[test]
    fn project_status_from_str() {
        assert_eq!(
            "active".parse::<ProjectStatus>().unwrap(),
            ProjectStatus::Active
        );
        assert_eq!(
            "PAUSED".parse::<ProjectStatus>().unwrap(),
            ProjectStatus::Paused
        );
        assert!("invalid".parse::<ProjectStatus>().is_err());
    }

    #[test]
    fn build_status_display() {
        assert_eq!(BuildStatus::Unknown.to_string(), "unknown");
        assert_eq!(BuildStatus::Passing.to_string(), "passing");
        assert_eq!(BuildStatus::Failing.to_string(), "failing");
    }

    #[test]
    fn build_status_from_str() {
        assert_eq!(
            "unknown".parse::<BuildStatus>().unwrap(),
            BuildStatus::Unknown
        );
        assert_eq!(
            "PASSING".parse::<BuildStatus>().unwrap(),
            BuildStatus::Passing
        );
        assert_eq!(
            "failing".parse::<BuildStatus>().unwrap(),
            BuildStatus::Failing
        );
        assert!("invalid".parse::<BuildStatus>().is_err());
    }

    #[test]
    fn member_role_display() {
        assert_eq!(MemberRole::Owner.to_string(), "owner");
        assert_eq!(MemberRole::Maintainer.to_string(), "maintainer");
        assert_eq!(MemberRole::Contributor.to_string(), "contributor");
    }

    #[test]
    fn member_role_from_str() {
        assert_eq!("owner".parse::<MemberRole>().unwrap(), MemberRole::Owner);
        assert_eq!(
            "MAINTAINER".parse::<MemberRole>().unwrap(),
            MemberRole::Maintainer
        );
        assert_eq!(
            "contributor".parse::<MemberRole>().unwrap(),
            MemberRole::Contributor
        );
        assert!("invalid".parse::<MemberRole>().is_err());
    }

    #[test]
    fn project_id_display() {
        let id = ProjectId(Uuid::nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }
}
