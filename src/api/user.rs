//! User management API endpoints: list, create, update, set password.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::auth::middleware::{AuthUser, RequireAdmin};
use crate::auth::password;
use crate::models::{
    CompactUser, CreateUserRequest, FullUser, Role, SetPasswordRequest, UpdateUserRequest,
};
use crate::repos::{self, RepoError};

use super::AppState;
use super::error::{conflict, forbidden, internal_error, not_found, validation_error};

/// Query parameters for `GET /api/users`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub include_inactive: bool,
}

/// `GET /api/users/compact` — list all active users as compact objects (any authenticated user).
#[utoipa::path(
    get, path = "/api/users/compact", tag = "Users",
    responses(
        (status = 200, description = "Compact list of active users", body = Vec<CompactUser>),
    ),
    security(("session_cookie" = []))
)]
pub async fn list_compact_users(State(state): State<AppState>, _user: AuthUser) -> Response {
    let rows = match repos::user::list(&state.pool, false).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    let items: Vec<CompactUser> = rows.iter().map(CompactUser::from).collect();
    (StatusCode::OK, Json(json!({ "items": items }))).into_response()
}

/// `GET /api/users` — list all users (admin only).
#[utoipa::path(
    get, path = "/api/users", tag = "Users",
    params(("include_inactive" = Option<bool>, Query, description = "Include deactivated users")),
    responses(
        (status = 200, description = "List of users", body = Vec<FullUser>),
        (status = 403, description = "Admin only"),
    ),
    security(("session_cookie" = []))
)]
pub async fn list_users(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Query(query): Query<ListQuery>,
) -> Response {
    let _ = &admin;

    let rows = match repos::user::list(&state.pool, query.include_inactive).await {
        Ok(r) => r,
        Err(_) => return internal_error(),
    };

    let items: Vec<FullUser> = rows.iter().map(FullUser::from).collect();
    (StatusCode::OK, Json(json!({ "items": items }))).into_response()
}

/// `POST /api/users` — create a new user (admin only).
#[utoipa::path(
    post, path = "/api/users", tag = "Users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = FullUser),
        (status = 403, description = "Admin only"),
        (status = 409, description = "Duplicate login"),
        (status = 422, description = "Validation error"),
    ),
    security(("session_cookie" = []))
)]
pub async fn create_user(
    State(state): State<AppState>,
    admin: RequireAdmin,
    Json(body): Json<CreateUserRequest>,
) -> Response {
    let _ = &admin;

    if body.login.is_empty() {
        return validation_error("login", "Login is required.");
    }
    if body.display_name.is_empty() {
        return validation_error("display_name", "Display name is required.");
    }
    if body.email.is_empty() {
        return validation_error("email", "Email is required.");
    }

    let hash = match &body.password {
        Some(pw) => {
            if let Err(e) = password::validate_policy(pw) {
                return validation_error("password", &e.to_string());
            }
            match password::hash_password(pw) {
                Ok(h) => Some(h),
                Err(_) => return internal_error(),
            }
        }
        None => None,
    };

    let row = match repos::user::create(&state.pool, &body, hash.as_deref()).await {
        Ok(r) => r,
        Err(RepoError::Conflict(_)) => return conflict("A user with this login already exists."),
        Err(_) => return internal_error(),
    };

    (StatusCode::CREATED, Json(FullUser::from(&row))).into_response()
}

/// `PATCH /api/users/:id` — update a user.
///
/// Admin can change: role, is_active, display_name, email.
/// Self (non-admin) can change: display_name, email.
#[utoipa::path(
    patch, path = "/api/users/{id}", tag = "Users",
    params(("id" = i64, Path, description = "User ID")),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = FullUser),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "User not found"),
    ),
    security(("session_cookie" = []))
)]
pub async fn update_user(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateUserRequest>,
) -> Response {
    let is_self = user.id == id;
    let is_admin = user.role == Role::Admin;

    // Non-admin can only edit own profile.
    if !is_admin && !is_self {
        return forbidden("You can only edit your own profile.");
    }

    // Non-admin cannot change role or is_active.
    if !is_admin && (body.role.is_some() || body.is_active.is_some()) {
        return forbidden("Only admins can change role or active status.");
    }

    let row = match repos::user::update(&state.pool, id, &body).await {
        Ok(r) => r,
        Err(RepoError::NotFound) => return not_found("User not found."),
        Err(_) => return internal_error(),
    };

    // When deactivating a user, delete all their sessions (DD 0.3 §7.5).
    if body.is_active == Some(false) {
        let _ = repos::user::delete_sessions_for_user(&state.pool, id).await;
    }

    (StatusCode::OK, Json(FullUser::from(&row))).into_response()
}

