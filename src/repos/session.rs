#![allow(dead_code)]

use chrono::{Duration, Utc};
use rand_core::{OsRng, RngCore};
use sqlx::SqlitePool;

use crate::models::SessionRow;

use super::RepoError;

/// Initial and sliding-window TTL for sessions.
const SESSION_TTL_DAYS: i64 = 30;

/// Extend the session when remaining life drops below this threshold.
const SLIDING_THRESHOLD_DAYS: i64 = 15;

/// Generates a cryptographically random 64-character hex session token.
pub fn generate_token() -> String {
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Creates a new session for the given user and returns the inserted row.
pub async fn create(pool: &SqlitePool, user_id: i64) -> Result<SessionRow, RepoError> {
    let token = generate_token();
    let now = Utc::now();
    let expires_at = now + Duration::days(SESSION_TTL_DAYS);

    sqlx::query("INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)")
        .bind(&token)
        .bind(user_id)
        .bind(expires_at)
        .bind(now)
        .execute(pool)
        .await?;

    // Safe to unwrap: we just inserted the row and it cannot be expired yet.
    Ok(get_valid(pool, &token).await?.unwrap())
}

/// Looks up a session by token, returning `None` if missing or expired.
pub async fn get_valid(pool: &SqlitePool, token: &str) -> Result<Option<SessionRow>, RepoError> {
    let now = Utc::now();
    let row =
        sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = ? AND expires_at > ?")
            .bind(token)
            .bind(now)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// Extends a session's expiry by the full TTL from now.
pub async fn extend(pool: &SqlitePool, token: &str) -> Result<(), RepoError> {
    let expires_at = Utc::now() + Duration::days(SESSION_TTL_DAYS);
    sqlx::query("UPDATE sessions SET expires_at = ? WHERE id = ?")
        .bind(expires_at)
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Returns `true` if the session's remaining life is below the sliding threshold.
pub fn needs_extension(session: &SessionRow) -> bool {
    session.expires_at - Utc::now() < Duration::days(SLIDING_THRESHOLD_DAYS)
}

/// Deletes a session by token (idempotent — no error if absent).
pub async fn delete(pool: &SqlitePool, token: &str) -> Result<(), RepoError> {
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Deletes all sessions for a user except the specified one.
pub async fn delete_others_for_user(
    pool: &SqlitePool,
    user_id: i64,
    keep_token: &str,
) -> Result<(), RepoError> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND id != ?")
        .bind(user_id)
        .bind(keep_token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Removes all expired sessions and returns the number of rows deleted.
pub async fn delete_expired(pool: &SqlitePool) -> Result<u64, RepoError> {
    let now = Utc::now();
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::CreateUserRequest;
    use crate::repos::user;

    async fn test_pool() -> SqlitePool {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();
        pool
    }

    async fn create_test_user(pool: &SqlitePool) -> i64 {
        let req = CreateUserRequest {
            login: "sessionuser".to_string(),
            display_name: "Session User".to_string(),
            email: "session@example.com".to_string(),
            password: None,
            role: None,
        };
        user::create(pool, &req, None).await.unwrap().id
    }

    #[test]
    fn generate_token_length() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_uniqueness() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn create_and_get() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool).await;

        let session = create(&pool, uid).await.unwrap();
        assert_eq!(session.user_id, uid);
        assert_eq!(session.id.len(), 64);

        let fetched = get_valid(&pool, &session.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, session.id);
        assert_eq!(fetched.user_id, uid);
    }

    #[tokio::test]
    async fn get_valid_returns_none_for_missing() {
        let pool = test_pool().await;
        let result = get_valid(&pool, "nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_valid_filters_expired() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool).await;
        let now = Utc::now();
        let expired = now - Duration::hours(1);

        sqlx::query(
            "INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("expired-token")
        .bind(uid)
        .bind(expired)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let result = get_valid(&pool, "expired-token").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn extend_updates_expiry() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool).await;
        let session = create(&pool, uid).await.unwrap();
        let original_expiry = session.expires_at;

        // Small delay to ensure timestamps differ.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        extend(&pool, &session.id).await.unwrap();

        let refreshed = get_valid(&pool, &session.id).await.unwrap().unwrap();
        assert!(refreshed.expires_at > original_expiry);
    }

    #[test]
    fn needs_extension_true() {
        let session = SessionRow {
            id: "tok".to_string(),
            user_id: 1,
            expires_at: Utc::now() + Duration::days(14),
            created_at: Utc::now(),
        };
        assert!(needs_extension(&session));
    }

    #[test]
    fn needs_extension_false() {
        let session = SessionRow {
            id: "tok".to_string(),
            user_id: 1,
            expires_at: Utc::now() + Duration::days(16),
            created_at: Utc::now(),
        };
        assert!(!needs_extension(&session));
    }

    #[tokio::test]
    async fn delete_session() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool).await;
        let session = create(&pool, uid).await.unwrap();

        delete(&pool, &session.id).await.unwrap();

        let result = get_valid(&pool, &session.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_expired_removes_old() {
        let pool = test_pool().await;
        let uid = create_test_user(&pool).await;
        let now = Utc::now();

        // Insert one expired and one valid session.
        sqlx::query(
            "INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("old-session")
        .bind(uid)
        .bind(now - Duration::hours(1))
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("fresh-session")
        .bind(uid)
        .bind(now + Duration::days(10))
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let removed = delete_expired(&pool).await.unwrap();
        assert_eq!(removed, 1);

        // Valid session survives.
        let valid = get_valid(&pool, "fresh-session").await.unwrap();
        assert!(valid.is_some());
    }
}
