use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Type of a ticket: bug report or feature request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum TicketType {
    Bug,
    Feature,
}

/// Workflow status of a ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum TicketStatus {
    New,
    InProgress,
    Verify,
    Done,
}

/// Priority level from P0 (critical) to P5 (cosmetic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
}

/// User role within the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum Role {
    Admin,
    User,
}

/// Open/closed status of a milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Open,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_ticket_type() {
        let json = serde_json::to_string(&TicketType::Bug).unwrap();
        assert_eq!(json, r#""bug""#);
        let parsed: TicketType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TicketType::Bug);

        let json = serde_json::to_string(&TicketType::Feature).unwrap();
        assert_eq!(json, r#""feature""#);
        let parsed: TicketType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TicketType::Feature);
    }

    #[test]
    fn serde_roundtrip_ticket_status() {
        let json = serde_json::to_string(&TicketStatus::InProgress).unwrap();
        assert_eq!(json, r#""in_progress""#);
        let parsed: TicketStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TicketStatus::InProgress);

        for status in [TicketStatus::New, TicketStatus::Verify, TicketStatus::Done] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: TicketStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn serde_roundtrip_priority() {
        for p in [
            Priority::P0,
            Priority::P1,
            Priority::P2,
            Priority::P3,
            Priority::P4,
            Priority::P5,
        ] {
            let json = serde_json::to_string(&p).unwrap();
            let parsed: Priority = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, p);
        }
        // P0 should serialize without rename
        assert_eq!(serde_json::to_string(&Priority::P0).unwrap(), r#""P0""#);
    }

    #[test]
    fn serde_roundtrip_role() {
        let json = serde_json::to_string(&Role::Admin).unwrap();
        assert_eq!(json, r#""admin""#);
        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Role::Admin);

        let json = serde_json::to_string(&Role::User).unwrap();
        assert_eq!(json, r#""user""#);
        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Role::User);
    }

    #[test]
    fn serde_roundtrip_milestone_status() {
        let json = serde_json::to_string(&MilestoneStatus::Open).unwrap();
        assert_eq!(json, r#""open""#);
        let parsed: MilestoneStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, MilestoneStatus::Open);

        let json = serde_json::to_string(&MilestoneStatus::Closed).unwrap();
        assert_eq!(json, r#""closed""#);
        let parsed: MilestoneStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, MilestoneStatus::Closed);
    }
}
