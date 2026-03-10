#![allow(dead_code)]

use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{
    CompactComponent, CompactMilestone, CompactUser, ComponentRow, CreateTicketRequest, CursorPage,
    MilestoneRow, TicketResponse, TicketRow, UpdateTicketRequest, UserRow, format_estimation,
    parse_estimation,
};

use super::RepoError;
use super::cursor::{decode_cursor, encode_cursor};

/// Parameters for listing tickets with cursor-based pagination.
pub struct ListTicketsParams {
    pub cursor: Option<String>,
    pub page_size: i64,
}

/// Returns a cursor-paginated page of tickets, ordered by `updated_at DESC, id DESC`.
pub async fn list(
    pool: &SqlitePool,
    params: &ListTicketsParams,
) -> Result<CursorPage<TicketRow>, RepoError> {
    let page_size = params.page_size.clamp(1, 200);
    let fetch_limit = page_size + 1;

    let mut rows = match &params.cursor {
        None => {
            sqlx::query_as::<_, TicketRow>(
                "SELECT * FROM tickets ORDER BY updated_at DESC, id DESC LIMIT ?",
            )
            .bind(fetch_limit)
            .fetch_all(pool)
            .await?
        }
        Some(cursor) => {
            let (cursor_ts, cursor_id) = decode_cursor(cursor)?;
            sqlx::query_as::<_, TicketRow>(
                "SELECT * FROM tickets
                 WHERE (updated_at, id) < (?, ?)
                 ORDER BY updated_at DESC, id DESC LIMIT ?",
            )
            .bind(cursor_ts)
            .bind(cursor_id)
            .bind(fetch_limit)
            .fetch_all(pool)
            .await?
        }
    };

    let has_more = rows.len() as i64 > page_size;
    if has_more {
        rows.truncate(page_size as usize);
    }

    let next_cursor = if has_more {
        rows.last().map(|r| encode_cursor(&r.updated_at, r.id))
    } else {
        None
    };

    Ok(CursorPage {
        items: rows,
        next_cursor,
        has_more,
    })
}

