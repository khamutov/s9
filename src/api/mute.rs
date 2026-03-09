//! Per-ticket mute/unmute endpoints (DD 0.4 §13).
//!
//! - `POST /api/tickets/:id/mute` — mute notifications for current user.
//! - `DELETE /api/tickets/:id/mute` — unmute notifications for current user.
//! - `GET /api/tickets/:id/mute` — check mute status for current user.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::auth::middleware::AuthUser;

use super::AppState;
use super::error::{internal_error, not_found};

/// Response for `GET /api/tickets/:id/mute`.
#[derive(Serialize, ToSchema)]
pub struct MuteStatusResponse {
    pub muted: bool,
}

/// Verifies a ticket exists, returning its ID or a 404 response.
async fn ensure_ticket_exists(pool: &sqlx::SqlitePool, ticket_id: i64) -> Result<(), Response> {
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM tickets WHERE id = ?")
        .bind(ticket_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| internal_error())?;
    if exists.is_none() {
        return Err(not_found("Ticket not found"));
    }
    Ok(())
}

/// `POST /api/tickets/:id/mute` — mute notifications for this ticket.
///
/// Idempotent: calling when already muted is a no-op (DD 0.4 §13.1).
#[utoipa::path(
    post, path = "/api/tickets/{id}/mute", tag = "Tickets",
    params(("id" = i64, Path, description = "Ticket ID")),
    responses(
        (status = 204, description = "Muted"),
        (status = 404, description = "Ticket not found"),
    ),
    security(("session_cookie" = []))
)]
pub async fn mute_ticket(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(resp) = ensure_ticket_exists(&state.pool, id).await {
        return resp;
    }

    let result =
        sqlx::query("INSERT OR IGNORE INTO notification_mutes (user_id, ticket_id) VALUES (?, ?)")
            .bind(user.id)
            .bind(id)
            .execute(&state.pool)
            .await;

    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => internal_error(),
    }
}

/// `DELETE /api/tickets/:id/mute` — unmute notifications for this ticket.
///
/// Idempotent: calling when not muted is a no-op (DD 0.4 §13.2).
#[utoipa::path(
    delete, path = "/api/tickets/{id}/mute", tag = "Tickets",
    params(("id" = i64, Path, description = "Ticket ID")),
    responses(
        (status = 204, description = "Unmuted"),
        (status = 404, description = "Ticket not found"),
    ),
    security(("session_cookie" = []))
)]
pub async fn unmute_ticket(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(resp) = ensure_ticket_exists(&state.pool, id).await {
        return resp;
    }

    let result = sqlx::query("DELETE FROM notification_mutes WHERE user_id = ? AND ticket_id = ?")
        .bind(user.id)
        .bind(id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => internal_error(),
    }
}

/// `GET /api/tickets/:id/mute` — check mute status for this ticket.
#[utoipa::path(
    get, path = "/api/tickets/{id}/mute", tag = "Tickets",
    params(("id" = i64, Path, description = "Ticket ID")),
    responses(
        (status = 200, description = "Mute status", body = MuteStatusResponse),
        (status = 404, description = "Ticket not found"),
    ),
    security(("session_cookie" = []))
)]
pub async fn get_mute_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(resp) = ensure_ticket_exists(&state.pool, id).await {
        return resp;
    }

    let row: Result<Option<(i64,)>, _> = sqlx::query_as(
        "SELECT user_id FROM notification_mutes WHERE user_id = ? AND ticket_id = ?",
    )
    .bind(user.id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some(_)) => (
            StatusCode::OK,
            axum::Json(MuteStatusResponse { muted: true }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::OK,
            axum::Json(MuteStatusResponse { muted: false }),
        )
            .into_response(),
        Err(_) => internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::db;
    use crate::events::EventBus;
    use crate::models::{CreateComponentRequest, CreateUserRequest, Role};
    use crate::repos::{component, session, user};

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

        // Create a ticket.
        let now = chrono::Utc::now();
        let ticket_id: i64 = sqlx::query_scalar(
            "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, created_by, created_at, updated_at)
             VALUES ('bug', 'Test ticket', 'new', 'P3', ?, 1, ?, ?, ?) RETURNING id",
        )
        .bind(admin.id)
        .bind(admin.id)
        .bind(now)
        .bind(now)
        .fetch_one(&pool)
        .await
        .unwrap();

        let state = AppState {
            notif_producer: crate::notifications::NotificationProducer::new(
                pool.clone(),
                120,
                false,
            ),
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
            event_bus: EventBus::new(),
        };

        (state, sess.id.clone(), ticket_id)
    }

    fn app(state: AppState) -> axum::Router {
        crate::api::build_router_with_state(state)
    }

    #[tokio::test]
    async fn mute_returns_204() {
        let (state, token, ticket_id) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::post(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn mute_is_idempotent() {
        let (state, token, ticket_id) = setup().await;
        let router = app(state);

        // First mute.
        let resp = router
            .clone()
            .oneshot(
                Request::post(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Second mute — should still be 204.
        let resp = router
            .oneshot(
                Request::post(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn unmute_returns_204() {
        let (state, token, ticket_id) = setup().await;
        let router = app(state);

        // Mute first.
        router
            .clone()
            .oneshot(
                Request::post(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Unmute.
        let resp = router
            .oneshot(
                Request::delete(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn unmute_is_idempotent() {
        let (state, token, ticket_id) = setup().await;
        // Unmute when not muted — should still be 204.
        let resp = app(state)
            .oneshot(
                Request::delete(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn mute_ticket_not_found() {
        let (state, token, _) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::post("/api/tickets/9999/mute")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn mute_requires_auth() {
        let (state, _, ticket_id) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::post(format!("/api/tickets/{ticket_id}/mute"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_mute_status_false() {
        let (state, token, ticket_id) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["muted"], false);
    }

    #[tokio::test]
    async fn get_mute_status_true_after_mute() {
        let (state, token, ticket_id) = setup().await;
        let router = app(state);

        // Mute.
        router
            .clone()
            .oneshot(
                Request::post(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Check status.
        let resp = router
            .oneshot(
                Request::get(format!("/api/tickets/{ticket_id}/mute"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["muted"], true);
    }
}
