use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Database row for the `attachments` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AttachmentRow {
    pub id: i64,
    pub sha256: String,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploader_id: i64,
    pub created_at: DateTime<Utc>,
}

/// Attachment in API responses. SHA-256 is hidden; URL is computed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AttachmentResponse {
    pub id: i64,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub url: String,
}

impl From<&AttachmentRow> for AttachmentResponse {
    fn from(row: &AttachmentRow) -> Self {
        Self {
            id: row.id,
            original_name: row.original_name.clone(),
            mime_type: row.mime_type.clone(),
            size_bytes: row.size_bytes,
            url: format!("/api/attachments/{}/{}", row.id, row.original_name),
        }
    }
}
