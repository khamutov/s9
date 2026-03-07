use std::collections::HashMap;

use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{
    AttachmentResponse, AttachmentRow, CommentEditResponse, CommentEditRow, CommentResponse,
    CommentRow, CompactUser, CreateCommentRequest, EditCommentRequest, UserRow,
};

use super::RepoError;

/// Returns all comments for a ticket, ordered by `number ASC`.
pub async fn list_by_ticket(
    pool: &SqlitePool,
    ticket_id: i64,
) -> Result<Vec<CommentRow>, RepoError> {
    let rows = sqlx::query_as::<_, CommentRow>(
        "SELECT * FROM comments WHERE ticket_id = ? ORDER BY number ASC",
    )
    .bind(ticket_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Finds a comment by primary key.
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<CommentRow>, RepoError> {
    let row = sqlx::query_as::<_, CommentRow>("SELECT * FROM comments WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Finds a comment by ticket ID and comment number.
pub async fn get_by_ticket_and_number(
    pool: &SqlitePool,
    ticket_id: i64,
    number: i64,
) -> Result<Option<CommentRow>, RepoError> {
    let row = sqlx::query_as::<_, CommentRow>(
        "SELECT * FROM comments WHERE ticket_id = ? AND number = ?",
    )
    .bind(ticket_id)
    .bind(number)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Creates a new comment on a ticket.
///
/// Uses a transaction to atomically verify the ticket exists, auto-assign the next
/// `number`, insert the comment row, and link any attachment IDs.
pub async fn create(
    pool: &SqlitePool,
    ticket_id: i64,
    req: &CreateCommentRequest,
    author_id: i64,
) -> Result<CommentRow, RepoError> {
    let now = Utc::now();
    let mut tx = pool.begin().await?;

    // Verify ticket exists
    let ticket_exists: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM tickets WHERE id = ?")
            .bind(ticket_id)
            .fetch_optional(&mut *tx)
            .await?;
    if ticket_exists.is_none() {
        return Err(RepoError::NotFound);
    }

    // Auto-assign number
    let (next_number,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(number), -1) + 1 FROM comments WHERE ticket_id = ?",
    )
    .bind(ticket_id)
    .fetch_one(&mut *tx)
    .await?;

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO comments (ticket_id, number, author_id, body, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(ticket_id)
    .bind(next_number)
    .bind(author_id)
    .bind(&req.body)
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    // Link attachments
    if let Some(attachment_ids) = &req.attachment_ids {
        for &aid in attachment_ids {
            sqlx::query("INSERT INTO comment_attachments (comment_id, attachment_id) VALUES (?, ?)")
                .bind(id)
                .bind(aid)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Updates a comment body, saving the old body to `comment_edits`.
///
/// Uses a transaction to atomically record the edit history and apply the update.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    req: &EditCommentRequest,
) -> Result<CommentRow, RepoError> {
    let now = Utc::now();
    let mut tx = pool.begin().await?;

    let existing = sqlx::query_as::<_, CommentRow>("SELECT * FROM comments WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepoError::NotFound)?;

    // Save edit history
    sqlx::query("INSERT INTO comment_edits (comment_id, old_body, edited_at) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&existing.body)
        .bind(now)
        .execute(&mut *tx)
        .await?;

    // Update comment
    sqlx::query("UPDATE comments SET body = ?, updated_at = ? WHERE id = ?")
        .bind(&req.body)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Deletes a comment by ID. FK CASCADE handles edits and attachment links.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), RepoError> {
    let result = sqlx::query("DELETE FROM comments WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RepoError::NotFound);
    }
    Ok(())
}

/// Returns the edit history for a comment, ordered by `edited_at ASC`.
pub async fn get_edit_history(
    pool: &SqlitePool,
    comment_id: i64,
) -> Result<Vec<CommentEditRow>, RepoError> {
    let rows = sqlx::query_as::<_, CommentEditRow>(
        "SELECT * FROM comment_edits WHERE comment_id = ? ORDER BY edited_at ASC",
    )
    .bind(comment_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Returns the attachment IDs linked to a comment.
pub async fn get_attachment_ids(
    pool: &SqlitePool,
    comment_id: i64,
) -> Result<Vec<i64>, RepoError> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT attachment_id FROM comment_attachments WHERE comment_id = ? ORDER BY attachment_id",
    )
    .bind(comment_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Enriches a single comment row into a full API response.
pub async fn enrich(
    pool: &SqlitePool,
    row: &CommentRow,
    include_edits: bool,
) -> Result<CommentResponse, RepoError> {
    let results = enrich_many(pool, &[row.clone()], include_edits).await?;
    Ok(results.into_iter().next().unwrap())
}

/// Batch-enriches comment rows into full API responses, avoiding N+1 queries.
///
/// Loads authors, attachments, and edit counts (plus full edits if requested)
/// in bulk, then assembles each `CommentResponse`.
pub async fn enrich_many(
    pool: &SqlitePool,
    rows: &[CommentRow],
    include_edits: bool,
) -> Result<Vec<CommentResponse>, RepoError> {
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let comment_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();

    // Bulk load authors
    let mut author_ids: Vec<i64> = rows.iter().map(|r| r.author_id).collect();
    author_ids.sort_unstable();
    author_ids.dedup();

    let users: Vec<UserRow> = {
        let placeholders = vec!["?"; author_ids.len()].join(",");
        let sql = format!("SELECT * FROM users WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as(&sql);
        for &uid in &author_ids {
            query = query.bind(uid);
        }
        query.fetch_all(pool).await?
    };
    let user_map: HashMap<i64, CompactUser> =
        users.iter().map(|u| (u.id, CompactUser::from(u))).collect();

    // Bulk load attachment associations
    let attachment_links: Vec<(i64, i64)> = {
        let placeholders = vec!["?"; comment_ids.len()].join(",");
        let sql = format!(
            "SELECT comment_id, attachment_id FROM comment_attachments WHERE comment_id IN ({placeholders})"
        );
        let mut query = sqlx::query_as(&sql);
        for &cid in &comment_ids {
            query = query.bind(cid);
        }
        query.fetch_all(pool).await?
    };

    // Bulk load attachment rows
    let mut attachment_ids: Vec<i64> = attachment_links.iter().map(|(_, aid)| *aid).collect();
    attachment_ids.sort_unstable();
    attachment_ids.dedup();

    let attachment_map: HashMap<i64, AttachmentResponse> = if !attachment_ids.is_empty() {
        let placeholders = vec!["?"; attachment_ids.len()].join(",");
        let sql = format!("SELECT * FROM attachments WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as::<_, AttachmentRow>(&sql);
        for &aid in &attachment_ids {
            query = query.bind(aid);
        }
        let att_rows: Vec<AttachmentRow> = query.fetch_all(pool).await?;
        att_rows
            .iter()
            .map(|a| (a.id, AttachmentResponse::from(a)))
            .collect()
    } else {
        HashMap::new()
    };

    // Build comment_id → Vec<AttachmentResponse>
    let mut comment_attachments: HashMap<i64, Vec<AttachmentResponse>> = HashMap::new();
    for (cid, aid) in &attachment_links {
        if let Some(att) = attachment_map.get(aid) {
            comment_attachments.entry(*cid).or_default().push(att.clone());
        }
    }

    // Bulk load edit counts
    let edit_counts: Vec<(i64, i64)> = {
        let placeholders = vec!["?"; comment_ids.len()].join(",");
        let sql = format!(
            "SELECT comment_id, COUNT(*) FROM comment_edits WHERE comment_id IN ({placeholders}) GROUP BY comment_id"
        );
        let mut query = sqlx::query_as(&sql);
        for &cid in &comment_ids {
            query = query.bind(cid);
        }
        query.fetch_all(pool).await?
    };
    let edit_count_map: HashMap<i64, i64> = edit_counts.into_iter().collect();

    // Optionally bulk load full edit history
    let edit_map: HashMap<i64, Vec<CommentEditResponse>> = if include_edits {
        let placeholders = vec!["?"; comment_ids.len()].join(",");
        let sql = format!(
            "SELECT * FROM comment_edits WHERE comment_id IN ({placeholders}) ORDER BY edited_at ASC"
        );
        let mut query = sqlx::query_as::<_, CommentEditRow>(&sql);
        for &cid in &comment_ids {
            query = query.bind(cid);
        }
        let edit_rows: Vec<CommentEditRow> = query.fetch_all(pool).await?;
        let mut map: HashMap<i64, Vec<CommentEditResponse>> = HashMap::new();
        for e in &edit_rows {
            map.entry(e.comment_id).or_default().push(CommentEditResponse {
                old_body: e.old_body.clone(),
                edited_at: e.edited_at,
            });
        }
        map
    } else {
        HashMap::new()
    };

    // Assemble responses
    let mut responses = Vec::with_capacity(rows.len());
    for row in rows {
        let author = user_map
            .get(&row.author_id)
            .cloned()
            .ok_or(RepoError::NotFound)?;

        responses.push(CommentResponse {
            id: row.id,
            ticket_id: row.ticket_id,
            number: row.number,
            author,
            body: row.body.clone(),
            attachments: comment_attachments.remove(&row.id).unwrap_or_default(),
            edit_count: edit_count_map.get(&row.id).copied().unwrap_or(0),
            edits: edit_map.get(&row.id).cloned().unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }

    Ok(responses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::{CreateComponentRequest, CreateUserRequest};
    use crate::repos::{component, user};

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

    async fn seed_ticket(pool: &SqlitePool, owner_id: i64) -> i64 {
        let comp_req = CreateComponentRequest {
            name: "TestComp".to_string(),
            parent_id: None,
            owner_id,
        };
        let comp = component::create(pool, &comp_req).await.unwrap();

        let now = Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, created_by, created_at, updated_at)
             VALUES ('bug', 'Test ticket', 'new', 'P3', ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(owner_id)
        .bind(comp.id)
        .bind(owner_id)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn seed_attachment(pool: &SqlitePool, uploader_id: i64, name: &str) -> i64 {
        let now = Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO attachments (sha256, original_name, mime_type, size_bytes, uploader_id, created_at)
             VALUES ('deadbeef', ?, 'text/plain', 42, ?, ?) RETURNING id",
        )
        .bind(name)
        .bind(uploader_id)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    // ── CRUD tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn create_and_get_by_id() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "alice").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let req = CreateCommentRequest {
            body: "Hello world".to_string(),
            attachment_ids: None,
        };
        let comment = create(&pool, ticket_id, &req, user_id).await.unwrap();

        assert_eq!(comment.ticket_id, ticket_id);
        assert_eq!(comment.number, 0);
        assert_eq!(comment.author_id, user_id);
        assert_eq!(comment.body, "Hello world");

        let fetched = get_by_id(&pool, comment.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, comment.id);
        assert_eq!(fetched.body, comment.body);
    }

    #[tokio::test]
    async fn create_auto_increments_number() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "bob").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let c0 = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "first".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();
        let c1 = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "second".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();
        let c2 = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "third".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        assert_eq!(c0.number, 0);
        assert_eq!(c1.number, 1);
        assert_eq!(c2.number, 2);
    }

    #[tokio::test]
    async fn create_with_attachments() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "carol").await;
        let ticket_id = seed_ticket(&pool, user_id).await;
        let att1 = seed_attachment(&pool, user_id, "file1.txt").await;
        let att2 = seed_attachment(&pool, user_id, "file2.txt").await;

        let req = CreateCommentRequest {
            body: "With files".to_string(),
            attachment_ids: Some(vec![att1, att2]),
        };
        let comment = create(&pool, ticket_id, &req, user_id).await.unwrap();

        let ids = get_attachment_ids(&pool, comment.id).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&att1));
        assert!(ids.contains(&att2));
    }

    #[tokio::test]
    async fn create_ticket_not_found() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "dave").await;

        let req = CreateCommentRequest {
            body: "Orphan".to_string(),
            attachment_ids: None,
        };
        let result = create(&pool, 9999, &req, user_id).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn get_by_ticket_and_number_found_and_not_found() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "eve").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "desc".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        let found = get_by_ticket_and_number(&pool, ticket_id, 0).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().body, "desc");

        let missing = get_by_ticket_and_number(&pool, ticket_id, 99).await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn list_by_ticket_ordered() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "frank").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        for body in ["a", "b", "c"] {
            create(
                &pool,
                ticket_id,
                &CreateCommentRequest { body: body.into(), attachment_ids: None },
                user_id,
            ).await.unwrap();
        }

        let comments = list_by_ticket(&pool, ticket_id).await.unwrap();
        assert_eq!(comments.len(), 3);
        assert_eq!(comments[0].number, 0);
        assert_eq!(comments[1].number, 1);
        assert_eq!(comments[2].number, 2);
    }

    #[tokio::test]
    async fn list_by_ticket_empty() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "grace").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comments = list_by_ticket(&pool, ticket_id).await.unwrap();
        assert!(comments.is_empty());
    }

    #[tokio::test]
    async fn update_saves_edit_history() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "heidi").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "original".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        let updated = update(
            &pool,
            comment.id,
            &EditCommentRequest { body: "revised".into() },
        ).await.unwrap();

        assert_eq!(updated.body, "revised");

        let edits = get_edit_history(&pool, comment.id).await.unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].old_body, "original");
    }

    #[tokio::test]
    async fn update_multiple_edits() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "ivan").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "v1".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        update(&pool, comment.id, &EditCommentRequest { body: "v2".into() }).await.unwrap();
        update(&pool, comment.id, &EditCommentRequest { body: "v3".into() }).await.unwrap();

        let edits = get_edit_history(&pool, comment.id).await.unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].old_body, "v1");
        assert_eq!(edits[1].old_body, "v2");
    }

    #[tokio::test]
    async fn update_not_found() {
        let pool = test_pool().await;
        let result = update(&pool, 9999, &EditCommentRequest { body: "ghost".into() }).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn delete_comment() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "judy").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "bye".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        delete(&pool, comment.id).await.unwrap();
        assert!(get_by_id(&pool, comment.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_cascades_edits_and_attachments() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "karl").await;
        let ticket_id = seed_ticket(&pool, user_id).await;
        let att = seed_attachment(&pool, user_id, "cascade.txt").await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest {
                body: "will be deleted".into(),
                attachment_ids: Some(vec![att]),
            },
            user_id,
        ).await.unwrap();

        // Add an edit so we can verify cascade
        update(&pool, comment.id, &EditCommentRequest { body: "edited".into() }).await.unwrap();

        delete(&pool, comment.id).await.unwrap();

        let edits = get_edit_history(&pool, comment.id).await.unwrap();
        assert!(edits.is_empty());

        let att_ids = get_attachment_ids(&pool, comment.id).await.unwrap();
        assert!(att_ids.is_empty());
    }

    #[tokio::test]
    async fn delete_not_found() {
        let pool = test_pool().await;
        let result = delete(&pool, 9999).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    // ── Edit history tests ──────────────────────────────────────

    #[tokio::test]
    async fn get_edit_history_ordered() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "lisa").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "a".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        update(&pool, comment.id, &EditCommentRequest { body: "b".into() }).await.unwrap();
        update(&pool, comment.id, &EditCommentRequest { body: "c".into() }).await.unwrap();
        update(&pool, comment.id, &EditCommentRequest { body: "d".into() }).await.unwrap();

        let edits = get_edit_history(&pool, comment.id).await.unwrap();
        assert_eq!(edits.len(), 3);
        // Chronological order
        assert!(edits[0].edited_at <= edits[1].edited_at);
        assert!(edits[1].edited_at <= edits[2].edited_at);
        assert_eq!(edits[0].old_body, "a");
        assert_eq!(edits[1].old_body, "b");
        assert_eq!(edits[2].old_body, "c");
    }

    // ── Enrich tests ────────────────────────────────────────────

    #[tokio::test]
    async fn enrich_single_comment() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "mike").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "enrich me".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        // Add one edit
        update(&pool, comment.id, &EditCommentRequest { body: "enriched".into() }).await.unwrap();
        let updated = get_by_id(&pool, comment.id).await.unwrap().unwrap();

        let resp = enrich(&pool, &updated, false).await.unwrap();
        assert_eq!(resp.id, comment.id);
        assert_eq!(resp.author.id, user_id);
        assert_eq!(resp.author.login, "mike");
        assert_eq!(resp.edit_count, 1);
    }

    #[tokio::test]
    async fn enrich_with_attachments() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "nancy").await;
        let ticket_id = seed_ticket(&pool, user_id).await;
        let att = seed_attachment(&pool, user_id, "doc.pdf").await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest {
                body: "has file".into(),
                attachment_ids: Some(vec![att]),
            },
            user_id,
        ).await.unwrap();

        let resp = enrich(&pool, &comment, false).await.unwrap();
        assert_eq!(resp.attachments.len(), 1);
        assert_eq!(resp.attachments[0].original_name, "doc.pdf");
        assert!(resp.attachments[0].url.contains("doc.pdf"));
    }

    #[tokio::test]
    async fn enrich_with_edits_included() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "oscar").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "v1".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        update(&pool, comment.id, &EditCommentRequest { body: "v2".into() }).await.unwrap();
        update(&pool, comment.id, &EditCommentRequest { body: "v3".into() }).await.unwrap();
        let updated = get_by_id(&pool, comment.id).await.unwrap().unwrap();

        let resp = enrich(&pool, &updated, true).await.unwrap();
        assert_eq!(resp.edit_count, 2);
        assert_eq!(resp.edits.len(), 2);
        assert_eq!(resp.edits[0].old_body, "v1");
        assert_eq!(resp.edits[1].old_body, "v2");
    }

    #[tokio::test]
    async fn enrich_without_edits() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "petra").await;
        let ticket_id = seed_ticket(&pool, user_id).await;

        let comment = create(
            &pool,
            ticket_id,
            &CreateCommentRequest { body: "v1".into(), attachment_ids: None },
            user_id,
        ).await.unwrap();

        update(&pool, comment.id, &EditCommentRequest { body: "v2".into() }).await.unwrap();
        let updated = get_by_id(&pool, comment.id).await.unwrap().unwrap();

        let resp = enrich(&pool, &updated, false).await.unwrap();
        assert_eq!(resp.edit_count, 1);
        assert!(resp.edits.is_empty());
    }
}