/// Finds a ticket by primary key.
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<TicketRow>, RepoError> {
    let row = sqlx::query_as::<_, TicketRow>("SELECT * FROM tickets WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Returns the total number of tickets.
pub async fn count(pool: &SqlitePool) -> Result<i64, RepoError> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tickets")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Creates a new ticket with optional CC users and milestones.
///
/// Uses a transaction to atomically insert the ticket row plus join table entries.
/// `created_by` comes from the session (separate from the request body).
pub async fn create(
    pool: &SqlitePool,
    req: &CreateTicketRequest,
    created_by: i64,
) -> Result<TicketRow, RepoError> {
    let now = Utc::now();
    let priority = req.priority.unwrap_or(crate::models::Priority::P3);

    let estimation_hours = match &req.estimation {
        Some(est) => Some(parse_estimation(est).map_err(RepoError::Conflict)?),
        None => None,
    };

    let mut tx = pool.begin().await?;

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, estimation_hours, created_by, created_at, updated_at)
         VALUES (?, ?, 'new', ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(req.ticket_type)
    .bind(&req.title)
    .bind(priority)
    .bind(req.owner_id)
    .bind(req.component_id)
    .bind(estimation_hours)
    .bind(created_by)
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    if let Some(cc) = &req.cc {
        for &user_id in cc {
            sqlx::query("INSERT INTO ticket_cc (ticket_id, user_id) VALUES (?, ?)")
                .bind(id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    if let Some(milestones) = &req.milestones {
        for &milestone_id in milestones {
            sqlx::query("INSERT INTO ticket_milestones (ticket_id, milestone_id) VALUES (?, ?)")
                .bind(id)
                .bind(milestone_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Applies a partial update to an existing ticket (read-merge-write).
///
/// Uses a transaction to atomically update the ticket row and replace CC/milestone
/// join table entries when provided.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    req: &UpdateTicketRequest,
) -> Result<TicketRow, RepoError> {
    let mut tx = pool.begin().await?;

    let existing = sqlx::query_as::<_, TicketRow>("SELECT * FROM tickets WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepoError::NotFound)?;

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let status = req.status.unwrap_or(existing.status);
    let priority = req.priority.unwrap_or(existing.priority);
    let owner_id = req.owner_id.unwrap_or(existing.owner_id);
    let component_id = req.component_id.unwrap_or(existing.component_id);
    let ticket_type = req.ticket_type.unwrap_or(existing.ticket_type);

    // Double-Option: None=keep, Some(None)=clear, Some(Some(s))=parse&set
    let estimation_hours = match &req.estimation {
        None => existing.estimation_hours,
        Some(None) => None,
        Some(Some(est)) => Some(parse_estimation(est).map_err(RepoError::Conflict)?),
    };

    let now = Utc::now();
    sqlx::query(
        "UPDATE tickets SET type = ?, title = ?, status = ?, priority = ?, owner_id = ?,
         component_id = ?, estimation_hours = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(ticket_type)
    .bind(title)
    .bind(status)
    .bind(priority)
    .bind(owner_id)
    .bind(component_id)
    .bind(estimation_hours)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if let Some(cc) = &req.cc {
        sqlx::query("DELETE FROM ticket_cc WHERE ticket_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for &user_id in cc {
            sqlx::query("INSERT INTO ticket_cc (ticket_id, user_id) VALUES (?, ?)")
                .bind(id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    if let Some(milestones) = &req.milestones {
        sqlx::query("DELETE FROM ticket_milestones WHERE ticket_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for &milestone_id in milestones {
            sqlx::query("INSERT INTO ticket_milestones (ticket_id, milestone_id) VALUES (?, ?)")
                .bind(id)
                .bind(milestone_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Deletes a ticket by ID. FK CASCADE handles cc/milestones/comments cleanup.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), RepoError> {
    let result = sqlx::query("DELETE FROM tickets WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RepoError::NotFound);
    }
    Ok(())
}

/// Returns all CC user IDs for a ticket.
pub async fn get_cc_user_ids(pool: &SqlitePool, ticket_id: i64) -> Result<Vec<i64>, RepoError> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT user_id FROM ticket_cc WHERE ticket_id = ? ORDER BY user_id")
            .bind(ticket_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Returns all milestone IDs for a ticket.
pub async fn get_milestone_ids(pool: &SqlitePool, ticket_id: i64) -> Result<Vec<i64>, RepoError> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT milestone_id FROM ticket_milestones WHERE ticket_id = ? ORDER BY milestone_id",
    )
    .bind(ticket_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Enriches a single ticket row into a full API response.
///
/// Loads owner, creator, component, CC users, milestones, and comment count.
pub async fn enrich(pool: &SqlitePool, row: &TicketRow) -> Result<TicketResponse, RepoError> {
    let results = enrich_many(pool, std::slice::from_ref(row)).await?;
    Ok(results.into_iter().next().unwrap())
}

/// Batch-enriches ticket rows into full API responses, avoiding N+1 queries.
///
/// Collects unique IDs across all tickets, bulk-loads related entities,
/// then maps them back to each ticket.
pub async fn enrich_many(
    pool: &SqlitePool,
    rows: &[TicketRow],
) -> Result<Vec<TicketResponse>, RepoError> {
    if rows.is_empty() {
        return Ok(vec![]);
    }

    // Collect unique user IDs (owners + creators)
    let mut user_ids: Vec<i64> = rows
        .iter()
        .flat_map(|r| [r.owner_id, r.created_by])
        .collect();

    // Collect CC user IDs for all tickets
    let ticket_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();

    let cc_rows: Vec<(i64, i64)> = if !ticket_ids.is_empty() {
        let placeholders = vec!["?"; ticket_ids.len()].join(",");
        let sql =
            format!("SELECT ticket_id, user_id FROM ticket_cc WHERE ticket_id IN ({placeholders})");
        let mut query = sqlx::query_as(&sql);
        for &tid in &ticket_ids {
            query = query.bind(tid);
        }
        query.fetch_all(pool).await?
    } else {
        vec![]
    };

    // Add CC user IDs to the set
    for (_, uid) in &cc_rows {
        user_ids.push(*uid);
    }

    // Deduplicate
    user_ids.sort_unstable();
    user_ids.dedup();

    // Bulk load users
    let users: Vec<UserRow> = if !user_ids.is_empty() {
        let placeholders = vec!["?"; user_ids.len()].join(",");
        let sql = format!("SELECT * FROM users WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as(&sql);
        for &uid in &user_ids {
            query = query.bind(uid);
        }
        query.fetch_all(pool).await?
    } else {
        vec![]
    };
    let user_map: std::collections::HashMap<i64, CompactUser> =
        users.iter().map(|u| (u.id, CompactUser::from(u))).collect();

    // Collect unique component IDs
    let mut component_ids: Vec<i64> = rows.iter().map(|r| r.component_id).collect();
    component_ids.sort_unstable();
    component_ids.dedup();

    let components: Vec<ComponentRow> = if !component_ids.is_empty() {
        let placeholders = vec!["?"; component_ids.len()].join(",");
        let sql = format!("SELECT * FROM components WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as(&sql);
        for &cid in &component_ids {
            query = query.bind(cid);
        }
        query.fetch_all(pool).await?
    } else {
        vec![]
    };
    let component_map: std::collections::HashMap<i64, CompactComponent> = components
        .iter()
        .map(|c| (c.id, CompactComponent::from(c)))
        .collect();

    // Load milestone associations
    let milestone_rows: Vec<(i64, i64)> = if !ticket_ids.is_empty() {
        let placeholders = vec!["?"; ticket_ids.len()].join(",");
        let sql = format!(
            "SELECT ticket_id, milestone_id FROM ticket_milestones WHERE ticket_id IN ({placeholders})"
        );
        let mut query = sqlx::query_as(&sql);
        for &tid in &ticket_ids {
            query = query.bind(tid);
        }
        query.fetch_all(pool).await?
    } else {
        vec![]
    };

    let mut milestone_ids: Vec<i64> = milestone_rows.iter().map(|(_, mid)| *mid).collect();
    milestone_ids.sort_unstable();
    milestone_ids.dedup();

    let milestones: Vec<MilestoneRow> = if !milestone_ids.is_empty() {
        let placeholders = vec!["?"; milestone_ids.len()].join(",");
        let sql = format!("SELECT * FROM milestones WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as(&sql);
        for &mid in &milestone_ids {
            query = query.bind(mid);
        }
        query.fetch_all(pool).await?
    } else {
        vec![]
    };
    let milestone_map: std::collections::HashMap<i64, CompactMilestone> = milestones
        .iter()
        .map(|m| (m.id, CompactMilestone::from(m)))
        .collect();

    // Load comment counts
    let comment_counts: Vec<(i64, i64)> = if !ticket_ids.is_empty() {
        let placeholders = vec!["?"; ticket_ids.len()].join(",");
        let sql = format!(
            "SELECT ticket_id, COUNT(*) FROM comments WHERE ticket_id IN ({placeholders}) GROUP BY ticket_id"
        );
        let mut query = sqlx::query_as(&sql);
        for &tid in &ticket_ids {
            query = query.bind(tid);
        }
        query.fetch_all(pool).await?
    } else {
        vec![]
    };
    let comment_count_map: std::collections::HashMap<i64, i64> =
        comment_counts.into_iter().collect();

    // Build CC lookup: ticket_id → Vec<CompactUser>
    let mut cc_map: std::collections::HashMap<i64, Vec<CompactUser>> =
        std::collections::HashMap::new();
    for (tid, uid) in &cc_rows {
        if let Some(user) = user_map.get(uid) {
            cc_map.entry(*tid).or_default().push(user.clone());
        }
    }

    // Build milestone lookup: ticket_id → Vec<CompactMilestone>
    let mut ms_map: std::collections::HashMap<i64, Vec<CompactMilestone>> =
        std::collections::HashMap::new();
    for (tid, mid) in &milestone_rows {
        if let Some(ms) = milestone_map.get(mid) {
            ms_map.entry(*tid).or_default().push(ms.clone());
        }
    }

    // Assemble responses
    let mut responses = Vec::with_capacity(rows.len());
    for row in rows {
        let owner = user_map
            .get(&row.owner_id)
            .cloned()
            .ok_or(RepoError::NotFound)?;
        let created_by = user_map
            .get(&row.created_by)
            .cloned()
            .ok_or(RepoError::NotFound)?;
        let component = component_map
            .get(&row.component_id)
            .cloned()
            .ok_or(RepoError::NotFound)?;

        let estimation_display = row.estimation_hours.map(format_estimation);

        responses.push(TicketResponse {
            id: row.id,
            slug: None,
            ticket_type: row.ticket_type,
            title: row.title.clone(),
            status: row.status,
            priority: row.priority,
            owner,
            component,
            estimation_hours: row.estimation_hours,
            estimation_display,
            created_by,
            cc: cc_map.remove(&row.id).unwrap_or_default(),
            milestones: ms_map.remove(&row.id).unwrap_or_default(),
            comment_count: comment_count_map.get(&row.id).copied().unwrap_or(0),
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
    use crate::models::{
        CreateComponentRequest, CreateUserRequest, Priority, TicketStatus, TicketType,
    };
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

    async fn seed_component(pool: &SqlitePool, name: &str, owner_id: i64) -> i64 {
        let req = CreateComponentRequest {
            name: name.to_string(),
            parent_id: None,
            slug: Some(name.to_uppercase()),
            owner_id,
        };
        component::create(pool, &req).await.unwrap().id
    }

    async fn seed_milestone(pool: &SqlitePool, name: &str) -> i64 {
        let now = Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO milestones (name, status, created_at, updated_at)
             VALUES (?, 'open', ?, ?) RETURNING id",
        )
        .bind(name)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn make_create_request(title: &str, owner_id: i64, component_id: i64) -> CreateTicketRequest {
        CreateTicketRequest {
            ticket_type: TicketType::Bug,
            title: title.to_string(),
            owner_id,
            component_id,
            priority: None,
            description: None,
            cc: None,
            milestones: None,
            estimation: None,
        }
    }

    // ── CRUD tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn create_and_get_by_id() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "alice").await;
        let comp_id = seed_component(&pool, "Backend", user_id).await;

        let req = CreateTicketRequest {
            ticket_type: TicketType::Feature,
            title: "Add login page".to_string(),
            owner_id: user_id,
            component_id: comp_id,
            priority: Some(Priority::P1),
            description: None,
            cc: None,
            milestones: None,
            estimation: Some("2d".to_string()),
        };
        let ticket = create(&pool, &req, user_id).await.unwrap();

        assert_eq!(ticket.title, "Add login page");
        assert_eq!(ticket.ticket_type, TicketType::Feature);
        assert_eq!(ticket.priority, Priority::P1);
        assert_eq!(ticket.owner_id, user_id);
        assert_eq!(ticket.component_id, comp_id);
        assert_eq!(ticket.created_by, user_id);
        assert_eq!(ticket.estimation_hours, Some(16.0));

        let fetched = get_by_id(&pool, ticket.id).await.unwrap().unwrap();
        assert_eq!(fetched.title, ticket.title);
    }

    #[tokio::test]
    async fn create_with_defaults() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "bob").await;
        let comp_id = seed_component(&pool, "Frontend", user_id).await;

        let ticket = create(
            &pool,
            &make_create_request("Default ticket", user_id, comp_id),
            user_id,
        )
        .await
        .unwrap();

        assert_eq!(ticket.status, TicketStatus::New);
        assert_eq!(ticket.priority, Priority::P3);
        assert!(ticket.estimation_hours.is_none());
    }

    #[tokio::test]
    async fn create_with_estimation() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "carol").await;
        let comp_id = seed_component(&pool, "API", user_id).await;

        let mut req = make_create_request("Estimated task", user_id, comp_id);
        req.estimation = Some("2d".to_string());

        let ticket = create(&pool, &req, user_id).await.unwrap();
        assert_eq!(ticket.estimation_hours, Some(16.0));
    }

    #[tokio::test]
    async fn create_with_cc_and_milestones() {
        let pool = test_pool().await;
        let user_a = seed_user(&pool, "owner1").await;
        let user_b = seed_user(&pool, "cc1").await;
        let user_c = seed_user(&pool, "cc2").await;
        let comp_id = seed_component(&pool, "Core", user_a).await;
        let ms1 = seed_milestone(&pool, "v1.0").await;
        let ms2 = seed_milestone(&pool, "v2.0").await;

        let mut req = make_create_request("With relations", user_a, comp_id);
        req.cc = Some(vec![user_b, user_c]);
        req.milestones = Some(vec![ms1, ms2]);

        let ticket = create(&pool, &req, user_a).await.unwrap();

        let cc = get_cc_user_ids(&pool, ticket.id).await.unwrap();
        assert_eq!(cc.len(), 2);
        assert!(cc.contains(&user_b));
        assert!(cc.contains(&user_c));

        let ms = get_milestone_ids(&pool, ticket.id).await.unwrap();
        assert_eq!(ms.len(), 2);
        assert!(ms.contains(&ms1));
        assert!(ms.contains(&ms2));
    }

    #[tokio::test]
    async fn get_by_id_not_found() {
        let pool = test_pool().await;
        assert!(get_by_id(&pool, 9999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_partial_fields() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "dave").await;
        let comp_id = seed_component(&pool, "Infra", user_id).await;
        let ticket = create(
            &pool,
            &make_create_request("Original title", user_id, comp_id),
            user_id,
        )
        .await
        .unwrap();

        let updated = update(
            &pool,
            ticket.id,
            &UpdateTicketRequest {
                title: Some("New title".to_string()),
                status: None,
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: None,
                milestones: None,
                estimation: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.title, "New title");
        assert_eq!(updated.priority, ticket.priority);
        assert_eq!(updated.owner_id, ticket.owner_id);
    }

    #[tokio::test]
    async fn update_status() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "eve").await;
        let comp_id = seed_component(&pool, "Platform", user_id).await;
        let ticket = create(
            &pool,
            &make_create_request("Status test", user_id, comp_id),
            user_id,
        )
        .await
        .unwrap();
        assert_eq!(ticket.status, TicketStatus::New);

        let updated = update(
            &pool,
            ticket.id,
            &UpdateTicketRequest {
                title: None,
                status: Some(TicketStatus::InProgress),
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: None,
                milestones: None,
                estimation: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.status, TicketStatus::InProgress);
    }

    #[tokio::test]
    async fn update_estimation_set() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "frank").await;
        let comp_id = seed_component(&pool, "DB", user_id).await;
        let ticket = create(
            &pool,
            &make_create_request("Est set", user_id, comp_id),
            user_id,
        )
        .await
        .unwrap();
        assert!(ticket.estimation_hours.is_none());

        let updated = update(
            &pool,
            ticket.id,
            &UpdateTicketRequest {
                title: None,
                status: None,
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: None,
                milestones: None,
                estimation: Some(Some("4h".to_string())),
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.estimation_hours, Some(4.0));
    }

    #[tokio::test]
    async fn update_estimation_clear() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "grace").await;
        let comp_id = seed_component(&pool, "Auth", user_id).await;
        let mut req = make_create_request("Est clear", user_id, comp_id);
        req.estimation = Some("1d".to_string());
        let ticket = create(&pool, &req, user_id).await.unwrap();
        assert_eq!(ticket.estimation_hours, Some(8.0));

        let updated = update(
            &pool,
            ticket.id,
            &UpdateTicketRequest {
                title: None,
                status: None,
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: None,
                milestones: None,
                estimation: Some(None),
            },
        )
        .await
        .unwrap();

        assert!(updated.estimation_hours.is_none());
    }

    #[tokio::test]
    async fn update_estimation_unchanged() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "heidi").await;
        let comp_id = seed_component(&pool, "Net", user_id).await;
        let mut req = make_create_request("Est keep", user_id, comp_id);
        req.estimation = Some("3h".to_string());
        let ticket = create(&pool, &req, user_id).await.unwrap();

        let updated = update(
            &pool,
            ticket.id,
            &UpdateTicketRequest {
                title: Some("Renamed".to_string()),
                status: None,
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: None,
                milestones: None,
                estimation: None, // absent = don't change
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.estimation_hours, Some(3.0));
    }

    #[tokio::test]
    async fn update_cc_replace() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner2").await;
        let a = seed_user(&pool, "uA").await;
        let b = seed_user(&pool, "uB").await;
        let c = seed_user(&pool, "uC").await;
        let comp_id = seed_component(&pool, "Comp1", owner).await;

        let mut req = make_create_request("CC test", owner, comp_id);
        req.cc = Some(vec![a, b]);
        let ticket = create(&pool, &req, owner).await.unwrap();

        let cc_before = get_cc_user_ids(&pool, ticket.id).await.unwrap();
        assert_eq!(cc_before.len(), 2);

        // Replace [A,B] with [B,C]
        update(
            &pool,
            ticket.id,
            &UpdateTicketRequest {
                title: None,
                status: None,
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: Some(vec![b, c]),
                milestones: None,
                estimation: None,
            },
        )
        .await
        .unwrap();

        let cc_after = get_cc_user_ids(&pool, ticket.id).await.unwrap();
        assert_eq!(cc_after.len(), 2);
        assert!(cc_after.contains(&b));
        assert!(cc_after.contains(&c));
        assert!(!cc_after.contains(&a));
    }

    #[tokio::test]
    async fn update_milestones_replace() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner3").await;
        let comp_id = seed_component(&pool, "Comp2", owner).await;
        let ms1 = seed_milestone(&pool, "Sprint 1").await;
        let ms2 = seed_milestone(&pool, "Sprint 2").await;
        let ms3 = seed_milestone(&pool, "Sprint 3").await;

        let mut req = make_create_request("MS test", owner, comp_id);
        req.milestones = Some(vec![ms1, ms2]);
        let ticket = create(&pool, &req, owner).await.unwrap();

        let ms_before = get_milestone_ids(&pool, ticket.id).await.unwrap();
        assert_eq!(ms_before.len(), 2);

        update(
            &pool,
            ticket.id,
            &UpdateTicketRequest {
                title: None,
                status: None,
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: None,
                milestones: Some(vec![ms2, ms3]),
                estimation: None,
            },
        )
        .await
        .unwrap();

        let ms_after = get_milestone_ids(&pool, ticket.id).await.unwrap();
        assert_eq!(ms_after.len(), 2);
        assert!(ms_after.contains(&ms2));
        assert!(ms_after.contains(&ms3));
        assert!(!ms_after.contains(&ms1));
    }

    #[tokio::test]
    async fn update_not_found() {
        let pool = test_pool().await;
        let result = update(
            &pool,
            9999,
            &UpdateTicketRequest {
                title: Some("Ghost".to_string()),
                status: None,
                priority: None,
                owner_id: None,
                component_id: None,
                ticket_type: None,
                cc: None,
                milestones: None,
                estimation: None,
            },
        )
        .await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn delete_ticket() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "ivan").await;
        let comp_id = seed_component(&pool, "Del", user_id).await;
        let ticket = create(
            &pool,
            &make_create_request("To delete", user_id, comp_id),
            user_id,
        )
        .await
        .unwrap();

        delete(&pool, ticket.id).await.unwrap();
        assert!(get_by_id(&pool, ticket.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_cascades_cc_and_milestones() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner4").await;
        let cc_user = seed_user(&pool, "cc3").await;
        let comp_id = seed_component(&pool, "Cascade", owner).await;
        let ms = seed_milestone(&pool, "Cascade MS").await;

        let mut req = make_create_request("Cascade test", owner, comp_id);
        req.cc = Some(vec![cc_user]);
        req.milestones = Some(vec![ms]);
        let ticket = create(&pool, &req, owner).await.unwrap();

        delete(&pool, ticket.id).await.unwrap();

        let cc = get_cc_user_ids(&pool, ticket.id).await.unwrap();
        assert!(cc.is_empty());
        let ms_ids = get_milestone_ids(&pool, ticket.id).await.unwrap();
        assert!(ms_ids.is_empty());
    }

    #[tokio::test]
    async fn delete_not_found() {
        let pool = test_pool().await;
        let result = delete(&pool, 9999).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn count_returns_correct_total() {
        let pool = test_pool().await;
        assert_eq!(count(&pool).await.unwrap(), 0);

        let user_id = seed_user(&pool, "counter").await;
        let comp_id = seed_component(&pool, "Count", user_id).await;
        create(&pool, &make_create_request("T1", user_id, comp_id), user_id)
            .await
            .unwrap();
        create(&pool, &make_create_request("T2", user_id, comp_id), user_id)
            .await
            .unwrap();

        assert_eq!(count(&pool).await.unwrap(), 2);
    }

    // ── Pagination tests ────────────────────────────────────────

    #[tokio::test]
    async fn list_first_page() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "pager").await;
        let comp_id = seed_component(&pool, "Page", user_id).await;

        for i in 0..3 {
            let mut req = make_create_request(&format!("Ticket {i}"), user_id, comp_id);
            req.estimation = None;
            create(&pool, &req, user_id).await.unwrap();
            // Small delay to ensure distinct updated_at values
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let page = list(
            &pool,
            &ListTicketsParams {
                cursor: None,
                page_size: 2,
            },
        )
        .await
        .unwrap();

        assert_eq!(page.items.len(), 2);
        assert!(page.has_more);
        assert!(page.next_cursor.is_some());
    }

    #[tokio::test]
    async fn list_second_page() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "pager2").await;
        let comp_id = seed_component(&pool, "Page2", user_id).await;

        for i in 0..3 {
            create(
                &pool,
                &make_create_request(&format!("T{i}"), user_id, comp_id),
                user_id,
            )
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let first = list(
            &pool,
            &ListTicketsParams {
                cursor: None,
                page_size: 2,
            },
        )
        .await
        .unwrap();

        let second = list(
            &pool,
            &ListTicketsParams {
                cursor: first.next_cursor,
                page_size: 2,
            },
        )
        .await
        .unwrap();

        assert_eq!(second.items.len(), 1);
        assert!(!second.has_more);
        assert!(second.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_empty() {
        let pool = test_pool().await;
        let page = list(
            &pool,
            &ListTicketsParams {
                cursor: None,
                page_size: 10,
            },
        )
        .await
        .unwrap();

        assert!(page.items.is_empty());
        assert!(!page.has_more);
        assert!(page.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_ordering() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "orderer").await;
        let comp_id = seed_component(&pool, "Order", user_id).await;

        for i in 0..3 {
            create(
                &pool,
                &make_create_request(&format!("Order{i}"), user_id, comp_id),
                user_id,
            )
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let page = list(
            &pool,
            &ListTicketsParams {
                cursor: None,
                page_size: 10,
            },
        )
        .await
        .unwrap();

        // Most recently updated first (DESC)
        for window in page.items.windows(2) {
            assert!(window[0].updated_at >= window[1].updated_at);
        }
    }

    // ── Enrich tests ────────────────────────────────────────────

    #[tokio::test]
    async fn enrich_single_ticket() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "enricher").await;
        let cc_user = seed_user(&pool, "cc_enrich").await;
        let comp_id = seed_component(&pool, "EnrichComp", owner).await;
        let ms = seed_milestone(&pool, "EnrichMS").await;

        let mut req = make_create_request("Enrich me", owner, comp_id);
        req.cc = Some(vec![cc_user]);
        req.milestones = Some(vec![ms]);
        let ticket = create(&pool, &req, owner).await.unwrap();

        let response = enrich(&pool, &ticket).await.unwrap();

        assert_eq!(response.id, ticket.id);
        assert_eq!(response.owner.id, owner);
        assert_eq!(response.created_by.id, owner);
        assert_eq!(response.component.id, comp_id);
        assert_eq!(response.cc.len(), 1);
        assert_eq!(response.cc[0].id, cc_user);
        assert_eq!(response.milestones.len(), 1);
        assert_eq!(response.milestones[0].id, ms);
        assert_eq!(response.comment_count, 0);
    }

    #[tokio::test]
    async fn enrich_comment_count() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "commenter").await;
        let comp_id = seed_component(&pool, "CmtComp", user_id).await;
        let ticket = create(
            &pool,
            &make_create_request("Comments", user_id, comp_id),
            user_id,
        )
        .await
        .unwrap();

        // Insert a comment via raw SQL
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO comments (ticket_id, number, author_id, body, created_at, updated_at)
             VALUES (?, 0, ?, 'Description', ?, ?)",
        )
        .bind(ticket.id)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let response = enrich(&pool, &ticket).await.unwrap();
        assert_eq!(response.comment_count, 1);
    }

    #[tokio::test]
    async fn enrich_estimation_display() {
        let pool = test_pool().await;
        let user_id = seed_user(&pool, "displayer").await;
        let comp_id = seed_component(&pool, "Display", user_id).await;
        let mut req = make_create_request("Est display", user_id, comp_id);
        req.estimation = Some("2d".to_string());
        let ticket = create(&pool, &req, user_id).await.unwrap();

        let response = enrich(&pool, &ticket).await.unwrap();
        assert_eq!(response.estimation_hours, Some(16.0));
        assert_eq!(response.estimation_display.as_deref(), Some("2d"));
    }
}
