//! Ticket API endpoints: list/search, get, create, update.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::auth::middleware::AuthUser;
use crate::models::{
    CreateTicketRequest, CursorPage, OffsetPage, SearchResult, TicketResponse, UpdateTicketRequest,
};
use crate::repos::{self, RepoError, cursor};
use crate::search;

use super::AppState;

/// Query parameters for `GET /api/tickets`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ListTicketsQuery {
    pub q: Option<String>,
    pub cursor: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

/// `GET /api/tickets` — list/search tickets with dual-mode pagination.
pub async fn list_tickets(
    State(state): State<AppState>,
    _user: AuthUser,
    Query(params): Query<ListTicketsQuery>,
) -> Response {
    let page_size = params.page_size.unwrap_or(50).clamp(1, 200);
    let q = params.q.as_deref().unwrap_or("");

    let parsed = search::parse(q);

    if parsed.has_text_search() {
        // Offset pagination with FTS (BM25 ranking).
        let page = params.page.unwrap_or(1).max(1);
        let offset = (page - 1) * page_size;

        let built = match search::build_search_query(&parsed, page_size, Some(offset), None) {
            Ok(b) => b,
            Err(e) => return validation_error("q", &e.to_string()),
        };

        let rows = match exec_search_query(&state, &built).await {
            Ok(r) => r,
            Err(_) => return internal_error(),
        };

        let total = match exec_count_query(&state, &built).await {
            Ok(t) => t,
            Err(_) => return internal_error(),
        };

        let enriched = match enrich_with_slugs(&state, &rows).await {
            Ok(r) => r,
            Err(_) => return internal_error(),
        };

        let result: SearchResult<TicketResponse> = SearchResult::Offset(OffsetPage {
            items: enriched,
            total,
            page,
            page_size,
        });

        (StatusCode::OK, Json(result)).into_response()
    } else {
        // Cursor pagination for structured-only queries.
        let cursor_val = match &params.cursor {
            Some(c) => match cursor::decode_cursor(c) {
                Ok((ts, id)) => Some((ts, id)),
                Err(_) => return validation_error("cursor", "Invalid cursor"),
            },
            None => None,
        };

        let built = match search::build_search_query(
            &parsed,
            page_size,
            None,
            cursor_val
                .as_ref()
                .map(|(ts, id)| {
                    // Use the same format sqlx uses for DateTime<Utc> in SQLite
                    // to ensure correct lexicographic comparison.
                    (ts.to_rfc3339_opts(chrono::SecondsFormat::Micros, true), *id)
                })
                .as_ref()
                .map(|(ts, id)| (ts.as_str(), *id)),
        ) {
            Ok(b) => b,
            Err(e) => return validation_error("q", &e.to_string()),
        };

        let mut rows = match exec_search_query(&state, &built).await {
            Ok(r) => r,
            Err(_) => return internal_error(),
        };

        let has_more = rows.len() as i64 > page_size;
        if has_more {
            rows.truncate(page_size as usize);
        }

        let next_cursor = if has_more {
            rows.last()
                .map(|r| cursor::encode_cursor(&r.updated_at, r.id))
        } else {
            None
        };

        let enriched = match enrich_with_slugs(&state, &rows).await {
            Ok(r) => r,
            Err(_) => return internal_error(),
        };

        let result: SearchResult<TicketResponse> = SearchResult::Cursor(CursorPage {
            items: enriched,
            next_cursor,
            has_more,
        });

        (StatusCode::OK, Json(result)).into_response()
    }
}

/// `GET /api/tickets/:id` — get a single ticket by ID.
pub async fn get_ticket(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<i64>,
) -> Response {
    let row = match repos::ticket::get_by_id(&state.pool, id).await {
        Ok(Some(r)) => r,
        Ok(None) => return not_found("Ticket not found"),
        Err(_) => return internal_error(),
    };

    let enriched = match enrich_with_slugs(&state, &[row]).await {
        Ok(mut v) => v.remove(0),
        Err(_) => return internal_error(),
    };

    (StatusCode::OK, Json(enriched)).into_response()
}

/// `POST /api/tickets` — create a new ticket.
pub async fn create_ticket(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<CreateTicketRequest>,
) -> Response {
    // Validate required fields.
    if body.title.is_empty() {
        return validation_error("title", "Title is required.");
    }
    if body.title.len() > 256 {
        return validation_error("title", "Title must be at most 256 characters.");
    }

    let row = match repos::ticket::create(&state.pool, &body, user.id).await {
        Ok(r) => r,
        Err(RepoError::Conflict(msg)) => return validation_error_msg(&msg),
        Err(_) => return internal_error(),
    };

    // If a description was provided, create comment #0.
    if let Some(desc) = &body.description
        && !desc.is_empty()
        && create_description_comment(&state, row.id, user.id, desc)
            .await
            .is_err()
    {
        return internal_error();
    }

    let enriched = match enrich_with_slugs(&state, &[row]).await {
        Ok(mut v) => v.remove(0),
        Err(_) => return internal_error(),
    };

    (StatusCode::CREATED, Json(enriched)).into_response()
}

