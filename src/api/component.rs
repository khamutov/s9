//! Component API endpoints: list, create, update, delete.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::auth::middleware::{AuthUser, RequireAdmin};
use crate::models::{
    CompactUser, ComponentResponse, CreateComponentRequest, UpdateComponentRequest,
};
use crate::repos::{self, RepoError};

use super::AppState;
use super::error::{conflict, internal_error, not_found, validation_error};

/// `GET /api/components` — list all components as a flat list.
#[utoipa::path(
    get, path = "/api/components", tag = "Components",
    responses((status = 200, description = "List of components", body = Vec<ComponentResponse>)),
    security(("session_cookie" = []))
)]
pub async fn list_components(State(state): State<AppState>, _user: AuthUser) -> Response {
    let rows = match repos::component::list(&state.pool).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        let owner = match repos::user::get_by_id(&state.pool, row.owner_id).await {
            Ok(Some(u)) => CompactUser::from(&u),
            Ok(None) => return internal_error(),
            Err(_) => return internal_error(),
        };

        let ticket_count = match count_tickets_for_component(&state, row.id).await {
            Ok(n) => n,
            Err(_) => return internal_error(),
        };

        let effective_slug = resolve_effective_slug(&state, row.id).await;

        items.push(ComponentResponse {
            id: row.id,
            name: row.name.clone(),
            parent_id: row.parent_id,
            path: row.path.clone(),
            slug: row.slug.clone(),
            effective_slug,
            owner,
            ticket_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }

    (StatusCode::OK, Json(json!({ "items": items }))).into_response()
}

/// `POST /api/components` — create a new component (admin only).
#[utoipa::path(
    post, path = "/api/components", tag = "Components",
    request_body = CreateComponentRequest,
    responses(
        (status = 201, description = "Component created", body = ComponentResponse),
        (status = 403, description = "Admin only"),
        (status = 409, description = "Duplicate name"),
        (status = 422, description = "Validation error"),
    ),
    security(("session_cookie" = []))
)]
pub async fn create_component(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Json(body): Json<CreateComponentRequest>,
) -> Response {
    let _ = &admin;

    if body.name.is_empty() {
        return validation_error("name", "Name is required.");
    }

    let row = match repos::component::create(&state.pool, &body).await {
        Ok(r) => r,
        Err(RepoError::NotFound) => {
            return validation_error("parent_id", "Parent component not found.");
        }
        Err(RepoError::Conflict(msg)) => return conflict(&msg),
        Err(_) => return internal_error(),
    };

    // Reload slug cache after component mutation.
    if let Some(ref cache) = state.slug_cache {
        let _ = cache.reload(&state.pool).await;
    }

    let owner = match repos::user::get_by_id(&state.pool, row.owner_id).await {
        Ok(Some(u)) => CompactUser::from(&u),
        _ => return internal_error(),
    };

    let effective_slug = resolve_effective_slug(&state, row.id).await;

    let resp = ComponentResponse {
        id: row.id,
        name: row.name,
        parent_id: row.parent_id,
        path: row.path,
        slug: row.slug,
        effective_slug,
        owner,
        ticket_count: 0,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    (StatusCode::CREATED, Json(resp)).into_response()
}

/// `PATCH /api/components/:id` — update a component (admin only).
#[utoipa::path(
    patch, path = "/api/components/{id}", tag = "Components",
    params(("id" = i64, Path, description = "Component ID")),
    request_body = UpdateComponentRequest,
    responses(
        (status = 200, description = "Component updated", body = ComponentResponse),
        (status = 403, description = "Admin only"),
        (status = 404, description = "Component not found"),
        (status = 409, description = "Conflict"),
    ),
    security(("session_cookie" = []))
)]
pub async fn update_component(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Path(id): Path<i64>,
    Json(body): Json<UpdateComponentRequest>,
) -> Response {
    let _ = &admin;

    let row = match repos::component::update(&state.pool, id, &body).await {
        Ok(r) => r,
        Err(RepoError::NotFound) => return not_found("Component not found"),
        Err(RepoError::Conflict(msg)) => return conflict(&msg),
        Err(_) => return internal_error(),
    };

    // Reload slug cache after component mutation.
    if let Some(ref cache) = state.slug_cache {
        let _ = cache.reload(&state.pool).await;
    }

    let owner = match repos::user::get_by_id(&state.pool, row.owner_id).await {
        Ok(Some(u)) => CompactUser::from(&u),
        _ => return internal_error(),
    };

    let ticket_count = match count_tickets_for_component(&state, row.id).await {
        Ok(n) => n,
        Err(_) => return internal_error(),
    };

    let effective_slug = resolve_effective_slug(&state, row.id).await;

    let resp = ComponentResponse {
        id: row.id,
        name: row.name,
        parent_id: row.parent_id,
        path: row.path,
        slug: row.slug,
        effective_slug,
        owner,
        ticket_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

/// `DELETE /api/components/:id` — delete a component (admin only).
///
/// Rejects if the component has child components or assigned tickets.
#[utoipa::path(
    delete, path = "/api/components/{id}", tag = "Components",
    params(("id" = i64, Path, description = "Component ID")),
    responses(
        (status = 204, description = "Component deleted"),
        (status = 403, description = "Admin only"),
        (status = 404, description = "Component not found"),
        (status = 409, description = "Has children or tickets"),
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_component(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Path(id): Path<i64>,
) -> Response {
    let _ = &admin;

    match repos::component::delete(&state.pool, id).await {
        Ok(()) => {
            // Reload slug cache after component mutation.
            if let Some(ref cache) = state.slug_cache {
                let _ = cache.reload(&state.pool).await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(RepoError::NotFound) => not_found("Component not found"),
        Err(RepoError::Conflict(msg)) => conflict(&msg),
        Err(_) => internal_error(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Count tickets assigned to a specific component.
async fn count_tickets_for_component(
    state: &AppState,
    component_id: i64,
) -> Result<i64, RepoError> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tickets WHERE component_id = ?")
        .bind(component_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(count)
}

/// Resolve the effective slug for a component, returning None on failure.
async fn resolve_effective_slug(state: &AppState, component_id: i64) -> Option<String> {
    if let Some(ref cache) = state.slug_cache {
        cache.resolve_effective_slug(component_id).await.ok()
    } else {
        None
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

    async fn setup() -> (AppState, String, String) {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();

        // Create admin user.
        let admin_req = CreateUserRequest {
            login: "admin".to_string(),
            display_name: "Admin User".to_string(),
            email: "admin@example.com".to_string(),
            password: None,
            role: Some(Role::Admin),
        };
        let admin = user::create(&pool, &admin_req, None).await.unwrap();
        let admin_sess = session::create(&pool, admin.id).await.unwrap();

        // Create regular user.
        let user_req = CreateUserRequest {
            login: "regular".to_string(),
            display_name: "Regular User".to_string(),
            email: "regular@example.com".to_string(),
            password: None,
            role: Some(Role::User),
        };
        let regular = user::create(&pool, &user_req, None).await.unwrap();
        let regular_sess = session::create(&pool, regular.id).await.unwrap();

        // Seed a root component so slug cache has data.
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
            data_dir: std::path::PathBuf::from("/tmp/test"),
            event_bus: crate::events::EventBus::new(),
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
    async fn list_components_returns_seeded() {
        let (state, token, _) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get("/api/components")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "Platform");
        assert_eq!(items[0]["slug"], "PLAT");
        assert_eq!(items[0]["effective_slug"], "PLAT");
        assert_eq!(items[0]["ticket_count"], 0);
        assert_eq!(items[0]["owner"]["login"], "admin");
    }

    #[tokio::test]
    async fn create_component_admin_only() {
        let (state, _, regular_token) = setup().await;
        let body = serde_json::json!({
            "name": "Auth",
            "parent_id": null,
            "slug": "AUTH",
            "owner_id": 1
        });

        let resp = app(state)
            .oneshot(
                Request::post("/api/components")
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
    async fn create_and_list_component() {
        let (state, token, _) = setup().await;
        let router = app(state);

        let body = serde_json::json!({
            "name": "Auth",
            "parent_id": null,
            "slug": "AUTH",
            "owner_id": 1
        });

        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/components")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["name"], "Auth");
        assert_eq!(json["slug"], "AUTH");
        assert_eq!(json["effective_slug"], "AUTH");
        assert_eq!(json["path"], "/Auth/");
        assert_eq!(json["ticket_count"], 0);

        // List should now have 2 components.
        let resp = router
            .oneshot(
                Request::get("/api/components")
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
    async fn create_child_component_inherits_slug() {
        let (state, token, _) = setup().await;
        let router = app(state);

        let body = serde_json::json!({
            "name": "Networking",
            "parent_id": 1,
            "owner_id": 1
        });

        let resp = router
            .oneshot(
                Request::post("/api/components")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["slug"], serde_json::Value::Null);
        assert_eq!(json["effective_slug"], "PLAT");
        assert_eq!(json["path"], "/Platform/Networking/");
    }

    #[tokio::test]
    async fn create_duplicate_name_conflict() {
        let (state, token, _) = setup().await;
        let router = app(state);

        let body = serde_json::json!({
            "name": "Platform",
            "parent_id": null,
            "slug": "PLAT2",
            "owner_id": 1
        });

        let resp = router
            .oneshot(
                Request::post("/api/components")
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
    async fn update_component_rename() {
        let (state, token, _) = setup().await;
        let router = app(state);

        let body = serde_json::json!({ "name": "Infrastructure" });

        let resp = router
            .oneshot(
                Request::patch("/api/components/1")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["name"], "Infrastructure");
        assert_eq!(json["path"], "/Infrastructure/");
    }

    #[tokio::test]
    async fn update_component_not_found() {
        let (state, token, _) = setup().await;
        let body = serde_json::json!({ "name": "Ghost" });

        let resp = app(state)
            .oneshot(
                Request::patch("/api/components/9999")
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
    async fn delete_component_success() {
        let (state, token, _) = setup().await;
        let router = app(state.clone());

        // Create a component to delete (no tickets, no children).
        let body = serde_json::json!({
            "name": "Temp",
            "parent_id": null,
            "slug": "TEMP",
            "owner_id": 1
        });
        let resp = router
            .clone()
            .oneshot(
                Request::post("/api/components")
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
                Request::delete(&format!("/api/components/{id}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_component_with_children_rejected() {
        let (state, token, _) = setup().await;
        let router = app(state);

        // Create a child of Platform (id=1).
        let body = serde_json::json!({
            "name": "Child",
            "parent_id": 1,
            "owner_id": 1
        });
        router
            .clone()
            .oneshot(
                Request::post("/api/components")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Try to delete Platform — should fail.
        let resp = router
            .oneshot(
                Request::delete("/api/components/1")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn delete_component_non_admin_forbidden() {
        let (state, _, regular_token) = setup().await;

        let resp = app(state)
            .oneshot(
                Request::delete("/api/components/1")
                    .header("Cookie", format!("s9_session={regular_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
