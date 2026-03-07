use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::attachment::AttachmentResponse;
use super::user::CompactUser;

/// Database row for the `comments` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommentRow {
    pub id: i64,
    pub ticket_id: i64,
    pub number: i64,
    pub author_id: i64,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Database row for the `comment_edits` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommentEditRow {
    pub id: i64,
    pub comment_id: i64,
    pub old_body: String,
    pub edited_at: DateTime<Utc>,
}

/// Full comment in API responses.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommentResponse {
    pub id: i64,
    pub ticket_id: i64,
    pub number: i64,
    pub author: CompactUser,
    pub body: String,
    pub attachments: Vec<AttachmentResponse>,
    pub edit_count: i64,
    pub edits: Vec<CommentEditResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A previous version of a comment body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommentEditResponse {
    pub old_body: String,
    pub edited_at: DateTime<Utc>,
}

/// Request body for creating a new comment.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCommentRequest {
    pub body: String,
    pub attachment_ids: Option<Vec<i64>>,
}

/// Request body for editing an existing comment.
#[derive(Debug, Deserialize, ToSchema)]
pub struct EditCommentRequest {
    pub body: String,
}
