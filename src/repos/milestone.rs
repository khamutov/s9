use std::collections::HashMap;

use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{
    CreateMilestoneRequest, MilestoneResponse, MilestoneRow, MilestoneStats, MilestoneStatus,
    UpdateMilestoneRequest,
};

use super::RepoError;

/// Returns all milestones, optionally filtered by status.
///
/// Results are ordered by `due_date ASC NULLS LAST, name ASC`.
pub async fn list(
    pool: &SqlitePool,
    status: Option<MilestoneStatus>,
) -> Result<Vec<MilestoneRow>, RepoError> {
    let rows = if let Some(s) = status {
        sqlx::query_as::<_, MilestoneRow>(
            "SELECT * FROM milestones WHERE status = ? ORDER BY due_date IS NULL, due_date ASC, name ASC",
        )
        .bind(s)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, MilestoneRow>(
            "SELECT * FROM milestones ORDER BY due_date IS NULL, due_date ASC, name ASC",
        )
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

/// Finds a milestone by primary key.
pub async fn get_by_id(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<MilestoneRow>, RepoError> {
    let row = sqlx::query_as::<_, MilestoneRow>("SELECT * FROM milestones WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Returns the total number of milestones.
pub async fn count(pool: &SqlitePool) -> Result<i64, RepoError> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM milestones")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Creates a new milestone and returns the inserted row.
///
/// Name uniqueness is enforced by a DB UNIQUE constraint; duplicates
/// surface as `RepoError::Conflict`.
pub async fn create(
    pool: &SqlitePool,
    req: &CreateMilestoneRequest,
) -> Result<MilestoneRow, RepoError> {
    let now = Utc::now();
    let status = req.status.unwrap_or(MilestoneStatus::Open);

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO milestones (name, description, due_date, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(req.due_date)
    .bind(status)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Applies a partial update to an existing milestone (read-merge-write).
///
/// Double-Option fields (`description`, `due_date`) support null-clearing:
/// `None` = keep existing, `Some(None)` = set to NULL, `Some(Some(v))` = set value.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    req: &UpdateMilestoneRequest,
) -> Result<MilestoneRow, RepoError> {
    let existing = get_by_id(pool, id).await?.ok_or(RepoError::NotFound)?;

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let description = match &req.description {
        Some(v) => v.as_deref(),
        None => existing.description.as_deref(),
    };
    let due_date = match &req.due_date {
        Some(v) => *v,
        None => existing.due_date,
    };
    let status = req.status.unwrap_or(existing.status);
    let now = Utc::now();

    sqlx::query(
        "UPDATE milestones SET name = ?, description = ?, due_date = ?, status = ?, updated_at = ? WHERE id = ?",
    )
    .bind(name)
    .bind(description)
    .bind(due_date)
    .bind(status)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Deletes a milestone if it has no assigned tickets.
///
/// Returns `RepoError::NotFound` if the milestone doesn't exist, or
/// `RepoError::Conflict` if tickets are still linked via `ticket_milestones`.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), RepoError> {
    let existing = get_by_id(pool, id).await?.ok_or(RepoError::NotFound)?;

    let (ticket_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM ticket_milestones WHERE milestone_id = ?")
            .bind(existing.id)
            .fetch_one(pool)
            .await?;

    if ticket_count > 0 {
        return Err(RepoError::Conflict("has assigned tickets".to_string()));
    }

    sqlx::query("DELETE FROM milestones WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Computes aggregated ticket statistics for a single milestone.
pub async fn compute_stats(
    pool: &SqlitePool,
    milestone_id: i64,
) -> Result<MilestoneStats, RepoError> {
    let row: Option<(i64, i64, i64, i64, i64, f64, f64)> = sqlx::query_as(
        "SELECT
            COUNT(*) as total,
            SUM(CASE WHEN t.status = 'new' THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.status = 'in_progress' THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.status = 'verify' THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.status = 'done' THEN 1 ELSE 0 END),
            COALESCE(SUM(t.estimation_hours), 0.0),
            COALESCE(SUM(CASE WHEN t.status != 'done' THEN t.estimation_hours ELSE 0.0 END), 0.0)
         FROM ticket_milestones tm
         JOIN tickets t ON t.id = tm.ticket_id
         WHERE tm.milestone_id = ?",
    )
    .bind(milestone_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some((total, new, in_progress, verify, done, estimated_hours, remaining_hours)) => {
            Ok(MilestoneStats {
                total,
                new,
                in_progress,
                verify,
                done,
                estimated_hours,
                remaining_hours,
            })
        }
        None => Ok(MilestoneStats::default()),
    }
}

/// Batch-computes stats for multiple milestones, returning a map keyed by milestone ID.
async fn compute_stats_batch(
    pool: &SqlitePool,
    milestone_ids: &[i64],
) -> Result<HashMap<i64, MilestoneStats>, RepoError> {
    if milestone_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = vec!["?"; milestone_ids.len()].join(",");
    let sql = format!(
        "SELECT
            tm.milestone_id,
            COUNT(*),
            SUM(CASE WHEN t.status = 'new' THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.status = 'in_progress' THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.status = 'verify' THEN 1 ELSE 0 END),
            SUM(CASE WHEN t.status = 'done' THEN 1 ELSE 0 END),
            COALESCE(SUM(t.estimation_hours), 0.0),
            COALESCE(SUM(CASE WHEN t.status != 'done' THEN t.estimation_hours ELSE 0.0 END), 0.0)
         FROM ticket_milestones tm
         JOIN tickets t ON t.id = tm.ticket_id
         WHERE tm.milestone_id IN ({placeholders})
         GROUP BY tm.milestone_id"
    );

    let mut query = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, f64, f64)>(&sql);
    for &mid in milestone_ids {
        query = query.bind(mid);
    }
    let rows = query.fetch_all(pool).await?;

    let mut map = HashMap::new();
    for (mid, total, new, in_progress, verify, done, estimated_hours, remaining_hours) in rows {
        map.insert(
            mid,
            MilestoneStats {
                total,
                new,
                in_progress,
                verify,
                done,
                estimated_hours,
                remaining_hours,
            },
        );
    }
    Ok(map)
}

/// Enriches a single milestone row into a full API response with stats.
pub async fn enrich(
    pool: &SqlitePool,
    row: MilestoneRow,
) -> Result<MilestoneResponse, RepoError> {
    let results = enrich_many(pool, vec![row]).await?;
    Ok(results.into_iter().next().unwrap())
}

/// Batch-enriches milestone rows into full API responses, avoiding N+1 queries.
///
/// Loads stats for all milestones in a single grouped query, then assembles
/// each `MilestoneResponse`.
pub async fn enrich_many(
    pool: &SqlitePool,
    rows: Vec<MilestoneRow>,
) -> Result<Vec<MilestoneResponse>, RepoError> {
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let stats_map = compute_stats_batch(pool, &ids).await?;

    let responses = rows
        .into_iter()
        .map(|row| {
            let stats = stats_map
                .get(&row.id)
                .cloned()
                .unwrap_or_default();
            MilestoneResponse {
                id: row.id,
                name: row.name,
                description: row.description,
                due_date: row.due_date,
                status: row.status,
                stats,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        })
        .collect();

    Ok(responses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::{CreateComponentRequest, CreateUserRequest};
    use crate::repos::{component, user};
    use chrono::NaiveDate;

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

    async fn seed_component(pool: &SqlitePool, owner_id: i64) -> i64 {
        let req = CreateComponentRequest {
            name: "TestComp".to_string(),
            parent_id: None,
            slug: Some("TESTCOMP".to_string()),
            owner_id,
        };
        component::create(pool, &req).await.unwrap().id
    }

    /// Inserts a ticket with the given status and estimation, returning its ID.
    async fn seed_ticket(
        pool: &SqlitePool,
        owner_id: i64,
        component_id: i64,
        status: &str,
        estimation: Option<f64>,
    ) -> i64 {
        let now = Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, estimation_hours, created_by, created_at, updated_at)
             VALUES ('bug', 'Test ticket', ?, 'P3', ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(status)
        .bind(owner_id)
        .bind(component_id)
        .bind(estimation)
        .bind(owner_id)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// Links a ticket to a milestone via the junction table.
    async fn link_ticket_milestone(pool: &SqlitePool, ticket_id: i64, milestone_id: i64) {
        sqlx::query("INSERT INTO ticket_milestones (ticket_id, milestone_id) VALUES (?, ?)")
            .bind(ticket_id)
            .bind(milestone_id)
            .execute(pool)
            .await
            .unwrap();
    }

    fn make_create(name: &str) -> CreateMilestoneRequest {
        CreateMilestoneRequest {
            name: name.to_string(),
            description: None,
            due_date: None,
            status: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let pool = test_pool().await;
        let req = CreateMilestoneRequest {
            name: "v1.0".to_string(),
            description: Some("First release".to_string()),
            due_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()),
            status: None,
        };
        let row = create(&pool, &req).await.unwrap();

        assert_eq!(row.name, "v1.0");
        assert_eq!(row.description.as_deref(), Some("First release"));
        assert_eq!(row.due_date, Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()));
        assert_eq!(row.status, MilestoneStatus::Open);

        let fetched = get_by_id(&pool, row.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "v1.0");
    }

    #[tokio::test]
    async fn test_create_duplicate_name() {
        let pool = test_pool().await;
        create(&pool, &make_create("dup")).await.unwrap();
        let result = create(&pool, &make_create("dup")).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_list_all() {
        let pool = test_pool().await;
        create(&pool, &make_create("beta")).await.unwrap();
        create(&pool, &make_create("alpha")).await.unwrap();

        let all = list(&pool, None).await.unwrap();
        assert_eq!(all.len(), 2);
        // Both have no due_date so sorted by name ASC
        assert_eq!(all[0].name, "alpha");
        assert_eq!(all[1].name, "beta");
    }

    #[tokio::test]
    async fn test_list_filter_by_status() {
        let pool = test_pool().await;
        create(&pool, &make_create("open-ms")).await.unwrap();
        let closed = create(&pool, &make_create("closed-ms")).await.unwrap();
        update(
            &pool,
            closed.id,
            &UpdateMilestoneRequest {
                name: None,
                description: None,
                due_date: None,
                status: Some(MilestoneStatus::Closed),
            },
        )
        .await
        .unwrap();

        let open = list(&pool, Some(MilestoneStatus::Open)).await.unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].name, "open-ms");

        let closed_list = list(&pool, Some(MilestoneStatus::Closed)).await.unwrap();
        assert_eq!(closed_list.len(), 1);
        assert_eq!(closed_list[0].name, "closed-ms");
    }

    #[tokio::test]
    async fn test_update_fields() {
        let pool = test_pool().await;
        let ms = create(&pool, &make_create("orig")).await.unwrap();

        let updated = update(
            &pool,
            ms.id,
            &UpdateMilestoneRequest {
                name: Some("renamed".to_string()),
                description: None,
                due_date: None,
                status: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.name, "renamed");
        // Status unchanged
        assert_eq!(updated.status, MilestoneStatus::Open);
    }

    #[tokio::test]
    async fn test_update_clear_nullable_fields() {
        let pool = test_pool().await;
        let req = CreateMilestoneRequest {
            name: "with-desc".to_string(),
            description: Some("will be cleared".to_string()),
            due_date: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            status: None,
        };
        let ms = create(&pool, &req).await.unwrap();
        assert!(ms.description.is_some());
        assert!(ms.due_date.is_some());

        let updated = update(
            &pool,
            ms.id,
            &UpdateMilestoneRequest {
                name: None,
                description: Some(None),
                due_date: Some(None),
                status: None,
            },
        )
        .await
        .unwrap();

        assert!(updated.description.is_none());
        assert!(updated.due_date.is_none());
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = test_pool().await;
        let result = update(
            &pool,
            9999,
            &UpdateMilestoneRequest {
                name: Some("ghost".to_string()),
                description: None,
                due_date: None,
                status: None,
            },
        )
        .await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = test_pool().await;
        let ms = create(&pool, &make_create("doomed")).await.unwrap();
        delete(&pool, ms.id).await.unwrap();
        assert!(get_by_id(&pool, ms.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = test_pool().await;
        let result = delete(&pool, 9999).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_with_tickets() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "alice").await;
        let cid = seed_component(&pool, uid).await;
        let tid = seed_ticket(&pool, uid, cid, "new", None).await;
        let ms = create(&pool, &make_create("linked")).await.unwrap();
        link_ticket_milestone(&pool, tid, ms.id).await;

        let result = delete(&pool, ms.id).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_stats_computation() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "bob").await;
        let cid = seed_component(&pool, uid).await;
        let ms = create(&pool, &make_create("release")).await.unwrap();

        let t1 = seed_ticket(&pool, uid, cid, "new", Some(2.0)).await;
        let t2 = seed_ticket(&pool, uid, cid, "in_progress", Some(4.0)).await;
        let t3 = seed_ticket(&pool, uid, cid, "verify", Some(1.5)).await;
        let t4 = seed_ticket(&pool, uid, cid, "done", Some(3.0)).await;

        for tid in [t1, t2, t3, t4] {
            link_ticket_milestone(&pool, tid, ms.id).await;
        }

        let stats = compute_stats(&pool, ms.id).await.unwrap();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.new, 1);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.verify, 1);
        assert_eq!(stats.done, 1);
        assert!((stats.estimated_hours - 10.5).abs() < f64::EPSILON);
        assert!((stats.remaining_hours - 7.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_stats_empty_milestone() {
        let pool = test_pool().await;
        let ms = create(&pool, &make_create("empty")).await.unwrap();
        let stats = compute_stats(&pool, ms.id).await.unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.new, 0);
        assert_eq!(stats.in_progress, 0);
        assert_eq!(stats.verify, 0);
        assert_eq!(stats.done, 0);
        assert!((stats.estimated_hours).abs() < f64::EPSILON);
        assert!((stats.remaining_hours).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_enrich() {
        let pool = test_pool().await;
        let uid = seed_user(&pool, "carol").await;
        let cid = seed_component(&pool, uid).await;
        let ms = create(&pool, &make_create("enriched")).await.unwrap();

        let tid = seed_ticket(&pool, uid, cid, "done", Some(5.0)).await;
        link_ticket_milestone(&pool, tid, ms.id).await;

        let resp = enrich(&pool, ms).await.unwrap();
        assert_eq!(resp.name, "enriched");
        assert_eq!(resp.stats.total, 1);
        assert_eq!(resp.stats.done, 1);
        assert!((resp.stats.estimated_hours - 5.0).abs() < f64::EPSILON);
        assert!((resp.stats.remaining_hours).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_count() {
        let pool = test_pool().await;
        assert_eq!(count(&pool).await.unwrap(), 0);

        create(&pool, &make_create("a")).await.unwrap();
        create(&pool, &make_create("b")).await.unwrap();
        create(&pool, &make_create("c")).await.unwrap();
        assert_eq!(count(&pool).await.unwrap(), 3);
    }
}
