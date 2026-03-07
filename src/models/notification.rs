use chrono::{DateTime, Utc};

/// Database row for the `pending_notifications` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingNotificationRow {
    pub id: i64,
    pub user_id: i64,
    pub ticket_id: i64,
    pub event_type: String,
    pub created_at: DateTime<Utc>,
    pub send_after: DateTime<Utc>,
}

/// Database row for the `notification_mutes` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotificationMuteRow {
    pub user_id: i64,
    pub ticket_id: i64,
}
