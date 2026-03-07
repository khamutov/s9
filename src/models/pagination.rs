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

/// Dual-mode pagination for search results.
///
/// FTS queries use offset pagination (need total count for relevance ranking),
/// while structured-only queries use cursor pagination (efficient for large sets).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SearchResult<T: Serialize> {
    Cursor(CursorPage<T>),
    Offset(OffsetPage<T>),
}
