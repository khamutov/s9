#![allow(dead_code)]

use chrono::{DateTime, Utc};

/// Database row for the `sessions` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub user_id: i64,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
