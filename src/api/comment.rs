//! Comment API endpoints: list, create, edit, delete.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::auth::middleware::{AuthUser, RequireAdmin};
use crate::models::{CommentResponse, CreateCommentRequest, EditCommentRequest, Role};
use crate::notifications::NotifEvent;
use crate::repos::{self, RepoError};

use super::AppState;
use super::error::{forbidden, internal_error, not_found, validation_error};

/// Query parameters for `GET /api/tickets/:id/comments`.
#[derive(Debug, Deserialize)]
pub struct ListCommentsQuery {
    #[serde(default)]
    pub include_edits: bool,
}

/// Path parameters for comment endpoints scoped to a ticket.
#[derive(Debug, Deserialize)]
pub struct TicketPath {
    pub id: i64,
}

/// Path parameters for endpoints addressing a specific comment by number.
#[derive(Debug, Deserialize)]
pub struct CommentPath {
    pub id: i64,
    pub num: i64,
}

/// `GET /api/tickets/:id/comments` — list all comments for a ticket.
#[utoipa::path(
    get, path = "/api/tickets/{id}/comments", tag = "Comments",
    params(
        ("id" = i64, Path, description = "Ticket ID"),
        ("include_edits" = Option<bool>, Query, description = "Include edit history"),
    ),
    responses(
        (status = 200, description = "List of comments", body = Vec<CommentResponse>),
        (status = 404, description = "Ticket not found"),
    ),
    security(("session_cookie" = []))
)]
pub async fn list_comments(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(path): Path<TicketPath>,
    Query(params): Query<ListCommentsQuery>,
) -> Response {
    // Verify ticket exists.
    match repos::ticket::get_by_id(&state.pool, path.id).await {
        Ok(Some(_)) => {}
        Ok(None) => return not_found("Ticket not found"),
        Err(_) => return internal_error(),
    }

    let rows = match repos::comment::list_by_ticket(&state.pool, path.id).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    let enriched = match repos::comment::enrich_many(&state.pool, &rows, params.include_edits).await
    {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    (StatusCode::OK, Json(json!({ "items": enriched }))).into_response()
}

/// `POST /api/tickets/:id/comments` — create a new comment on a ticket.
#[utoipa::path(
    post, path = "/api/tickets/{id}/comments", tag = "Comments",
    params(("id" = i64, Path, description = "Ticket ID")),
    request_body = CreateCommentRequest,
    responses(
        (status = 201, description = "Comment created", body = CommentResponse),
        (status = 404, description = "Ticket not found"),
        (status = 422, description = "Validation error"),
    ),
    security(("session_cookie" = []))
)]
pub async fn create_comment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(path): Path<TicketPath>,
    Json(body): Json<CreateCommentRequest>,
) -> Response {
    if body.body.is_empty() {
        return validation_error("body", "Body is required.");
    }

    let row = match repos::comment::create(&state.pool, path.id, &body, user.id).await {
        Ok(r) => r,
        Err(RepoError::NotFound) => return not_found("Ticket not found"),
        Err(_) => return internal_error(),
    };

    let enriched = match repos::comment::enrich(&state.pool, &row, false).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    // Parse @mentions and resolve to user IDs (best-effort).
    let mentioned_logins = crate::mentions::parse_mentions(&body.body);
    let mut mentioned_ids = Vec::new();
    for login in &mentioned_logins {
        if let Ok(Some(u)) = repos::user::get_by_login(&state.pool, login).await {
            mentioned_ids.push(u.id);
        }
    }

    // Emit notification event (best-effort).
    let _ = state
        .notif_producer
        .emit_with_mentions(
            path.id,
            NotifEvent::CommentAdded,
            user.id,
            json!({"actor": user.login}),
            &mentioned_ids,
        )
        .await;

    (StatusCode::CREATED, Json(enriched)).into_response()
}

