#![allow(dead_code)]

//! FTS5 index maintenance for the `tickets_fts` virtual table.
//!
//! Provides functions to keep the full-text search index in sync with
//! ticket and comment data. All functions accept a generic executor so
//! they work with both a pool and a transaction.

use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Executor, Sqlite};

use super::RepoError;

/// Inserts a ticket into the FTS index with the given title and body text.
pub async fn index_ticket<'e, E>(
    exec: E,
    ticket_id: i64,
    title: &str,
    body: &str,
) -> Result<(), RepoError>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO tickets_fts(rowid, title, body) VALUES (?, ?, ?)")
        .bind(ticket_id)
        .bind(title)
        .bind(body)
        .execute(exec)
        .await?;
    Ok(())
}

/// Rebuilds the FTS entry for a single ticket by reading the current
/// title from the `tickets` table and concatenating all comment bodies.
pub async fn reindex_ticket<'e, E>(exec: E, ticket_id: i64) -> Result<(), RepoError>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "DELETE FROM tickets_fts WHERE rowid = ?; \
         INSERT INTO tickets_fts(rowid, title, body) \
         SELECT t.id, t.title, COALESCE(( \
             SELECT GROUP_CONCAT(c.body, ' ') FROM comments c WHERE c.ticket_id = t.id \
         ), '') \
         FROM tickets t WHERE t.id = ?",
    )
    .bind(ticket_id)
    .bind(ticket_id)
    .execute(exec)
    .await?;
    Ok(())
}

/// Removes a ticket from the FTS index.
pub async fn delete_ticket_index<'e, E>(exec: E, ticket_id: i64) -> Result<(), RepoError>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM tickets_fts WHERE rowid = ?")
        .bind(ticket_id)
        .execute(exec)
        .await?;
    Ok(())
}

/// Drops and rebuilds the entire FTS index from the tickets and comments tables.
///
/// Use for initial data load or when the index may have drifted.
pub async fn rebuild_all<'e, E>(exec: E) -> Result<SqliteQueryResult, RepoError>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "DELETE FROM tickets_fts; \
         INSERT INTO tickets_fts(rowid, title, body) \
         SELECT t.id, t.title, COALESCE(( \
             SELECT GROUP_CONCAT(c.body, ' ') FROM comments c WHERE c.ticket_id = t.id \
         ), '') \
         FROM tickets t; \
         INSERT INTO tickets_fts(tickets_fts) VALUES ('optimize')",
    )
    .execute(exec)
    .await?;
    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    /// Creates an in-memory SQLite database with the full schema + FTS table.
    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                login TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                email TEXT NOT NULL,
                password_hash TEXT,
                role TEXT NOT NULL DEFAULT 'user',
                oidc_sub TEXT UNIQUE,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE components (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                parent_id INTEGER REFERENCES components(id),
                path TEXT NOT NULL UNIQUE,
                owner_id INTEGER NOT NULL REFERENCES users(id),
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                UNIQUE(parent_id, name)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE tickets (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL CHECK (type IN ('bug', 'feature')),
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'new',
                priority TEXT NOT NULL DEFAULT 'P3',
                owner_id INTEGER NOT NULL REFERENCES users(id),
                component_id INTEGER NOT NULL REFERENCES components(id),
                estimation_hours REAL,
                created_by INTEGER NOT NULL REFERENCES users(id),
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE comments (
                id INTEGER PRIMARY KEY,
                ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
                number INTEGER NOT NULL,
                author_id INTEGER NOT NULL REFERENCES users(id),
                body TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                UNIQUE(ticket_id, number)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE VIRTUAL TABLE tickets_fts USING fts5(
                title,
                body,
                content='',
                contentless_delete=1,
                tokenize='porter unicode61 remove_diacritics 2',
                prefix='2,3'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Seed a user and component for FK constraints.
        sqlx::query(
            "INSERT INTO users(id, login, display_name, email) VALUES (1, 'test', 'Test', 'test@example.com')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO components(id, name, path, owner_id) VALUES (1, 'Root', '/Root/', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    /// Inserts a ticket row for testing.
    async fn insert_ticket(pool: &SqlitePool, id: i64, title: &str) {
        sqlx::query(
            "INSERT INTO tickets(id, type, title, owner_id, component_id, created_by) \
             VALUES (?, 'bug', ?, 1, 1, 1)",
        )
        .bind(id)
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Searches FTS and returns matching rowids.
    async fn fts_search(pool: &SqlitePool, match_expr: &str) -> Vec<i64> {
        sqlx::query_scalar::<_, i64>("SELECT rowid FROM tickets_fts WHERE tickets_fts MATCH ?")
            .bind(match_expr)
            .fetch_all(pool)
            .await
            .unwrap()
    }

    #[sqlx::test]
    async fn index_ticket_makes_searchable() {
        let pool = test_pool().await;
        insert_ticket(&pool, 1, "Login crash bug").await;

        super::index_ticket(&pool, 1, "Login crash bug", "App crashes on login page")
            .await
            .unwrap();

        let results = fts_search(&pool, "crash").await;
        assert_eq!(results, vec![1]);

        let results = fts_search(&pool, "login page").await;
        assert_eq!(results, vec![1]);
    }

    #[sqlx::test]
    async fn reindex_ticket_updates_content() {
        let pool = test_pool().await;
        insert_ticket(&pool, 1, "Old title").await;
        super::index_ticket(&pool, 1, "Old title", "old body")
            .await
            .unwrap();

        // Update the ticket title and add a comment.
        sqlx::query("UPDATE tickets SET title = 'New title' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO comments(ticket_id, number, author_id, body) VALUES (1, 0, 1, 'new comment body')",
        )
        .execute(&pool)
        .await
        .unwrap();

        super::reindex_ticket(&pool, 1).await.unwrap();

        // Old content no longer found.
        let results = fts_search(&pool, "\"Old title\"").await;
        assert!(results.is_empty());

        // New content searchable.
        let results = fts_search(&pool, "\"New title\"").await;
        assert_eq!(results, vec![1]);

        let results = fts_search(&pool, "\"new comment body\"").await;
        assert_eq!(results, vec![1]);
    }

    #[sqlx::test]
    async fn delete_ticket_index_removes_entry() {
        let pool = test_pool().await;
        insert_ticket(&pool, 1, "Deletable").await;
        super::index_ticket(&pool, 1, "Deletable", "some body")
            .await
            .unwrap();

        assert_eq!(fts_search(&pool, "Deletable").await.len(), 1);

        super::delete_ticket_index(&pool, 1).await.unwrap();

        assert!(fts_search(&pool, "Deletable").await.is_empty());
    }

    #[sqlx::test]
    async fn rebuild_all_recreates_index() {
        let pool = test_pool().await;

        // Insert tickets with comments but don't index them.
        insert_ticket(&pool, 1, "First ticket").await;
        insert_ticket(&pool, 2, "Second ticket").await;
        sqlx::query(
            "INSERT INTO comments(ticket_id, number, author_id, body) VALUES (1, 0, 1, 'first body')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO comments(ticket_id, number, author_id, body) VALUES (2, 0, 1, 'second body')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // No FTS results yet.
        assert!(fts_search(&pool, "ticket").await.is_empty());

        super::rebuild_all(&pool).await.unwrap();

        // Both tickets now searchable.
        let mut results = fts_search(&pool, "ticket").await;
        results.sort();
        assert_eq!(results, vec![1, 2]);

        // Comment bodies indexed too.
        assert_eq!(fts_search(&pool, "\"first body\"").await, vec![1]);
        assert_eq!(fts_search(&pool, "\"second body\"").await, vec![2]);
    }
}
