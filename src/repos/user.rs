use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{CreateUserRequest, Role, UpdateUserRequest, UserRow};

use super::RepoError;

/// Returns all users, optionally including inactive ones.
pub async fn list(pool: &SqlitePool, include_inactive: bool) -> Result<Vec<UserRow>, RepoError> {
    let rows = if include_inactive {
        sqlx::query_as::<_, UserRow>("SELECT * FROM users ORDER BY display_name")
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as::<_, UserRow>(
            "SELECT * FROM users WHERE is_active = 1 ORDER BY display_name",
        )
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

/// Finds a user by primary key.
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<UserRow>, RepoError> {
    let row = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Finds a user by login (for password authentication).
pub async fn get_by_login(pool: &SqlitePool, login: &str) -> Result<Option<UserRow>, RepoError> {
    let row = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE login = ?")
        .bind(login)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Finds a user by OIDC subject identifier (for OIDC callback).
pub async fn get_by_oidc_sub(
    pool: &SqlitePool,
    sub: &str,
) -> Result<Option<UserRow>, RepoError> {
    let row = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE oidc_sub = ?")
        .bind(sub)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Returns the total number of users.
pub async fn count(pool: &SqlitePool) -> Result<i64, RepoError> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Creates a new user and returns the inserted row.
pub async fn create(
    pool: &SqlitePool,
    req: &CreateUserRequest,
    password_hash: Option<&str>,
) -> Result<UserRow, RepoError> {
    let now = Utc::now();
    let role = req.role.unwrap_or(Role::User);

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (login, display_name, email, password_hash, role, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 1, ?, ?)
         RETURNING id",
    )
    .bind(&req.login)
    .bind(&req.display_name)
    .bind(&req.email)
    .bind(password_hash)
    .bind(role)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    // Safe to unwrap: we just inserted the row.
    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Applies a partial update to an existing user (read-merge-write).
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    req: &UpdateUserRequest,
) -> Result<UserRow, RepoError> {
    let existing = get_by_id(pool, id).await?.ok_or(RepoError::NotFound)?;

    let display_name = req.display_name.as_deref().unwrap_or(&existing.display_name);
    let email = req.email.as_deref().unwrap_or(&existing.email);
    let role = req.role.unwrap_or(existing.role);
    let is_active = req
        .is_active
        .map(|b| if b { 1i64 } else { 0 })
        .unwrap_or(existing.is_active);
    let now = Utc::now();

    sqlx::query(
        "UPDATE users SET display_name = ?, email = ?, role = ?, is_active = ?, updated_at = ? WHERE id = ?",
    )
    .bind(display_name)
    .bind(email)
    .bind(role)
    .bind(is_active)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Updates a user's password hash. Returns `false` if the user doesn't exist.
pub async fn set_password(
    pool: &SqlitePool,
    id: i64,
    password_hash: &str,
) -> Result<bool, RepoError> {
    let now = Utc::now();
    let result = sqlx::query(
        "UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?",
    )
    .bind(password_hash)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Sets the OIDC subject identifier. Returns `false` if the user doesn't exist.
pub async fn set_oidc_sub(
    pool: &SqlitePool,
    id: i64,
    oidc_sub: &str,
) -> Result<bool, RepoError> {
    let now = Utc::now();
    let result =
        sqlx::query("UPDATE users SET oidc_sub = ?, updated_at = ? WHERE id = ?")
            .bind(oidc_sub)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

/// Deletes all sessions for a user (used during deactivation).
pub async fn delete_sessions_for_user(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<(), RepoError> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::Role;

    async fn test_pool() -> SqlitePool {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();
        pool
    }

    fn make_create_request(login: &str) -> CreateUserRequest {
        CreateUserRequest {
            login: login.to_string(),
            display_name: format!("User {login}"),
            email: format!("{login}@example.com"),
            password: None,
            role: None,
        }
    }

    #[tokio::test]
    async fn create_and_get_by_id() {
        let pool = test_pool().await;
        let req = make_create_request("alice");
        let user = create(&pool, &req, None).await.unwrap();

        assert_eq!(user.login, "alice");
        assert_eq!(user.display_name, "User alice");
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.role, Role::User);
        assert_eq!(user.is_active, 1);
        assert!(user.password_hash.is_none());

        let fetched = get_by_id(&pool, user.id).await.unwrap().unwrap();
        assert_eq!(fetched.login, user.login);
    }

    #[tokio::test]
    async fn create_with_password_hash() {
        let pool = test_pool().await;
        let req = make_create_request("bob");
        let user = create(&pool, &req, Some("argon2hash")).await.unwrap();

        assert_eq!(user.password_hash.as_deref(), Some("argon2hash"));
    }

    #[tokio::test]
    async fn create_duplicate_login_conflict() {
        let pool = test_pool().await;
        let req = make_create_request("charlie");
        create(&pool, &req, None).await.unwrap();

        let result = create(&pool, &req, None).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn get_by_login_found() {
        let pool = test_pool().await;
        let req = make_create_request("dave");
        create(&pool, &req, None).await.unwrap();

        let found = get_by_login(&pool, "dave").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().login, "dave");
    }

    #[tokio::test]
    async fn get_by_login_not_found() {
        let pool = test_pool().await;
        let found = get_by_login(&pool, "nonexistent").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn list_excludes_inactive() {
        let pool = test_pool().await;
        let req_a = make_create_request("active");
        let req_i = make_create_request("inactive");
        create(&pool, &req_a, None).await.unwrap();
        let inactive = create(&pool, &req_i, None).await.unwrap();

        // Deactivate the second user.
        update(
            &pool,
            inactive.id,
            &UpdateUserRequest {
                display_name: None,
                email: None,
                role: None,
                is_active: Some(false),
            },
        )
        .await
        .unwrap();

        let active_only = list(&pool, false).await.unwrap();
        assert_eq!(active_only.len(), 1);
        assert_eq!(active_only[0].login, "active");
    }

    #[tokio::test]
    async fn list_includes_inactive() {
        let pool = test_pool().await;
        let req_a = make_create_request("active2");
        let req_i = make_create_request("inactive2");
        create(&pool, &req_a, None).await.unwrap();
        let inactive = create(&pool, &req_i, None).await.unwrap();

        update(
            &pool,
            inactive.id,
            &UpdateUserRequest {
                display_name: None,
                email: None,
                role: None,
                is_active: Some(false),
            },
        )
        .await
        .unwrap();

        let all = list(&pool, true).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn count_returns_correct_total() {
        let pool = test_pool().await;
        assert_eq!(count(&pool).await.unwrap(), 0);

        create(&pool, &make_create_request("u1"), None).await.unwrap();
        create(&pool, &make_create_request("u2"), None).await.unwrap();
        assert_eq!(count(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn update_partial_fields() {
        let pool = test_pool().await;
        let user = create(&pool, &make_create_request("eve"), None).await.unwrap();

        let updated = update(
            &pool,
            user.id,
            &UpdateUserRequest {
                display_name: Some("Eve Updated".into()),
                email: None,
                role: None,
                is_active: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.display_name, "Eve Updated");
        // Unchanged fields preserved.
        assert_eq!(updated.email, "eve@example.com");
        assert_eq!(updated.role, Role::User);
    }

    #[tokio::test]
    async fn update_not_found() {
        let pool = test_pool().await;
        let result = update(
            &pool,
            9999,
            &UpdateUserRequest {
                display_name: Some("Ghost".into()),
                email: None,
                role: None,
                is_active: None,
            },
        )
        .await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn set_password_updates_hash() {
        let pool = test_pool().await;
        let user = create(&pool, &make_create_request("frank"), None).await.unwrap();
        assert!(user.password_hash.is_none());

        let ok = set_password(&pool, user.id, "newhash").await.unwrap();
        assert!(ok);

        let refreshed = get_by_id(&pool, user.id).await.unwrap().unwrap();
        assert_eq!(refreshed.password_hash.as_deref(), Some("newhash"));
    }

    #[tokio::test]
    async fn set_password_not_found() {
        let pool = test_pool().await;
        let ok = set_password(&pool, 9999, "hash").await.unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn set_oidc_sub_and_get() {
        let pool = test_pool().await;
        let user = create(&pool, &make_create_request("grace"), None).await.unwrap();

        set_oidc_sub(&pool, user.id, "oidc-abc").await.unwrap();

        let found = get_by_oidc_sub(&pool, "oidc-abc").await.unwrap().unwrap();
        assert_eq!(found.id, user.id);
    }

    #[tokio::test]
    async fn delete_sessions_for_user_removes_rows() {
        let pool = test_pool().await;
        let user = create(&pool, &make_create_request("heidi"), None).await.unwrap();

        // Insert a session directly.
        let now = Utc::now();
        sqlx::query("INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)")
            .bind("sess-1")
            .bind(user.id)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();

        let (before,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
                .bind(user.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, 1);

        delete_sessions_for_user(&pool, user.id).await.unwrap();

        let (after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
                .bind(user.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, 0);
    }

    #[tokio::test]
    async fn create_with_admin_role() {
        let pool = test_pool().await;
        let req = CreateUserRequest {
            login: "admin1".into(),
            display_name: "Admin One".into(),
            email: "admin@example.com".into(),
            password: None,
            role: Some(Role::Admin),
        };
        let user = create(&pool, &req, None).await.unwrap();
        assert_eq!(user.role, Role::Admin);
    }
}