/// `PATCH /api/tickets/:id/comments/:num` — edit a comment.
///
/// Only the comment author or an admin may edit.
#[utoipa::path(
    patch, path = "/api/tickets/{id}/comments/{num}", tag = "Comments",
    params(
        ("id" = i64, Path, description = "Ticket ID"),
        ("num" = i64, Path, description = "Comment number"),
    ),
    request_body = EditCommentRequest,
    responses(
        (status = 200, description = "Comment updated", body = CommentResponse),
        (status = 403, description = "Not the author or admin"),
        (status = 404, description = "Comment not found"),
    ),
    security(("session_cookie" = []))
)]
pub async fn edit_comment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(path): Path<CommentPath>,
    Json(body): Json<EditCommentRequest>,
) -> Response {
    if body.body.is_empty() {
        return validation_error("body", "Body is required.");
    }

    let comment =
        match repos::comment::get_by_ticket_and_number(&state.pool, path.id, path.num).await {
            Ok(Some(c)) => c,
            Ok(None) => return not_found("Comment not found"),
            Err(_) => return internal_error(),
        };

    // Authorization: comment author or admin.
    if comment.author_id != user.id && user.role != Role::Admin {
        return forbidden("Only the comment author or an admin can edit this comment.");
    }

    let updated = match repos::comment::update(&state.pool, comment.id, &body).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    let enriched = match repos::comment::enrich(&state.pool, &updated, false).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    (StatusCode::OK, Json(enriched)).into_response()
}

