use std::ops::Deref;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::COOKIE;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::AppState;
use crate::api::error;
use crate::models::Role;
use crate::repos;

/// Authenticated user extracted from the session cookie.
///
/// Include this in a handler's arguments to require authentication.
/// If the `require_auth` middleware has already run, the validated user
/// is read from request extensions (no extra DB lookup). Otherwise the
/// extractor performs the full validation itself.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub login: String,
    pub display_name: String,
    pub email: String,
    pub role: Role,
    pub session_id: String,
}

/// Wrapper extractor that requires the authenticated user to be an admin.
///
/// Extracts `AuthUser` first, then verifies `role == Role::Admin`.
/// Returns 403 if the user is not an administrator.
#[derive(Debug, Clone)]
pub struct RequireAdmin(pub AuthUser);

impl Deref for RequireAdmin {
    type Target = AuthUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Error type for authentication/authorization failures.
pub(crate) enum AuthError {
    Unauthorized,
    Forbidden,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized => error::unauthorized("Authentication required."),
            Self::Forbidden => error::forbidden("Administrator access required."),
        }
    }
}

/// Extracts the `s9_session` token value from the `Cookie` header.
pub(crate) fn extract_session_cookie(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get_all(COOKIE)
        .iter()
        .flat_map(|value| value.to_str().ok())
        .flat_map(|s| s.split("; "))
        .find_map(|pair| pair.strip_prefix("s9_session="))
}

/// Validates the session cookie and returns an `AuthUser` or an auth error.
///
/// Shared by both the middleware layer and the `AuthUser` extractor.
async fn validate_session(parts: &Parts, state: &AppState) -> Result<AuthUser, AuthError> {
    let token = extract_session_cookie(parts).ok_or(AuthError::Unauthorized)?;

    let session = repos::session::get_valid(&state.pool, token)
        .await
        .map_err(|_| AuthError::Unauthorized)?
        .ok_or(AuthError::Unauthorized)?;

    let user = repos::user::get_by_id(&state.pool, session.user_id)
        .await
        .map_err(|_| AuthError::Unauthorized)?
        .ok_or(AuthError::Unauthorized)?;

    if user.is_active == 0 {
        return Err(AuthError::Unauthorized);
    }

    // Extend sliding window if past the halfway mark (fire-and-forget).
    if repos::session::needs_extension(&session) {
        let pool = state.pool.clone();
        let token = session.id.clone();
        tokio::spawn(async move {
            if let Err(e) = repos::session::extend(&pool, &token).await {
                tracing::warn!("failed to extend session: {e}");
            }
        });
    }

    Ok(AuthUser {
        id: user.id,
        login: user.login,
        display_name: user.display_name,
        email: user.email,
        role: user.role,
        session_id: session.id,
    })
}

/// Route-level middleware that enforces authentication.
///
/// Apply via `axum::middleware::from_fn_with_state` on route groups that
/// require a valid session. The validated `AuthUser` is stored in request
/// extensions so that downstream extractors (`AuthUser`, `RequireAdmin`)
/// can retrieve it without a second database lookup.
pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    let (mut parts, body) = request.into_parts();
    let user = validate_session(&parts, &state)
        .await
        .map_err(|e| e.into_response())?;
    parts.extensions.insert(user);
    request = Request::from_parts(parts, body);
    Ok(next.run(request).await)
}

/// Route-level middleware that enforces admin role.
///
/// Must be applied *after* `require_auth` (or on a route group that already
/// guarantees an `AuthUser` in extensions). Returns 403 if the user is not
/// an admin, 401 if no user is present.
pub async fn require_admin(request: Request, next: Next) -> Result<Response, Response> {
    let user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or_else(|| AuthError::Unauthorized.into_response())?;
    if user.role != Role::Admin {
        return Err(AuthError::Forbidden.into_response());
    }
    Ok(next.run(request).await)
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Reuse the user injected by the require_auth middleware layer.
        if let Some(user) = parts.extensions.get::<AuthUser>() {
            return Ok(user.clone());
        }

        // Fallback: full validation (for routes without the middleware layer).
        validate_session(parts, state)
            .await
            .map_err(|e| e.into_response())
    }
}

