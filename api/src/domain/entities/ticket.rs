//! Ticket domain entity
//!
//! Represents a project ticket/issue that agents can work on in Ant Farm mode.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent::AgentId;
use super::project::ProjectId;

/// Unique identifier for a ticket
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TicketId(pub Uuid);

impl TicketId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TicketId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for TicketId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TicketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Ticket status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TicketStatus {
    Open,
    InProgress,
    Closed,
}

impl std::fmt::Display for TicketStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TicketStatus::Open => write!(f, "open"),
            TicketStatus::InProgress => write!(f, "in_progress"),
            TicketStatus::Closed => write!(f, "closed"),
        }
    }
}

impl std::str::FromStr for TicketStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(TicketStatus::Open),
            "in_progress" | "inprogress" => Ok(TicketStatus::InProgress),
            "closed" => Ok(TicketStatus::Closed),
            _ => Err(format!("Unknown ticket status: {}", s)),
        }
    }
}

/// Ticket priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TicketPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for TicketPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TicketPriority::Low => write!(f, "low"),
            TicketPriority::Medium => write!(f, "medium"),
            TicketPriority::High => write!(f, "high"),
            TicketPriority::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for TicketPriority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(TicketPriority::Low),
            "medium" => Ok(TicketPriority::Medium),
            "high" => Ok(TicketPriority::High),
            "critical" => Ok(TicketPriority::Critical),
            _ => Err(format!("Unknown ticket priority: {}", s)),
        }
    }
}

/// A project ticket that agents can work on
#[derive(Debug, Clone, Serialize)]
pub struct Ticket {
    pub id: TicketId,
    pub project_id: ProjectId,
    pub title: String,
    pub body: Option<String>,
    /// Gitea issue number (synced with Gitea)
    pub gitea_issue_number: Option<i32>,
    pub gitea_issue_url: Option<String>,
    pub status: TicketStatus,
    pub priority: TicketPriority,
    /// Agent currently working on this ticket
    pub assigned_to: Option<AgentId>,
    pub created_by: Option<AgentId>,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl Ticket {
    /// Check if the ticket is available for assignment
    pub fn is_available(&self) -> bool {
        self.status == TicketStatus::Open && self.assigned_to.is_none()
    }

    /// Check if the ticket can be assigned to a specific agent
    pub fn can_be_assigned_to(&self, agent_id: &AgentId) -> bool {
        // Can assign if ticket is open and either unassigned or already assigned to this agent
        self.status == TicketStatus::Open
            && (self.assigned_to.is_none() || self.assigned_to.as_ref() == Some(agent_id))
    }
}

/// Data needed to create a new ticket
#[derive(Debug, Clone)]
pub struct NewTicket {
    pub project_id: ProjectId,
    pub title: String,
    pub body: Option<String>,
    pub gitea_issue_number: Option<i32>,
    pub gitea_issue_url: Option<String>,
    pub priority: TicketPriority,
    pub created_by: Option<AgentId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ticket(status: TicketStatus, assigned_to: Option<AgentId>) -> Ticket {
        Ticket {
            id: TicketId::new(),
            project_id: ProjectId::new(),
            title: "Test Ticket".to_string(),
            body: Some("Test body".to_string()),
            gitea_issue_number: Some(1),
            gitea_issue_url: Some("https://gitea.test/issue/1".to_string()),
            status,
            priority: TicketPriority::Medium,
            assigned_to,
            created_by: None,
            created_at: Utc::now(),
            closed_at: None,
        }
    }

    #[test]
    fn ticket_is_available_when_open_and_unassigned() {
        let ticket = make_ticket(TicketStatus::Open, None);
        assert!(ticket.is_available());
    }

    #[test]
    fn ticket_is_not_available_when_assigned() {
        let ticket = make_ticket(TicketStatus::Open, Some(AgentId::new()));
        assert!(!ticket.is_available());
    }

    #[test]
    fn ticket_is_not_available_when_in_progress() {
        let ticket = make_ticket(TicketStatus::InProgress, None);
        assert!(!ticket.is_available());
    }

    #[test]
    fn ticket_is_not_available_when_closed() {
        let ticket = make_ticket(TicketStatus::Closed, None);
        assert!(!ticket.is_available());
    }

    #[test]
    fn can_assign_to_agent_when_open_and_unassigned() {
        let ticket = make_ticket(TicketStatus::Open, None);
        let agent_id = AgentId::new();
        assert!(ticket.can_be_assigned_to(&agent_id));
    }

    #[test]
    fn can_assign_to_same_agent_if_already_assigned() {
        let agent_id = AgentId::new();
        let ticket = make_ticket(TicketStatus::Open, Some(agent_id));
        assert!(ticket.can_be_assigned_to(&agent_id));
    }

    #[test]
    fn cannot_assign_to_different_agent_if_already_assigned() {
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();
        let ticket = make_ticket(TicketStatus::Open, Some(agent1));
        assert!(!ticket.can_be_assigned_to(&agent2));
    }

    #[test]
    fn ticket_status_display() {
        assert_eq!(TicketStatus::Open.to_string(), "open");
        assert_eq!(TicketStatus::InProgress.to_string(), "in_progress");
        assert_eq!(TicketStatus::Closed.to_string(), "closed");
    }

    #[test]
    fn ticket_status_from_str() {
        assert_eq!("open".parse::<TicketStatus>().unwrap(), TicketStatus::Open);
        assert_eq!(
            "in_progress".parse::<TicketStatus>().unwrap(),
            TicketStatus::InProgress
        );
        assert_eq!(
            "inprogress".parse::<TicketStatus>().unwrap(),
            TicketStatus::InProgress
        );
        assert_eq!(
            "closed".parse::<TicketStatus>().unwrap(),
            TicketStatus::Closed
        );
        assert!("invalid".parse::<TicketStatus>().is_err());
    }

    #[test]
    fn ticket_priority_display() {
        assert_eq!(TicketPriority::Low.to_string(), "low");
        assert_eq!(TicketPriority::Medium.to_string(), "medium");
        assert_eq!(TicketPriority::High.to_string(), "high");
        assert_eq!(TicketPriority::Critical.to_string(), "critical");
    }

    #[test]
    fn ticket_priority_from_str() {
        assert_eq!(
            "low".parse::<TicketPriority>().unwrap(),
            TicketPriority::Low
        );
        assert_eq!(
            "medium".parse::<TicketPriority>().unwrap(),
            TicketPriority::Medium
        );
        assert_eq!(
            "high".parse::<TicketPriority>().unwrap(),
            TicketPriority::High
        );
        assert_eq!(
            "critical".parse::<TicketPriority>().unwrap(),
            TicketPriority::Critical
        );
        assert!("invalid".parse::<TicketPriority>().is_err());
    }

    #[test]
    fn ticket_id_display() {
        let id = TicketId(uuid::Uuid::nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }
}
