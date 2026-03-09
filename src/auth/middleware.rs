use std::ops::Deref;

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::COOKIE;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::api::AppState;
use crate::models::Role;
use crate::repos;

/// Authenticated user extracted from the session cookie.
///
/// Include this in a handler's arguments to require authentication.
/// The extractor validates the `s9_session` cookie, checks the session
/// and user in the database, and extends the session's sliding window
/// when it is past the halfway mark.
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
enum AuthError {
    Unauthorized,
    Forbidden,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({
                    "error": "unauthorized",
                    "message": "Authentication required."
                })),
            )
                .into_response(),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                axum::Json(json!({
                    "error": "forbidden",
                    "message": "Administrator access required."
                })),
            )
                .into_response(),
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

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token =
            extract_session_cookie(parts).ok_or_else(|| AuthError::Unauthorized.into_response())?;

        let session = repos::session::get_valid(&state.pool, token)
            .await
            .map_err(|_| AuthError::Unauthorized.into_response())?
            .ok_or_else(|| AuthError::Unauthorized.into_response())?;

        let user = repos::user::get_by_id(&state.pool, session.user_id)
            .await
            .map_err(|_| AuthError::Unauthorized.into_response())?
            .ok_or_else(|| AuthError::Unauthorized.into_response())?;

        if user.is_active == 0 {
            return Err(AuthError::Unauthorized.into_response());
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
    use axum::http::Request;
    use chrono::{Duration, Utc};
    use sqlx::SqlitePool;

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
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
        };
        let mut parts = make_parts(None);

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn auth_user_invalid_token() {
        let pool = test_pool().await;
        let state = AppState {
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
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
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
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
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
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
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
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
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
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
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
        };
        let mut parts = make_parts(Some(&format!("s9_session={}", sess.id)));

        let result = RequireAdmin::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
    }
}