/// `DELETE /api/tickets/:id/comments/:num` — delete a comment (admin only).
///
/// Comment #0 (ticket description) cannot be deleted.
#[utoipa::path(
    delete, path = "/api/tickets/{id}/comments/{num}", tag = "Comments",
    params(
        ("id" = i64, Path, description = "Ticket ID"),
        ("num" = i64, Path, description = "Comment number"),
    ),
    responses(
        (status = 204, description = "Comment deleted"),
        (status = 403, description = "Admin only"),
        (status = 404, description = "Comment not found"),
        (status = 422, description = "Cannot delete comment #0"),
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_comment(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Path(path): Path<CommentPath>,
) -> Response {
    // Suppress unused variable warning — admin extraction enforces authorization.
    let _ = &admin;

    if path.num == 0 {
        return super::error::validation_error_msg(
            "Cannot delete comment #0 (ticket description).",
        );
    }

    let comment =
        match repos::comment::get_by_ticket_and_number(&state.pool, path.id, path.num).await {
            Ok(Some(c)) => c,
            Ok(None) => return not_found("Comment not found"),
            Err(_) => return internal_error(),
        };

    match repos::comment::delete(&state.pool, comment.id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => internal_error(),
    }
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

    async fn setup() -> (AppState, String, i64) {
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
            notif_producer: crate::notifications::NotificationProducer::new(
                pool.clone(),
                120,
                false,
            ),
            pool,
            oidc: None,
            slug_cache: Some(slug_cache),
            data_dir: std::path::PathBuf::from("/tmp/test"),
            event_bus: crate::events::EventBus::new(),
        };

        (state, sess.id, admin.id)
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

    async fn create_ticket(router: &axum::Router, token: &str) -> i64 {
        let body = serde_json::json!({
            "type": "bug",
            "title": "Test ticket",
            "owner_id": 1,
            "component_id": 1,
            "description": "Ticket description"
        });
        let resp = router
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
        let json = body_json(resp).await;
        json["id"].as_i64().unwrap()
    }

    #[tokio::test]
    async fn list_comments_for_ticket() {
        let (state, token, _) = setup().await;
        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        let resp = router
            .oneshot(
                Request::get(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        // Comment #0 (description) should be present.
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
        assert_eq!(json["items"][0]["number"], 0);
    }

    #[tokio::test]
    async fn list_comments_ticket_not_found() {
        let (state, token, _) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get("/api/tickets/9999/comments")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_and_list_comment() {
        let (state, token, _) = setup().await;
        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        // Create a comment.
        let body = serde_json::json!({ "body": "New comment here" });
        let resp = router
            .clone()
            .oneshot(
                Request::post(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["number"], 1);
        assert_eq!(json["body"], "New comment here");
        assert_eq!(json["author"]["login"], "admin");
        assert_eq!(json["edit_count"], 0);

        // Verify list now shows 2 comments.
        let resp = router
            .oneshot(
                Request::get(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["items"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn create_comment_empty_body() {
        let (state, token, _) = setup().await;
        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        let body = serde_json::json!({ "body": "" });
        let resp = router
            .oneshot(
                Request::post(&format!("/api/tickets/{ticket_id}/comments"))
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
    async fn edit_comment_by_author() {
        let (state, token, _) = setup().await;
        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        // Create comment.
        let body = serde_json::json!({ "body": "Original" });
        let resp = router
            .clone()
            .oneshot(
                Request::post(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(resp).await;
        let num = created["number"].as_i64().unwrap();

        // Edit comment.
        let edit_body = serde_json::json!({ "body": "Revised" });
        let resp = router
            .oneshot(
                Request::patch(&format!("/api/tickets/{ticket_id}/comments/{num}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&edit_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["body"], "Revised");
        assert_eq!(json["edit_count"], 1);
    }

    #[tokio::test]
    async fn edit_comment_forbidden_for_non_author() {
        let (state, token, _) = setup().await;

        // Create a regular user.
        let user_req = CreateUserRequest {
            login: "regular".to_string(),
            display_name: "Regular User".to_string(),
            email: "regular@example.com".to_string(),
            password: None,
            role: Some(Role::User),
        };
        let regular = user::create(&state.pool, &user_req, None).await.unwrap();
        let sess2 = session::create(&state.pool, regular.id).await.unwrap();

        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        // Admin creates a comment.
        let body = serde_json::json!({ "body": "Admin's comment" });
        router
            .clone()
            .oneshot(
                Request::post(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Regular user tries to edit it.
        let edit_body = serde_json::json!({ "body": "Hijacked" });
        let resp = router
            .oneshot(
                Request::patch(&format!("/api/tickets/{ticket_id}/comments/1"))
                    .header("Cookie", format!("s9_session={}", sess2.id))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&edit_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn delete_comment_admin_only() {
        let (state, token, _) = setup().await;

        // Create a regular user.
        let user_req = CreateUserRequest {
            login: "regular".to_string(),
            display_name: "Regular User".to_string(),
            email: "regular@example.com".to_string(),
            password: None,
            role: Some(Role::User),
        };
        let regular = user::create(&state.pool, &user_req, None).await.unwrap();
        let sess2 = session::create(&state.pool, regular.id).await.unwrap();

        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        // Create a comment to delete.
        let body = serde_json::json!({ "body": "To be deleted" });
        router
            .clone()
            .oneshot(
                Request::post(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Regular user tries to delete — should get 403.
        let resp = router
            .clone()
            .oneshot(
                Request::delete(&format!("/api/tickets/{ticket_id}/comments/1"))
                    .header("Cookie", format!("s9_session={}", sess2.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Admin deletes — should get 204.
        let resp = router
            .oneshot(
                Request::delete(&format!("/api/tickets/{ticket_id}/comments/1"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_comment_zero_rejected() {
        let (state, token, _) = setup().await;
        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        let resp = router
            .oneshot(
                Request::delete(&format!("/api/tickets/{ticket_id}/comments/0"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn list_comments_with_edits() {
        let (state, token, _) = setup().await;
        let router = app(state);
        let ticket_id = create_ticket(&router, &token).await;

        // Create and edit a comment.
        let body = serde_json::json!({ "body": "v1" });
        router
            .clone()
            .oneshot(
                Request::post(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let edit_body = serde_json::json!({ "body": "v2" });
        router
            .clone()
            .oneshot(
                Request::patch(&format!("/api/tickets/{ticket_id}/comments/1"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&edit_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // List without edits.
        let resp = router
            .clone()
            .oneshot(
                Request::get(&format!("/api/tickets/{ticket_id}/comments"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        let comment1 = &json["items"][1];
        assert_eq!(comment1["edit_count"], 1);
        assert!(comment1["edits"].as_array().unwrap().is_empty());

        // List with edits.
        let resp = router
            .oneshot(
                Request::get(&format!(
                    "/api/tickets/{ticket_id}/comments?include_edits=true"
                ))
                .header("Cookie", format!("s9_session={token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        let comment1 = &json["items"][1];
        assert_eq!(comment1["edits"].as_array().unwrap().len(), 1);
        assert_eq!(comment1["edits"][0]["old_body"], "v1");
    }
}