/// `PATCH /api/tickets/:id` — update a ticket.
pub async fn update_ticket(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateTicketRequest>,
) -> Response {
    // If title is being changed, check authorization: only creator or admin.
    if body.title.is_some() {
        let existing = match repos::ticket::get_by_id(&state.pool, id).await {
            Ok(Some(r)) => r,
            Ok(None) => return not_found("Ticket not found"),
            Err(_) => return internal_error(),
        };
        if existing.created_by != user.id && user.role != crate::models::Role::Admin {
            return forbidden("Only the ticket creator or an admin can change the title.");
        }
    }

    if let Some(ref title) = body.title {
        if title.is_empty() {
            return validation_error("title", "Title is required.");
        }
        if title.len() > 256 {
            return validation_error("title", "Title must be at most 256 characters.");
        }
    }

    let row = match repos::ticket::update(&state.pool, id, &body).await {
        Ok(r) => r,
        Err(RepoError::NotFound) => return not_found("Ticket not found"),
        Err(RepoError::Conflict(msg)) => return validation_error_msg(&msg),
        Err(_) => return internal_error(),
    };

    let enriched = match enrich_with_slugs(&state, &[row]).await {
        Ok(mut v) => v.remove(0),
        Err(_) => return internal_error(),
    };

    (StatusCode::OK, Json(enriched)).into_response()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Executes the main search SQL and returns raw ticket rows.
async fn exec_search_query(
    state: &AppState,
    built: &search::BuiltQuery,
) -> Result<Vec<crate::models::TicketRow>, RepoError> {
    let mut query = sqlx::query_as::<_, crate::models::TicketRow>(&built.sql);
    for val in &built.binds {
        query = match val {
            search::SqlValue::Text(t) => query.bind(t.clone()),
            search::SqlValue::Int(i) => query.bind(*i),
            search::SqlValue::Float(f) => query.bind(*f),
        };
    }
    let rows = query.fetch_all(&state.pool).await?;
    Ok(rows)
}

/// Executes the count SQL for FTS offset pagination.
async fn exec_count_query(state: &AppState, built: &search::BuiltQuery) -> Result<i64, RepoError> {
    let count_sql = built.count_sql.as_deref().unwrap_or("SELECT 0");
    let mut query = sqlx::query_scalar::<_, i64>(count_sql);
    for val in &built.count_binds {
        query = match val {
            search::SqlValue::Text(t) => query.bind(t.clone()),
            search::SqlValue::Int(i) => query.bind(*i),
            search::SqlValue::Float(f) => query.bind(*f),
        };
    }
    let total = query.fetch_one(&state.pool).await?;
    Ok(total)
}

/// Enriches ticket rows with relations and computes slugs.
async fn enrich_with_slugs(
    state: &AppState,
    rows: &[crate::models::TicketRow],
) -> Result<Vec<TicketResponse>, RepoError> {
    let mut responses = repos::ticket::enrich_many(&state.pool, rows).await?;

    // Compute slugs if slug_cache is available.
    if let Some(ref cache) = state.slug_cache {
        let component_ids: Vec<i64> = rows.iter().map(|r| r.component_id).collect();
        // Resolve effective slugs; if slug resolution fails (e.g., missing root slug),
        // degrade gracefully — leave slug as None.
        if let Ok(slug_map) = cache.resolve_many(&component_ids).await {
            for (resp, row) in responses.iter_mut().zip(rows.iter()) {
                if let Some(effective) = slug_map.get(&row.component_id) {
                    resp.slug = Some(format!("{effective}-{}", row.id));
                    resp.component.effective_slug = Some(effective.clone());
                }
            }
        }
    }

    Ok(responses)
}

/// Creates comment #0 (the ticket description).
async fn create_description_comment(
    state: &AppState,
    ticket_id: i64,
    author_id: i64,
    body: &str,
) -> Result<(), RepoError> {
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO comments (ticket_id, number, author_id, body, created_at, updated_at)
         VALUES (?, 0, ?, ?, ?, ?)",
    )
    .bind(ticket_id)
    .bind(author_id)
    .bind(body)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Error responses (consistent JSON format per DD 0.4 §5.3)
// ---------------------------------------------------------------------------

fn not_found(message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": message,
        })),
    )
        .into_response()
}

fn forbidden(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": "forbidden",
            "message": message,
        })),
    )
        .into_response()
}

fn validation_error(field: &str, message: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "error": "validation_error",
            "message": "Request validation failed.",
            "details": { field: message },
        })),
    )
        .into_response()
}

fn validation_error_msg(message: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "error": "validation_error",
            "message": message,
        })),
    )
        .into_response()
}

fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal_error",
            "message": "An internal error occurred.",
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::db;
    use crate::models::{CreateComponentRequest, CreateUserRequest, Role};
    use crate::repos::{component, session, user};
    use crate::slug::SlugCache;

    use super::*;

    async fn setup() -> (AppState, String) {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();

        let admin_req = CreateUserRequest {
            login: "admin".to_string(),
            display_name: "Admin User".to_string(),
            email: "admin@example.com".to_string(),
            password: None,
            role: Some(Role::Admin),
        };
        let admin = user::create(&pool, &admin_req, None).await.unwrap();

        let sess = session::create(&pool, admin.id).await.unwrap();

        let comp_req = CreateComponentRequest {
            name: "Platform".to_string(),
            parent_id: None,
            slug: Some("PLAT".to_string()),
            owner_id: admin.id,
        };
        component::create(&pool, &comp_req).await.unwrap();

        let slug_cache = SlugCache::new(&pool).await.unwrap();

        let state = AppState {
            pool,
            oidc: None,
            slug_cache: Some(slug_cache),
        };

        (state, sess.id)
    }

    fn app(state: AppState) -> axum::Router {
        crate::api::build_router_with_state(state)
    }

    async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn list_tickets_empty() {
        let (state, token) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get("/api/tickets")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["items"].as_array().unwrap().len(), 0);
        assert_eq!(json["has_more"], false);
    }

    #[tokio::test]
    async fn create_and_get_ticket() {
        let (state, token) = setup().await;
        let router = app(state);

        // Get component ID.
        let create_body = serde_json::json!({
            "type": "bug",
            "title": "Test bug",
            "owner_id": 1,
            "component_id": 1,
            "priority": "P1",
            "description": "Bug description here."
        });

        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/tickets")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["title"], "Test bug");
        assert_eq!(json["type"], "bug");
        assert_eq!(json["priority"], "P1");
        assert_eq!(json["status"], "new");
        assert_eq!(json["slug"], "PLAT-1");
        assert_eq!(json["comment_count"], 1);

        // GET the created ticket.
        let ticket_id = json["id"].as_i64().unwrap();
        let resp = router
            .oneshot(
                Request::get(&format!("/api/tickets/{ticket_id}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["id"], ticket_id);
        assert_eq!(json["slug"], "PLAT-1");
    }

    #[tokio::test]
    async fn update_ticket_status() {
        let (state, token) = setup().await;
        let router = app(state);

        // Create a ticket first.
        let create_body = serde_json::json!({
            "type": "feature",
            "title": "New feature",
            "owner_id": 1,
            "component_id": 1,
        });

        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/tickets")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(resp).await;
        let id = created["id"].as_i64().unwrap();

        // Update status.
        let update_body = serde_json::json!({
            "status": "in_progress",
        });

        let resp = router
            .oneshot(
                Request::patch(&format!("/api/tickets/{id}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "in_progress");
    }

    #[tokio::test]
    async fn get_ticket_not_found() {
        let (state, token) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get("/api/tickets/9999")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unauthenticated_request_returns_401() {
        let (state, _token) = setup().await;
        let resp = app(state)
            .oneshot(Request::get("/api/tickets").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_ticket_validation() {
        let (state, token) = setup().await;

        // Empty title.
        let body = serde_json::json!({
            "type": "bug",
            "title": "",
            "owner_id": 1,
            "component_id": 1,
        });

        let resp = app(state)
            .oneshot(
                Request::post("/api/tickets")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn title_change_by_non_creator_forbidden() {
        let (state, token) = setup().await;
        let router = app(state.clone());

        // Create a second user (non-admin).
        let user_req = CreateUserRequest {
            login: "regular".to_string(),
            display_name: "Regular User".to_string(),
            email: "regular@example.com".to_string(),
            password: None,
            role: Some(Role::User),
        };
        let regular = user::create(&state.pool, &user_req, None).await.unwrap();
        let sess2 = session::create(&state.pool, regular.id).await.unwrap();

        // Admin creates a ticket.
        let create_body = serde_json::json!({
            "type": "bug",
            "title": "Admin's ticket",
            "owner_id": 1,
            "component_id": 1,
        });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/tickets")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(resp).await;
        let id = created["id"].as_i64().unwrap();

        // Non-creator, non-admin tries to change title.
        let update_body = serde_json::json!({ "title": "Hijacked!" });
        let resp = router
            .oneshot(
                Request::patch(&format!("/api/tickets/{id}"))
                    .header("Cookie", format!("s9_session={}", sess2.id))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_with_pagination() {
        let (state, token) = setup().await;
        let router = app(state);

        // Create 3 tickets.
        for i in 1..=3 {
            let body = serde_json::json!({
                "type": "bug",
                "title": format!("Ticket {i}"),
                "owner_id": 1,
                "component_id": 1,
            });
            router
                .clone()
                .oneshot(
                    Request::post("/api/tickets")
                        .header("Cookie", format!("s9_session={token}"))
                        .header("Content-Type", "application/json")
                        .body(Body::from(serde_json::to_vec(&body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // First page of 2.
        let resp = router
            .clone()
            .oneshot(
                Request::get("/api/tickets?page_size=2")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["items"].as_array().unwrap().len(), 2);
        assert_eq!(json["has_more"], true);
        let cursor = json["next_cursor"].as_str().unwrap();

        // Second page using cursor.
        let resp = router
            .oneshot(
                Request::get(&format!("/api/tickets?page_size=2&cursor={cursor}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
        assert_eq!(json["has_more"], false);
    }
}
