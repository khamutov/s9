use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::deserialize_optional_field;
use super::enums::MilestoneStatus;

/// Database row for the `milestones` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MilestoneRow {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub status: MilestoneStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Aggregated ticket statistics for a milestone.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct MilestoneStats {
    pub total: i64,
    pub new: i64,
    pub in_progress: i64,
    pub verify: i64,
    pub done: i64,
    pub estimated_hours: f64,
    pub remaining_hours: f64,
}

/// Full milestone in API responses, with computed stats.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MilestoneResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub status: MilestoneStatus,
    pub stats: MilestoneStats,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Compact milestone reference embedded in ticket responses.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompactMilestone {
    pub id: i64,
    pub name: String,
}

/// Request body for creating a new milestone.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMilestoneRequest {
    pub name: String,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub status: Option<MilestoneStatus>,
}

/// Request body for updating an existing milestone.
///
/// `description` and `due_date` use double-Option for null-clearing semantics.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMilestoneRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub due_date: Option<Option<NaiveDate>>,
    pub status: Option<MilestoneStatus>,
}