impl FromRequestParts<AppState> for RequireAdmin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if user.role != Role::Admin {
            return Err(AuthError::Forbidden.into_response());
        }
        Ok(RequireAdmin(user))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::FromRequestParts;
    use axum::http::{Request, StatusCode};
    use chrono::{Duration, Utc};
    use sqlx::SqlitePool;

    use tower::ServiceExt;

    use crate::db;
    use crate::models::CreateUserRequest;
    use crate::repos::{session, user};

    async fn test_pool() -> SqlitePool {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();
        pool
    }

    async fn create_test_user(pool: &SqlitePool, role: Option<Role>) -> i64 {
        let req = CreateUserRequest {
            login: "testuser".to_string(),
            display_name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            password: None,
            role,
        };
        user::create(pool, &req, None).await.unwrap().id
    }

    fn make_parts(cookie: Option<&str>) -> Parts {
        let mut builder = Request::builder().uri("/api/test");
        if let Some(cookie_val) = cookie {
            builder = builder.header("Cookie", cookie_val);
        }
        let (parts, _body) = builder.body(Body::empty()).unwrap().into_parts();
        parts
    }

    // --- Cookie parser unit tests ---

    #[test]
    fn extract_cookie_found() {
        let parts = make_parts(Some("s9_session=abc123"));
        assert_eq!(extract_session_cookie(&parts), Some("abc123"));
    }

    #[test]
    fn extract_cookie_missing() {
        let parts = make_parts(None);
        assert_eq!(extract_session_cookie(&parts), None);
    }

    #[test]
    fn extract_cookie_other_cookies() {
        let parts = make_parts(Some("theme=dark; lang=en"));
        assert_eq!(extract_session_cookie(&parts), None);
    }

    #[test]
    fn extract_cookie_multiple() {
        let parts = make_parts(Some("theme=dark; s9_session=tok42; lang=en"));
        assert_eq!(extract_session_cookie(&parts), Some("tok42"));
    }

    // --- AuthUser extractor integration tests ---

    #[tokio::test]
    async fn auth_user_missing_cookie() {
        let pool = test_pool().await;
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
            event_bus: crate::events::EventBus::new(),
        };
        let mut parts = make_parts(None);

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn auth_user_invalid_token() {
        let pool = test_pool().await;
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
            event_bus: crate::events::EventBus::new(),
        };
        let mut parts = make_parts(Some("s9_session=nonexistent"));

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn auth_user_expired_session() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, None).await;
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("expired-tok")
        .bind(uid)
        .bind(now - Duration::hours(1))
        .bind(now)
        .execute(&pool)
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
            event_bus: crate::events::EventBus::new(),
        };
        let mut parts = make_parts(Some("s9_session=expired-tok"));

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn auth_user_inactive_user() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, None).await;

        // Deactivate the user.
        sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
            .bind(uid)
            .execute(&pool)
            .await
            .unwrap();

        let sess = session::create(&pool, uid).await.unwrap();
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
            event_bus: crate::events::EventBus::new(),
        };
        let mut parts = make_parts(Some(&format!("s9_session={}", sess.id)));

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn auth_user_valid() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, Some(Role::Admin)).await;
        let sess = session::create(&pool, uid).await.unwrap();

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
            event_bus: crate::events::EventBus::new(),
        };
        let mut parts = make_parts(Some(&format!("s9_session={}", sess.id)));

        let auth = AuthUser::from_request_parts(&mut parts, &state)
            .await
            .unwrap();
        assert_eq!(auth.id, uid);
        assert_eq!(auth.login, "testuser");
        assert_eq!(auth.display_name, "Test User");
        assert_eq!(auth.email, "test@example.com");
        assert_eq!(auth.role, Role::Admin);
        assert_eq!(auth.session_id, sess.id);
    }

    // --- RequireAdmin extractor tests ---

    #[tokio::test]
    async fn require_admin_allows_admin() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, Some(Role::Admin)).await;
        let sess = session::create(&pool, uid).await.unwrap();

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
            event_bus: crate::events::EventBus::new(),
        };
        let mut parts = make_parts(Some(&format!("s9_session={}", sess.id)));

        let admin = RequireAdmin::from_request_parts(&mut parts, &state)
            .await
            .unwrap();
        assert_eq!(admin.role, Role::Admin);
        assert_eq!(admin.id, uid);
    }

    #[tokio::test]
    async fn require_admin_rejects_user() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, Some(Role::User)).await;
        let sess = session::create(&pool, uid).await.unwrap();

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
            event_bus: crate::events::EventBus::new(),
        };
        let mut parts = make_parts(Some(&format!("s9_session={}", sess.id)));

        let result = RequireAdmin::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
    }

    // --- Middleware layer integration tests ---

    fn build_test_app(pool: SqlitePool) -> axum::Router {
        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        crate::api::build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        )
    }

    #[tokio::test]
    async fn middleware_blocks_unauthenticated_protected_route() {
        let pool = test_pool().await;
        let app = build_test_app(pool);

        let resp: axum::response::Response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tickets")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_allows_public_routes() {
        let pool = test_pool().await;
        let app = build_test_app(pool);

        // POST /api/auth/logout should work without session (returns 204).
        let resp: axum::response::Response = app
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

    #[tokio::test]
    async fn middleware_admin_layer_blocks_non_admin() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, Some(Role::User)).await;
        let sess = session::create(&pool, uid).await.unwrap();
        let app = build_test_app(pool);

        let resp: axum::response::Response = app
            .oneshot(
                Request::builder()
                    .uri("/api/users")
                    .header("Cookie", format!("s9_session={}", sess.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn middleware_admin_layer_allows_admin() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, Some(Role::Admin)).await;
        let sess = session::create(&pool, uid).await.unwrap();
        let app = build_test_app(pool);

        let resp: axum::response::Response = app
            .oneshot(
                Request::builder()
                    .uri("/api/users")
                    .header("Cookie", format!("s9_session={}", sess.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extractor_reuses_middleware_injected_user() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool, Some(Role::User)).await;
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
            event_bus: crate::events::EventBus::new(),
        };

        // Pre-inject an AuthUser into extensions (simulates what the middleware does).
        let injected = AuthUser {
            id: uid,
            login: "testuser".to_string(),
            display_name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            role: Role::User,
            session_id: "fake-session".to_string(),
        };
        let builder = Request::builder().uri("/api/test");
        let (mut parts, _) = builder.body(Body::empty()).unwrap().into_parts();
        parts.extensions.insert(injected);

        // AuthUser extractor should return the injected user without a DB lookup.
        let user = AuthUser::from_request_parts(&mut parts, &state)
            .await
            .unwrap();
        assert_eq!(user.id, uid);
        assert_eq!(user.session_id, "fake-session");
    }
}
