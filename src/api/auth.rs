use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::{COOKIE, HeaderValue, SET_COOKIE};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::AppState;
use crate::auth::middleware::AuthUser;
use crate::auth::password::{dummy_verify, verify_password};
use crate::models::Role;
use crate::repos;

use super::error;

/// Max-Age for a session cookie (30 days).
const COOKIE_MAX_AGE: i64 = 30 * 24 * 60 * 60;

/// Credentials for password login.
#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    login: String,
    password: String,
}

/// Public user info returned from auth endpoints.
#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    id: i64,
    login: String,
    display_name: String,
    email: String,
    role: Role,
}

/// Build a `Set-Cookie` header value for the session token.
fn session_cookie(token: &str, max_age: i64) -> HeaderValue {
    let value =
        format!("s9_session={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age}");
    HeaderValue::from_str(&value).expect("cookie value is valid ASCII")
}

/// `POST /api/auth/login` — authenticate with login and password.
#[utoipa::path(
    post, path = "/api/auth/login", tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
    )
)]
pub async fn login(State(state): State<AppState>, Json(body): Json<LoginRequest>) -> Response {
    let user = match repos::user::get_by_login(&state.pool, &body.login).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Burn CPU to prevent timing-based user enumeration.
            let pw = body.password.clone();
            tokio::task::spawn_blocking(move || dummy_verify(&pw))
                .await
                .ok();
            return error::unauthorized("Invalid login or password.");
        }
        Err(_) => return error::internal_error(),
    };

    if user.is_active == 0 {
        let pw = body.password.clone();
        tokio::task::spawn_blocking(move || dummy_verify(&pw))
            .await
            .ok();
        return error::unauthorized("Invalid login or password.");
    }

    let hash = match &user.password_hash {
        Some(h) => h.clone(),
        None => {
            // OIDC-only user — no password to verify.
            let pw = body.password.clone();
            tokio::task::spawn_blocking(move || dummy_verify(&pw))
                .await
                .ok();
            return error::unauthorized("Invalid login or password.");
        }
    };

    let pw = body.password.clone();
    let matched = tokio::task::spawn_blocking(move || verify_password(&pw, &hash))
        .await
        .unwrap_or(Ok(false));

    match matched {
        Ok(true) => {}
        Ok(false) => return error::unauthorized("Invalid login or password."),
        Err(_) => return error::internal_error(),
    }

    let session = match repos::session::create(&state.pool, user.id).await {
        Ok(s) => s,
        Err(_) => return error::internal_error(),
    };

    let resp = AuthResponse {
        id: user.id,
        login: user.login,
        display_name: user.display_name,
        email: user.email,
        role: user.role,
    };

    (
        StatusCode::OK,
        [(SET_COOKIE, session_cookie(&session.id, COOKIE_MAX_AGE))],
        Json(resp),
    )
        .into_response()
}

/// Extract the `s9_session` token from a `Cookie` header value.
fn extract_session_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .flat_map(|value| value.to_str().ok())
        .flat_map(|s| s.split("; "))
        .find_map(|pair| pair.strip_prefix("s9_session="))
        .map(|s| s.to_string())
}

/// `POST /api/auth/logout` — clear the session cookie and delete the session.
#[utoipa::path(
    post, path = "/api/auth/logout", tag = "Auth",
    responses((status = 204, description = "Session destroyed")),
    security(("session_cookie" = []))
)]
pub async fn logout(State(state): State<AppState>, headers: axum::http::HeaderMap) -> Response {
    if let Some(token) = extract_session_token(&headers) {
        let _ = repos::session::delete(&state.pool, &token).await;
    }

    (
        StatusCode::NO_CONTENT,
        [(SET_COOKIE, session_cookie("", 0))],
    )
        .into_response()
}

