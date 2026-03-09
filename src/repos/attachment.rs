use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use crate::models::AttachmentRow;

use super::RepoError;

/// An orphaned attachment row pending cleanup.
#[derive(Debug)]
pub struct OrphanAttachment {
    pub id: i64,
    pub sha256: String,
}

// ── CRUD ───────────────────────────────────────────────────────

/// Returns an attachment by primary key, or `None` if not found.
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<AttachmentRow>, RepoError> {
    let row = sqlx::query_as::<_, AttachmentRow>("SELECT * FROM attachments WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Inserts a new attachment row and returns it.
pub async fn create(
    pool: &SqlitePool,
    sha256: &str,
    original_name: &str,
    mime_type: &str,
    size_bytes: i64,
    uploader_id: i64,
) -> Result<AttachmentRow, RepoError> {
    let now = Utc::now();
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO attachments (sha256, original_name, mime_type, size_bytes, uploader_id, created_at)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(sha256)
    .bind(original_name)
    .bind(mime_type)
    .bind(size_bytes)
    .bind(uploader_id)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Deletes an attachment row by ID. Returns `NotFound` if missing.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), RepoError> {
    let result = sqlx::query("DELETE FROM attachments WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RepoError::NotFound);
    }
    Ok(())
}

/// Returns attachment rows for the given IDs (batch fetch).
pub async fn get_by_ids(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<AttachmentRow>, RepoError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("SELECT * FROM attachments WHERE id IN ({placeholders})");
    let mut query = sqlx::query_as::<_, AttachmentRow>(&sql);
    for &id in ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(pool).await?;
    Ok(rows)
}

// ── Comment linking ────────────────────────────────────────────

/// Links an attachment to a comment via the junction table.
pub async fn link_to_comment(
    pool: &SqlitePool,
    comment_id: i64,
    attachment_id: i64,
) -> Result<(), RepoError> {
    sqlx::query("INSERT INTO comment_attachments (comment_id, attachment_id) VALUES (?, ?)")
        .bind(comment_id)
        .bind(attachment_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Removes the link between an attachment and a comment.
pub async fn unlink_from_comment(
    pool: &SqlitePool,
    comment_id: i64,
    attachment_id: i64,
) -> Result<(), RepoError> {
    sqlx::query("DELETE FROM comment_attachments WHERE comment_id = ? AND attachment_id = ?")
        .bind(comment_id)
        .bind(attachment_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Lists all attachments linked to a comment.
pub async fn list_by_comment(
    pool: &SqlitePool,
    comment_id: i64,
) -> Result<Vec<AttachmentRow>, RepoError> {
    let rows = sqlx::query_as::<_, AttachmentRow>(
        "SELECT a.* FROM attachments a
         JOIN comment_attachments ca ON ca.attachment_id = a.id
         WHERE ca.comment_id = ?
         ORDER BY a.id",
    )
    .bind(comment_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ── Orphan cleanup ─────────────────────────────────────────────

/// Returns `true` if any attachment row references the given SHA-256 digest.
pub async fn has_references(pool: &SqlitePool, sha256: &str) -> Result<bool, RepoError> {
    let (exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM attachments WHERE sha256 = ?)")
            .bind(sha256)
            .fetch_one(pool)
            .await?;
    Ok(exists)
}

/// Finds attachment rows that are not linked to any comment and older than `ttl`.
pub async fn find_orphans(
    pool: &SqlitePool,
    ttl: Duration,
) -> Result<Vec<OrphanAttachment>, RepoError> {
    let cutoff = Utc::now() - ttl;
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT a.id, a.sha256 FROM attachments a
         LEFT JOIN comment_attachments ca ON ca.attachment_id = a.id
         WHERE ca.comment_id IS NULL AND a.created_at < ?",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, sha256)| OrphanAttachment { id, sha256 })
        .collect())
}

/// Deletes orphan attachment rows by ID and returns the distinct SHA-256 digests
/// of the deleted rows.
pub async fn delete_orphan_rows(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<String>, RepoError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    // Collect distinct sha256s before deleting.
    let placeholders = vec!["?"; ids.len()].join(",");
    let select_sql =
        format!("SELECT DISTINCT sha256 FROM attachments WHERE id IN ({placeholders})");
    let mut select_query = sqlx::query_as::<_, (String,)>(&select_sql);
    for &id in ids {
        select_query = select_query.bind(id);
    }
    let sha_rows: Vec<(String,)> = select_query.fetch_all(pool).await?;
    let sha256s: Vec<String> = sha_rows.into_iter().map(|(s,)| s).collect();

    // Delete the rows.
    let delete_sql = format!("DELETE FROM attachments WHERE id IN ({placeholders})");
    let mut delete_query = sqlx::query(&delete_sql);
    for &id in ids {
        delete_query = delete_query.bind(id);
    }
    delete_query.execute(pool).await?;

    Ok(sha256s)
}

// ── Tests ──────────────────────────────────────────────────────

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

    async fn seed_user(pool: &SqlitePool, login: &str) -> i64 {
        let req = CreateUserRequest {
            login: login.to_string(),
            display_name: format!("User {login}"),
            email: format!("{login}@example.com"),
            password: None,
            role: None,
        };
        user::create(pool, &req, None).await.unwrap().id
    }

    /// Seed a comment on a ticket, returning (ticket_id, comment_id).
    async fn seed_ticket_and_comment(pool: &SqlitePool, user_id: i64) -> (i64, i64) {
        use crate::models::CreateComponentRequest;
        use crate::repos::component;

        let comp = component::create(
            pool,
            &CreateComponentRequest {
                name: "Comp".to_string(),
                parent_id: None,
                slug: Some("COMP".to_string()),
                owner_id: user_id,
            },
        )
        .await
        .unwrap();

        let now = Utc::now();
        let ticket_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, created_by, created_at, updated_at)
             VALUES ('bug', 'Test', 'new', 'P3', ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user_id)
        .bind(comp.id)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap();

        let comment_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO comments (ticket_id, number, author_id, body, created_at, updated_at)
             VALUES (?, 0, ?, 'test', ?, ?) RETURNING id",
        )
        .bind(ticket_id)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap();

        (ticket_id, comment_id)
    }

    // ── CRUD ───────────────────────────────────────────────────

    #[tokio::test]
    async fn create_and_get_by_id() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "alice").await;

        let att = create(&pool, "abc123", "file.txt", "text/plain", 42, uid)
            .await
            .unwrap();

        assert_eq!(att.sha256, "abc123");
        assert_eq!(att.original_name, "file.txt");
        assert_eq!(att.mime_type, "text/plain");
        assert_eq!(att.size_bytes, 42);
        assert_eq!(att.uploader_id, uid);

        let fetched = get_by_id(&pool, att.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, att.id);
        assert_eq!(fetched.sha256, "abc123");
    }

    #[tokio::test]
    async fn get_by_id_not_found() {
        let pool = test_pool().await;
        let result = get_by_id(&pool, 9999).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_by_ids_batch() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "bob").await;

        let a1 = create(&pool, "hash1", "f1.txt", "text/plain", 10, uid)
            .await
            .unwrap();
        let a2 = create(&pool, "hash2", "f2.txt", "text/plain", 20, uid)
            .await
            .unwrap();
        let _a3 = create(&pool, "hash3", "f3.txt", "text/plain", 30, uid)
            .await
            .unwrap();

        let batch = get_by_ids(&pool, &[a1.id, a2.id]).await.unwrap();
        assert_eq!(batch.len(), 2);

        let ids: Vec<i64> = batch.iter().map(|r| r.id).collect();
        assert!(ids.contains(&a1.id));
        assert!(ids.contains(&a2.id));
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "carol").await;

        let att = create(&pool, "del", "d.txt", "text/plain", 1, uid)
            .await
            .unwrap();
        delete(&pool, att.id).await.unwrap();

        assert!(get_by_id(&pool, att.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_not_found() {
        let pool = test_pool().await;
        let result = delete(&pool, 9999).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    // ── Comment linking ────────────────────────────────────────

    #[tokio::test]
    async fn link_and_list_by_comment() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "dave").await;
        let (_, comment_id) = seed_ticket_and_comment(&pool, uid).await;

        let att = create(&pool, "linked", "l.txt", "text/plain", 5, uid)
            .await
            .unwrap();
        link_to_comment(&pool, comment_id, att.id).await.unwrap();

        let list = list_by_comment(&pool, comment_id).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, att.id);
    }

    #[tokio::test]
    async fn link_duplicate_conflict() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "eve").await;
        let (_, comment_id) = seed_ticket_and_comment(&pool, uid).await;

        let att = create(&pool, "dup", "d.txt", "text/plain", 1, uid)
            .await
            .unwrap();
        link_to_comment(&pool, comment_id, att.id).await.unwrap();

        let result = link_to_comment(&pool, comment_id, att.id).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn unlink_from_comment_test() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "frank").await;
        let (_, comment_id) = seed_ticket_and_comment(&pool, uid).await;

        let att = create(&pool, "unlink", "u.txt", "text/plain", 1, uid)
            .await
            .unwrap();
        link_to_comment(&pool, comment_id, att.id).await.unwrap();
        unlink_from_comment(&pool, comment_id, att.id)
            .await
            .unwrap();

        let list = list_by_comment(&pool, comment_id).await.unwrap();
        assert!(list.is_empty());
    }

    // ── Orphan cleanup ─────────────────────────────────────────

    #[tokio::test]
    async fn has_references_true_and_false() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "grace").await;

        let att = create(&pool, "refcheck", "r.txt", "text/plain", 1, uid)
            .await
            .unwrap();
        assert!(has_references(&pool, "refcheck").await.unwrap());

        delete(&pool, att.id).await.unwrap();
        assert!(!has_references(&pool, "refcheck").await.unwrap());
    }

    #[tokio::test]
    async fn find_orphans_returns_unlinked() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "heidi").await;

        // Insert an old unlinked attachment (backdate created_at).
        let old_time = Utc::now() - Duration::hours(48);
        sqlx::query(
            "INSERT INTO attachments (sha256, original_name, mime_type, size_bytes, uploader_id, created_at)
             VALUES ('orphan_sha', 'orphan.txt', 'text/plain', 1, ?, ?)",
        )
        .bind(uid)
        .bind(old_time)
        .execute(&pool)
        .await
        .unwrap();

        let orphans = find_orphans(&pool, Duration::hours(24)).await.unwrap();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].sha256, "orphan_sha");
    }

    #[tokio::test]
    async fn find_orphans_skips_linked() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "ivan").await;
        let (_, comment_id) = seed_ticket_and_comment(&pool, uid).await;

        // Insert old attachment and link it.
        let old_time = Utc::now() - Duration::hours(48);
        let att_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO attachments (sha256, original_name, mime_type, size_bytes, uploader_id, created_at)
             VALUES ('linked_sha', 'linked.txt', 'text/plain', 1, ?, ?) RETURNING id",
        )
        .bind(uid)
        .bind(old_time)
        .fetch_one(&pool)
        .await
        .unwrap();

        link_to_comment(&pool, comment_id, att_id).await.unwrap();

        let orphans = find_orphans(&pool, Duration::hours(24)).await.unwrap();
        assert!(orphans.is_empty());
    }

    #[tokio::test]
    async fn find_orphans_skips_recent() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "judy").await;

        // Recent unlinked attachment (just created).
        create(&pool, "recent_sha", "recent.txt", "text/plain", 1, uid)
            .await
            .unwrap();

        let orphans = find_orphans(&pool, Duration::hours(24)).await.unwrap();
        assert!(orphans.is_empty());
    }

    #[tokio::test]
    async fn delete_orphan_rows_returns_sha256s() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "karl").await;

        let a1 = create(&pool, "sha_a", "a.txt", "text/plain", 1, uid)
            .await
            .unwrap();
        let a2 = create(&pool, "sha_b", "b.txt", "text/plain", 1, uid)
            .await
            .unwrap();

        let shas = delete_orphan_rows(&pool, &[a1.id, a2.id]).await.unwrap();
        assert_eq!(shas.len(), 2);
        assert!(shas.contains(&"sha_a".to_string()));
        assert!(shas.contains(&"sha_b".to_string()));

        // Rows should be gone.
        assert!(get_by_id(&pool, a1.id).await.unwrap().is_none());
        assert!(get_by_id(&pool, a2.id).await.unwrap().is_none());
    }

    // ── cleanup_orphans integration ────────────────────────────

    #[tokio::test]
    async fn cleanup_deletes_orphan_row_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let pool = test_pool().await;
        let uid = seed_user(&pool, "lisa").await;

        // Store a file on disk.
        crate::storage::init_dirs(dir.path()).await.unwrap();
        let store_result =
            crate::storage::store_file(dir.path(), b"orphan data", "orphan.txt", 1024)
                .await
                .unwrap();

        // Create a DB row with backdated timestamp (old enough to be orphaned).
        let old_time = Utc::now() - Duration::hours(48);
        sqlx::query(
            "INSERT INTO attachments (sha256, original_name, mime_type, size_bytes, uploader_id, created_at)
             VALUES (?, 'orphan.txt', 'text/plain', ?, ?, ?)",
        )
        .bind(&store_result.sha256)
        .bind(store_result.size_bytes as i64)
        .bind(uid)
        .bind(old_time)
        .execute(&pool)
        .await
        .unwrap();

        assert!(crate::storage::file_exists(dir.path(), &store_result.sha256).await);

        let removed = crate::storage::cleanup_orphans(&pool, dir.path(), Duration::hours(24))
            .await
            .unwrap();
        assert_eq!(removed, 1);

        // Row and file should both be gone.
        assert!(!has_references(&pool, &store_result.sha256).await.unwrap());
        assert!(!crate::storage::file_exists(dir.path(), &store_result.sha256).await);
    }

    #[tokio::test]
    async fn cleanup_preserves_referenced_file() {
        let dir = tempfile::tempdir().unwrap();
        let pool = test_pool().await;
        let uid = seed_user(&pool, "mike").await;

        crate::storage::init_dirs(dir.path()).await.unwrap();
        let store_result =
            crate::storage::store_file(dir.path(), b"shared data", "shared.txt", 1024)
                .await
                .unwrap();

        let old_time = Utc::now() - Duration::hours(48);

        // Row 1: orphan (old, unlinked).
        sqlx::query(
            "INSERT INTO attachments (sha256, original_name, mime_type, size_bytes, uploader_id, created_at)
             VALUES (?, 'shared.txt', 'text/plain', ?, ?, ?)",
        )
        .bind(&store_result.sha256)
        .bind(store_result.size_bytes as i64)
        .bind(uid)
        .bind(old_time)
        .execute(&pool)
        .await
        .unwrap();

        // Row 2: same SHA, but linked to a comment (keeps file alive).
        let (_, comment_id) = seed_ticket_and_comment(&pool, uid).await;
        let att2 = create(
            &pool,
            &store_result.sha256,
            "shared.txt",
            "text/plain",
            store_result.size_bytes as i64,
            uid,
        )
        .await
        .unwrap();
        link_to_comment(&pool, comment_id, att2.id).await.unwrap();

        let removed = crate::storage::cleanup_orphans(&pool, dir.path(), Duration::hours(24))
            .await
            .unwrap();
        assert_eq!(removed, 1);

        // File should still exist because row 2 references the same SHA.
        assert!(crate::storage::file_exists(dir.path(), &store_result.sha256).await);
    }
}
