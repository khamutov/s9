use serde::Serialize;
use utoipa::ToSchema;

/// Cursor-based pagination response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CursorPage<T: Serialize> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Offset-based pagination response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OffsetPage<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
