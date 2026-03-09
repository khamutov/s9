//! Milestone API endpoints: list, create, update, delete.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::auth::middleware::{AuthUser, RequireAdmin};
use crate::models::{CreateMilestoneRequest, MilestoneStatus, UpdateMilestoneRequest};
use crate::repos::{self, RepoError};

use super::AppState;

/// Query parameters for `GET /api/milestones`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub status: Option<MilestoneStatus>,
}

/// `GET /api/milestones` — list all milestones, optionally filtered by status.
pub async fn list_milestones(
    State(state): State<AppState>,
    _user: AuthUser,
    Query(query): Query<ListQuery>,
) -> Response {
    let rows = match repos::milestone::list(&state.pool, query.status).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    let items = match repos::milestone::enrich_many(&state.pool, rows).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    (StatusCode::OK, Json(json!({ "items": items }))).into_response()
}

/// `POST /api/milestones` — create a new milestone (admin only).
pub async fn create_milestone(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Json(body): Json<CreateMilestoneRequest>,
) -> Response {
    let _ = &admin;

    if body.name.is_empty() {
        return validation_error("name", "Name is required.");
    }

    let row = match repos::milestone::create(&state.pool, &body).await {
        Ok(r) => r,
        Err(RepoError::Conflict(msg)) => return conflict(&msg),
        Err(_) => return internal_error(),
    };

    let resp = match repos::milestone::enrich(&state.pool, row).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    (StatusCode::CREATED, Json(resp)).into_response()
}

/// `PATCH /api/milestones/:id` — update a milestone (admin only).
pub async fn update_milestone(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Path(id): Path<i64>,
    Json(body): Json<UpdateMilestoneRequest>,
) -> Response {
    let _ = &admin;

    let row = match repos::milestone::update(&state.pool, id, &body).await {
        Ok(r) => r,
        Err(RepoError::NotFound) => return not_found("Milestone not found"),
        Err(RepoError::Conflict(msg)) => return conflict(&msg),
        Err(_) => return internal_error(),
    };

    let resp = match repos::milestone::enrich(&state.pool, row).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    (StatusCode::OK, Json(resp)).into_response()
}

/// `DELETE /api/milestones/:id` — delete a milestone (admin only).
///
/// Rejects if the milestone has assigned tickets.
pub async fn delete_milestone(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Path(id): Path<i64>,
) -> Response {
    let _ = &admin;

    match repos::milestone::delete(&state.pool, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(RepoError::NotFound) => not_found("Milestone not found"),
        Err(RepoError::Conflict(msg)) => conflict(&msg),
        Err(_) => internal_error(),
    }
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

fn conflict(message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({
            "error": "conflict",
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

    async fn setup() -> (AppState, String, String) {
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
        let admin_sess = session::create(&pool, admin.id).await.unwrap();

        let user_req = CreateUserRequest {
            login: "regular".to_string(),
            display_name: "Regular User".to_string(),
            email: "regular@example.com".to_string(),
            password: None,
            role: Some(Role::User),
        };
        let regular = user::create(&pool, &user_req, None).await.unwrap();
        let regular_sess = session::create(&pool, regular.id).await.unwrap();

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

        (state, admin_sess.id, regular_sess.id)
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
    async fn list_milestones_empty() {
        let (state, token, _) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_milestones_requires_auth() {
        let (state, _, _) = setup().await;
        let resp = app(state)
            .oneshot(Request::get("/api/milestones").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_milestone_admin_only() {
        let (state, _, regular_token) = setup().await;
        let body = serde_json::json!({
            "name": "v1.0",
        });

        let resp = app(state)
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={regular_token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn create_and_list_milestone() {
        let (state, token, _) = setup().await;
        let router = app(state);

        let body = serde_json::json!({
            "name": "v2.0",
            "description": "Major release",
            "due_date": "2026-06-01"
        });

        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["name"], "v2.0");
        assert_eq!(json["description"], "Major release");
        assert_eq!(json["due_date"], "2026-06-01");
        assert_eq!(json["status"], "open");
        assert_eq!(json["stats"]["total"], 0);

        // List should now have 1 milestone.
        let resp = router
            .oneshot(
                Request::get("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(resp).await;
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn create_milestone_duplicate_name() {
        let (state, token, _) = setup().await;
        let router = app(state);
        let body = serde_json::json!({ "name": "dup" });

        // Create first.
        router
            .clone()
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Create duplicate.
        let resp = router
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn create_milestone_empty_name() {
        let (state, token, _) = setup().await;
        let body = serde_json::json!({ "name": "" });

        let resp = app(state)
            .oneshot(
                Request::post("/api/milestones")
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
    async fn update_milestone_success() {
        let (state, token, _) = setup().await;
        let router = app(state);

        // Create milestone.
        let body = serde_json::json!({ "name": "orig" });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(resp).await;
        let id = created["id"].as_i64().unwrap();

        // Update it.
        let body = serde_json::json!({ "name": "renamed", "status": "closed" });
        let resp = router
            .oneshot(
                Request::patch(&format!("/api/milestones/{id}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["name"], "renamed");
        assert_eq!(json["status"], "closed");
    }

    #[tokio::test]
    async fn update_milestone_not_found() {
        let (state, token, _) = setup().await;
        let body = serde_json::json!({ "name": "ghost" });

        let resp = app(state)
            .oneshot(
                Request::patch("/api/milestones/9999")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_milestone_success() {
        let (state, token, _) = setup().await;
        let router = app(state);

        // Create milestone to delete.
        let body = serde_json::json!({ "name": "doomed" });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(resp).await;
        let id = created["id"].as_i64().unwrap();

        let resp = router
            .oneshot(
                Request::delete(&format!("/api/milestones/{id}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_milestone_not_found() {
        let (state, token, _) = setup().await;

        let resp = app(state)
            .oneshot(
                Request::delete("/api/milestones/9999")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_milestones_filter_by_status() {
        let (state, token, _) = setup().await;
        let router = app(state);

        // Create two milestones.
        let body = serde_json::json!({ "name": "open-ms" });
        router
            .clone()
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = serde_json::json!({ "name": "closed-ms", "status": "closed" });
        router
            .clone()
            .oneshot(
                Request::post("/api/milestones")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Filter by open.
        let resp = router
            .clone()
            .oneshot(
                Request::get("/api/milestones?status=open")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "open-ms");

        // Filter by closed.
        let resp = router
            .oneshot(
                Request::get("/api/milestones?status=closed")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "closed-ms");
    }
}