/// `GET /api/auth/me` — return the authenticated user's info.
#[utoipa::path(
    get, path = "/api/auth/me", tag = "Auth",
    responses((status = 200, description = "Current user info", body = AuthResponse)),
    security(("session_cookie" = []))
)]
pub async fn me(user: AuthUser) -> Json<AuthResponse> {
    Json(AuthResponse {
        id: user.id,
        login: user.login,
        display_name: user.display_name,
        email: user.email,
        role: user.role,
    })
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use sqlx::SqlitePool;
    use tower::ServiceExt;

    use crate::api::build_router;
    use crate::auth::password::hash_password;
    use crate::db;
    use crate::models::{CreateUserRequest, Role};
    use crate::repos::{session, user};

    async fn test_pool() -> SqlitePool {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();
        pool
    }

    async fn create_user(pool: &SqlitePool, login: &str, password: Option<&str>) -> i64 {
        let hash = password.map(|p| hash_password(p).unwrap());
        let req = CreateUserRequest {
            login: login.to_string(),
            display_name: format!("User {login}"),
            email: format!("{login}@example.com"),
            password: None,
            role: Some(Role::User),
        };
        user::create(pool, &req, hash.as_deref()).await.unwrap().id
    }

    fn login_request(login: &str, password: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({
                    "login": login,
                    "password": password
                }))
                .unwrap(),
            ))
            .unwrap()
    }

    async fn body_json(resp: axum::response::Response) -> Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn extract_set_cookie(resp: &axum::response::Response) -> Option<String> {
        resp.headers()
            .get("set-cookie")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }

    // --- Login tests ---

    #[tokio::test]
    async fn login_success() {
        let pool = test_pool().await;
        create_user(&pool, "alice", Some("correct_password")).await;
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(login_request("alice", "correct_password"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let cookie = extract_set_cookie(&resp).unwrap();
        assert!(cookie.contains("s9_session="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=2592000"));

        let body = body_json(resp).await;
        assert_eq!(body["login"], "alice");
        assert_eq!(body["display_name"], "User alice");
        assert_eq!(body["email"], "alice@example.com");
        assert_eq!(body["role"], "user");
        assert!(body["id"].is_number());
    }

    #[tokio::test]
    async fn login_wrong_password() {
        let pool = test_pool().await;
        create_user(&pool, "bob", Some("correct_password")).await;
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(login_request("bob", "wrong_password"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "unauthorized");
    }

    #[tokio::test]
    async fn login_unknown_user() {
        let pool = test_pool().await;
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(login_request("nonexistent", "password123"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "unauthorized");
    }

    #[tokio::test]
    async fn login_inactive_user() {
        let pool = test_pool().await;
        let uid = create_user(&pool, "inactive", Some("password123")).await;
        sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
            .bind(uid)
            .execute(&pool)
            .await
            .unwrap();
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(login_request("inactive", "password123"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "unauthorized");
    }

    #[tokio::test]
    async fn login_no_password_hash() {
        let pool = test_pool().await;
        create_user(&pool, "oidcuser", None).await;
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(login_request("oidcuser", "password123"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "unauthorized");
    }

    // --- Logout tests ---

    #[tokio::test]
    async fn logout_clears_session() {
        let pool = test_pool().await;
        let uid = create_user(&pool, "charlie", Some("password123")).await;
        let sess = session::create(&pool, uid).await.unwrap();
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool.clone(),
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header("Cookie", format!("s9_session={}", sess.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let cookie = extract_set_cookie(&resp).unwrap();
        assert!(cookie.contains("Max-Age=0"));

        // Session should be gone from DB.
        let found = session::get_valid(&pool, &sess.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn logout_no_cookie() {
        let pool = test_pool().await;
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    // --- Me tests ---

    #[tokio::test]
    async fn me_authenticated() {
        let pool = test_pool().await;
        let uid = create_user(&pool, "diana", Some("password123")).await;
        let sess = session::create(&pool, uid).await.unwrap();
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/auth/me")
                    .header("Cookie", format!("s9_session={}", sess.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(body["login"], "diana");
        assert_eq!(body["email"], "diana@example.com");
        assert_eq!(body["role"], "user");
    }

    #[tokio::test]
    async fn me_unauthenticated() {
        let pool = test_pool().await;
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
