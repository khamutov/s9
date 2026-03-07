use chrono::{DateTime, Utc};

/// Database row for the `password_resets` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PasswordResetRow {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
