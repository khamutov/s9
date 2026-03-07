pub mod comment;
pub mod component;
pub mod cursor;
pub mod ticket;
pub mod user;

use std::fmt;

/// Common error type for repository operations.
#[derive(Debug)]
pub enum RepoError {
    /// Requested entity does not exist.
    NotFound,
    /// Operation violates a uniqueness constraint (e.g. duplicate login).
    Conflict(String),
    /// Unclassified database error.
    Database(sqlx::Error),
}

impl fmt::Display for RepoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::Conflict(msg) => write!(f, "conflict: {msg}"),
            Self::Database(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for RepoError {}

impl From<sqlx::Error> for RepoError {
    fn from(err: sqlx::Error) -> Self {
        // SQLite UNIQUE constraint violation has error code 2067.
        if let sqlx::Error::Database(ref db_err) = err
            && db_err.code().as_deref() == Some("2067")
        {
            return Self::Conflict(db_err.message().to_string());
        }
        Self::Database(err)
    }
}
