//! Issue domain entity
//!
//! Issues live in Gitea (source of truth). This entity represents
//! the domain's view of an issue.

use serde::{Deserialize, Serialize};

use super::ProjectId;

/// Unique identifier for an issue (Gitea issue number + project)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IssueId {
    pub project_id: ProjectId,
    pub number: i64,
}

impl IssueId {
    pub fn new(project_id: ProjectId, number: i64) -> Self {
        Self { project_id, number }
    }
}

/// An issue in a project (from Gitea)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: IssueId,
    pub title: String,
    pub body: Option<String>,
    pub state: IssueState,
    pub url: String,
    pub labels: Vec<Label>,
    pub assignees: Vec<String>,
}

/// A label on an issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

/// A comment on an issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    pub id: i64,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Issue state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
}

impl std::fmt::Display for IssueState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueState::Open => write!(f, "open"),
            IssueState::Closed => write!(f, "closed"),
        }
    }
}

impl std::str::FromStr for IssueState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(IssueState::Open),
            "closed" => Ok(IssueState::Closed),
            _ => Err(format!("Unknown issue state: {}", s)),
        }
    }
}

/// Data needed to create a new issue
#[derive(Debug, Clone)]
pub struct NewIssue {
    pub title: String,
    pub body: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn issue_state_display() {
        assert_eq!(IssueState::Open.to_string(), "open");
        assert_eq!(IssueState::Closed.to_string(), "closed");
    }

    #[test]
    fn issue_state_parse() {
        assert_eq!("open".parse::<IssueState>().unwrap(), IssueState::Open);
        assert_eq!("CLOSED".parse::<IssueState>().unwrap(), IssueState::Closed);
    }

    #[test]
    fn issue_id_equality() {
        let pid = ProjectId(Uuid::new_v4());
        let id1 = IssueId::new(pid, 1);
        let id2 = IssueId::new(pid, 1);
        let id3 = IssueId::new(pid, 2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}
