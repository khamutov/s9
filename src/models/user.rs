use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::enums::Role;

/// Database row for the `users` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub login: String,
    pub display_name: String,
    pub email: String,
    pub password_hash: Option<String>,
    pub role: Role,
    pub oidc_sub: Option<String>,
    pub is_active: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Compact user reference embedded in other API responses.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompactUser {
    pub id: i64,
    pub login: String,
    pub display_name: String,
}

/// Full user details for admin-facing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FullUser {
    pub id: i64,
    pub login: String,
    pub display_name: String,
    pub email: String,
    pub role: Role,
    pub is_active: bool,
    pub has_password: bool,
    pub has_oidc: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&UserRow> for CompactUser {
    fn from(row: &UserRow) -> Self {
        Self {
            id: row.id,
            login: row.login.clone(),
            display_name: row.display_name.clone(),
        }
    }
}

impl From<&UserRow> for FullUser {
    fn from(row: &UserRow) -> Self {
        Self {
            id: row.id,
            login: row.login.clone(),
            display_name: row.display_name.clone(),
            email: row.email.clone(),
            role: row.role,
            is_active: row.is_active != 0,
            has_password: row.password_hash.is_some(),
            has_oidc: row.oidc_sub.is_some(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Request body for creating a new user.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub login: String,
    pub display_name: String,
    pub email: String,
    pub password: Option<String>,
    pub role: Option<Role>,
}

/// Request body for updating an existing user.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<Role>,
    pub is_active: Option<bool>,
}

/// Request body for setting/changing a user's password.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPasswordRequest {
    pub current_password: Option<String>,
    pub new_password: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_user_row() -> UserRow {
        UserRow {
            id: 42,
            login: "jdoe".into(),
            display_name: "Jane Doe".into(),
            email: "jane@example.com".into(),
            password_hash: Some("hashed".into()),
            role: Role::Admin,
            oidc_sub: Some("oidc-123".into()),
            is_active: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn compact_user_from_row() {
        let row = sample_user_row();
        let compact = CompactUser::from(&row);

        assert_eq!(compact.id, 42);
        assert_eq!(compact.login, "jdoe");
        assert_eq!(compact.display_name, "Jane Doe");
    }

    #[test]
    fn full_user_from_row_active_with_password_and_oidc() {
        let row = sample_user_row();
        let full = FullUser::from(&row);

        assert_eq!(full.id, 42);
        assert_eq!(full.role, Role::Admin);
        assert!(full.is_active);
        assert!(full.has_password);
        assert!(full.has_oidc);
    }

    #[test]
    fn full_user_from_row_inactive_no_password_no_oidc() {
        let mut row = sample_user_row();
        row.is_active = 0;
        row.password_hash = None;
        row.oidc_sub = None;

        let full = FullUser::from(&row);

        assert!(!full.is_active);
        assert!(!full.has_password);
        assert!(!full.has_oidc);
    }
}
