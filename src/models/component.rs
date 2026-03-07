use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::deserialize_optional_field;
use super::user::CompactUser;

/// Database row for the `components` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ComponentRow {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub path: String,
    pub owner_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Full component in API responses, with expanded owner and ticket count.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ComponentResponse {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub path: String,
    pub owner: CompactUser,
    pub ticket_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Compact component reference embedded in ticket responses.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompactComponent {
    pub id: i64,
    pub name: String,
    pub path: String,
}

impl From<&ComponentRow> for CompactComponent {
    fn from(row: &ComponentRow) -> Self {
        Self {
            id: row.id,
            name: row.name.clone(),
            path: row.path.clone(),
        }
    }
}

/// Request body for creating a new component.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateComponentRequest {
    pub name: String,
    pub parent_id: Option<i64>,
    pub owner_id: i64,
}

/// Request body for updating an existing component.
///
/// `parent_id` uses double-Option: absent means "don't change",
/// `null` means "make root", value means "set new parent".
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateComponentRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub parent_id: Option<Option<i64>>,
    pub owner_id: Option<i64>,
}