/// `POST /api/users/:id/password` — set/change a user's password.
///
/// Self: requires current_password. Admin: can set without current_password.
#[utoipa::path(
    post, path = "/api/users/{id}/password", tag = "Users",
    params(("id" = i64, Path, description = "User ID")),
    request_body = SetPasswordRequest,
    responses(
        (status = 204, description = "Password changed"),
        (status = 401, description = "Incorrect current password"),
        (status = 403, description = "Cannot change another user's password"),
        (status = 422, description = "Password too short"),
    ),
    security(("session_cookie" = []))
)]
pub async fn set_password(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<SetPasswordRequest>,
) -> Response {
    let is_self = user.id == id;
    let is_admin = user.role == Role::Admin;

    // Non-admin can only change own password.
    if !is_admin && !is_self {
        return forbidden("You can only change your own password.");
    }

    // Self-service requires current_password.
    if is_self && !is_admin {
        let current = match &body.current_password {
            Some(p) => p,
            None => {
                return validation_error("current_password", "Current password is required.");
            }
        };

        let target_user = match repos::user::get_by_id(&state.pool, id).await {
            Ok(Some(u)) => u,
            Ok(None) => return not_found("User not found."),
            Err(_) => return internal_error(),
        };

        let hash = match &target_user.password_hash {
            Some(h) => h,
            None => {
                return super::error::unauthorized("Incorrect current password.");
            }
        };

        match password::verify_password(current, hash) {
            Ok(true) => {}
            Ok(false) => {
                return super::error::unauthorized("Incorrect current password.");
            }
            Err(_) => return internal_error(),
        }
    }

    if let Err(e) = password::validate_policy(&body.new_password) {
        return validation_error("new_password", &e.to_string());
    }

    let hash = match password::hash_password(&body.new_password) {
        Ok(h) => h,
        Err(_) => return internal_error(),
    };

    match repos::user::set_password(&state.pool, id, &hash).await {
        Ok(false) => return not_found("User not found."),
        Ok(true) => {}
        Err(_) => return internal_error(),
    }

    // Delete all other sessions for the user (force re-login).
    let _ = repos::session::delete_others_for_user(&state.pool, id, &user.session_id).await;

    StatusCode::NO_CONTENT.into_response()
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

    async fn setup() -> (AppState, String, String, i64, i64) {
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

        (state, admin_sess.id, regular_sess.id, admin.id, regular.id)
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
    async fn list_users_requires_admin() {
        let (state, _, regular_token, _, _) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get("/api/users")
                    .header("Cookie", format!("s9_session={regular_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_users_requires_auth() {
        let (state, _, _, _, _) = setup().await;
        let resp = app(state)
            .oneshot(Request::get("/api/users").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_users_admin_success() {
        let (state, token, _, _, _) = setup().await;
        let resp = app(state)
            .oneshot(
                Request::get("/api/users")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        // Verify FullUser fields are present.
        assert!(items[0]["has_password"].is_boolean());
        assert!(items[0]["has_oidc"].is_boolean());
        assert!(items[0]["email"].is_string());
    }

    #[tokio::test]
    async fn list_users_exclude_inactive() {
        let (state, token, _, _, regular_id) = setup().await;
        let router = app(state.clone());

        // Deactivate the regular user.
        repos::user::update(
            &state.pool,
            regular_id,
            &UpdateUserRequest {
                display_name: None,
                email: None,
                role: None,
                is_active: Some(false),
            },
        )
        .await
        .unwrap();

        // Default: exclude inactive.
        let resp = router
            .clone()
            .oneshot(
                Request::get("/api/users")
                    .header("Cookie", format!("s9_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["items"].as_array().unwrap().len(), 1);

        // include_inactive=true.
        let resp = router
            .oneshot(
                Request::get("/api/users?include_inactive=true")
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
    async fn create_user_admin_only() {
        let (state, _, regular_token, _, _) = setup().await;
        let body = json!({
            "login": "newbie",
            "display_name": "New User",
            "email": "new@example.com"
        });

        let resp = app(state)
            .oneshot(
                Request::post("/api/users")
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
    async fn create_user_success() {
        let (state, token, _, _, _) = setup().await;
        let body = json!({
            "login": "newuser",
            "display_name": "New User",
            "email": "new@example.com",
            "password": "securepass123",
            "role": "admin"
        });

        let resp = app(state)
            .oneshot(
                Request::post("/api/users")
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["login"], "newuser");
        assert_eq!(json["display_name"], "New User");
        assert_eq!(json["email"], "new@example.com");
        assert_eq!(json["role"], "admin");
        assert_eq!(json["is_active"], true);
        assert_eq!(json["has_password"], true);
        assert_eq!(json["has_oidc"], false);
    }

    #[tokio::test]
    async fn create_user_duplicate_login() {
        let (state, token, _, _, _) = setup().await;
        let body = json!({
            "login": "admin",
            "display_name": "Another Admin",
            "email": "admin2@example.com"
        });

        let resp = app(state)
            .oneshot(
                Request::post("/api/users")
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
    async fn create_user_short_password() {
        let (state, token, _, _, _) = setup().await;
        let body = json!({
            "login": "shortpw",
            "display_name": "Short PW",
            "email": "short@example.com",
            "password": "1234567"
        });

        let resp = app(state)
            .oneshot(
                Request::post("/api/users")
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
    async fn update_user_self_edit() {
        let (state, _, regular_token, _, regular_id) = setup().await;
        let body = json!({
            "display_name": "Updated Name"
        });

        let resp = app(state)
            .oneshot(
                Request::patch(&format!("/api/users/{regular_id}"))
                    .header("Cookie", format!("s9_session={regular_token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["display_name"], "Updated Name");
    }

    #[tokio::test]
    async fn update_user_non_admin_cannot_change_role() {
        let (state, _, regular_token, _, regular_id) = setup().await;
        let body = json!({ "role": "admin" });

        let resp = app(state)
            .oneshot(
                Request::patch(&format!("/api/users/{regular_id}"))
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
    async fn update_user_non_admin_cannot_edit_others() {
        let (state, _, regular_token, admin_id, _) = setup().await;
        let body = json!({ "display_name": "Hacked" });

        let resp = app(state)
            .oneshot(
                Request::patch(&format!("/api/users/{admin_id}"))
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
    async fn update_user_admin_can_deactivate() {
        let (state, token, _, _, regular_id) = setup().await;
        let body = json!({ "is_active": false });

        let resp = app(state)
            .oneshot(
                Request::patch(&format!("/api/users/{regular_id}"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["is_active"], false);
    }

    #[tokio::test]
    async fn update_user_not_found() {
        let (state, token, _, _, _) = setup().await;
        let body = json!({ "display_name": "Ghost" });

        let resp = app(state)
            .oneshot(
                Request::patch("/api/users/9999")
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
    async fn set_password_admin_no_current_required() {
        let (state, token, _, _, regular_id) = setup().await;
        let body = json!({ "new_password": "newsecurepass" });

        let resp = app(state)
            .oneshot(
                Request::post(&format!("/api/users/{regular_id}/password"))
                    .header("Cookie", format!("s9_session={token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn set_password_short_rejected() {
        let (state, token, _, _, regular_id) = setup().await;
        let body = json!({ "new_password": "short" });

        let resp = app(state)
            .oneshot(
                Request::post(&format!("/api/users/{regular_id}/password"))
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
    async fn set_password_non_admin_cannot_change_others() {
        let (state, _, regular_token, admin_id, _) = setup().await;
        let body = json!({ "new_password": "hackedpass123" });

        let resp = app(state)
            .oneshot(
                Request::post(&format!("/api/users/{admin_id}/password"))
                    .header("Cookie", format!("s9_session={regular_token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
