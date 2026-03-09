//! Notification event producer per DD 0.6 §7.
//!
//! Resolves recipients for each event type, checks mutes and active status,
//! then inserts rows into `pending_notifications` for batched email delivery.

use std::collections::HashSet;

use chrono::Utc;
use sqlx::SqlitePool;

/// Notification event types per DD 0.6 §7.2 / PRD §7.1.
pub enum NotifEvent {
    TicketCreated,
    CommentAdded,
    StatusChanged,
    PriorityChanged,
    OwnerChanged,
    MilestoneAdded,
}

impl NotifEvent {
    /// Returns the event type string stored in `pending_notifications.event_type`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TicketCreated => "ticket_created",
            Self::CommentAdded => "comment_added",
            Self::StatusChanged => "status_changed",
            Self::PriorityChanged => "priority_changed",
            Self::OwnerChanged => "owner_changed",
            Self::MilestoneAdded => "milestone_added",
        }
    }
}

/// Produces notification events by inserting rows into `pending_notifications`.
///
/// When SMTP is disabled (`smtp_enabled = false`), all methods are no-ops —
/// no rows are inserted (DD 0.6 §15).
#[derive(Clone)]
pub struct NotificationProducer {
    pool: SqlitePool,
    delay_seconds: i64,
    smtp_enabled: bool,
}

impl NotificationProducer {
    /// Creates a new producer.
    ///
    /// - `smtp_enabled`: when false, `emit` is a no-op.
    /// - `delay_seconds`: the batching window (default 120s per DD 0.6 §5.1).
    pub fn new(pool: SqlitePool, delay_seconds: i64, smtp_enabled: bool) -> Self {
        Self {
            pool,
            delay_seconds,
            smtp_enabled,
        }
    }

    /// Record a notification event. Resolves recipients, applies mute/active
    /// filters, and inserts one `pending_notifications` row per recipient.
    ///
    /// - `ticket_id`: the ticket this event relates to.
    /// - `event`: the event type.
    /// - `actor_id`: the user who performed the action (excluded from recipients).
    /// - `payload`: JSON metadata for the email template (DD 0.6 §14.2).
    pub async fn emit(
        &self,
        ticket_id: i64,
        event: NotifEvent,
        actor_id: i64,
        payload: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        self.emit_with_mentions(ticket_id, event, actor_id, payload, &[])
            .await
    }

    /// Like [`emit`], but adds extra recipient user IDs (e.g. from @mentions).
    pub async fn emit_with_mentions(
        &self,
        ticket_id: i64,
        event: NotifEvent,
        actor_id: i64,
        payload: serde_json::Value,
        extra_recipient_ids: &[i64],
    ) -> Result<(), sqlx::Error> {
        if !self.smtp_enabled {
            return Ok(());
        }

        let mut raw_recipients = self
            .resolve_raw_recipients(ticket_id, &event, actor_id)
            .await?;
        raw_recipients.extend_from_slice(extra_recipient_ids);

        // Self-exclusion + dedup.
        let mut recipients: HashSet<i64> = raw_recipients.into_iter().collect();
        recipients.remove(&actor_id);

        if recipients.is_empty() {
            return Ok(());
        }

        // Mute check: remove users who muted this ticket.
        let muted = self.get_muted_users(ticket_id).await?;
        for uid in &muted {
            recipients.remove(uid);
        }

        if recipients.is_empty() {
            return Ok(());
        }

        // Active user check: remove inactive users.
        let inactive = self.get_inactive_users(&recipients).await?;
        for uid in &inactive {
            recipients.remove(uid);
        }

        if recipients.is_empty() {
            return Ok(());
        }

        // Insert one row per recipient.
        let now = Utc::now();
        let send_after = now + chrono::Duration::seconds(self.delay_seconds);
        let event_type = event.as_str();
        let payload_str = serde_json::to_string(&payload).unwrap_or_default();

        for user_id in recipients {
            sqlx::query(
                "INSERT INTO pending_notifications (user_id, ticket_id, event_type, payload, created_at, send_after)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(user_id)
            .bind(ticket_id)
            .bind(event_type)
            .bind(&payload_str)
            .bind(now)
            .bind(send_after)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Resolves raw recipients per DD 0.6 §8.1 mapping table.
    async fn resolve_raw_recipients(
        &self,
        ticket_id: i64,
        event: &NotifEvent,
        _actor_id: i64,
    ) -> Result<Vec<i64>, sqlx::Error> {
        match event {
            NotifEvent::TicketCreated => {
                // Component owner.
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT c.owner_id FROM tickets t
                     JOIN components c ON c.id = t.component_id
                     WHERE t.id = ?",
                )
                .bind(ticket_id)
                .fetch_optional(&self.pool)
                .await?;
                Ok(row.map(|(id,)| vec![id]).unwrap_or_default())
            }

            NotifEvent::CommentAdded => {
                // Ticket owner + CC list. @mention parsing is task 4.5.
                let mut recipients = self.get_ticket_owner_and_cc(ticket_id).await?;
                // Deduplicated later.
                let _ = &mut recipients;
                Ok(recipients)
            }

            NotifEvent::StatusChanged | NotifEvent::PriorityChanged => {
                // Ticket owner + CC list.
                self.get_ticket_owner_and_cc(ticket_id).await
            }

            NotifEvent::OwnerChanged => {
                // Old owner, new owner, CC list.
                // Old/new owner are passed via payload and resolved by the caller.
                // Here we return ticket owner + CC; the caller adds old/new owner
                // to the payload before calling emit. The old owner is the ticket's
                // current owner_id (before update), which the handler queries.
                // For simplicity, we resolve the ticket's current state here.
                self.get_ticket_owner_and_cc(ticket_id).await
            }

            NotifEvent::MilestoneAdded => {
                // Ticket owner only.
                let row: Option<(i64,)> =
                    sqlx::query_as("SELECT owner_id FROM tickets WHERE id = ?")
                        .bind(ticket_id)
                        .fetch_optional(&self.pool)
                        .await?;
                Ok(row.map(|(id,)| vec![id]).unwrap_or_default())
            }
        }
    }

    /// Returns the ticket owner + all CC users.
    async fn get_ticket_owner_and_cc(&self, ticket_id: i64) -> Result<Vec<i64>, sqlx::Error> {
        let mut result = Vec::new();

        // Ticket owner.
        let owner: Option<(i64,)> = sqlx::query_as("SELECT owner_id FROM tickets WHERE id = ?")
            .bind(ticket_id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some((owner_id,)) = owner {
            result.push(owner_id);
        }

        // CC list.
        let cc_rows: Vec<(i64,)> =
            sqlx::query_as("SELECT user_id FROM ticket_cc WHERE ticket_id = ?")
                .bind(ticket_id)
                .fetch_all(&self.pool)
                .await?;
        for (uid,) in cc_rows {
            result.push(uid);
        }

        Ok(result)
    }

    /// Returns user IDs that have muted this ticket.
    async fn get_muted_users(&self, ticket_id: i64) -> Result<HashSet<i64>, sqlx::Error> {
        let rows: Vec<(i64,)> =
            sqlx::query_as("SELECT user_id FROM notification_mutes WHERE ticket_id = ?")
                .bind(ticket_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Returns inactive user IDs from the given set.
    async fn get_inactive_users(
        &self,
        user_ids: &HashSet<i64>,
    ) -> Result<HashSet<i64>, sqlx::Error> {
        if user_ids.is_empty() {
            return Ok(HashSet::new());
        }

        let placeholders = vec!["?"; user_ids.len()].join(",");
        let sql = format!("SELECT id FROM users WHERE id IN ({placeholders}) AND is_active = 0");
        let mut query = sqlx::query_as::<_, (i64,)>(&sql);
        for &uid in user_ids {
            query = query.bind(uid);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::{CreateComponentRequest, CreateUserRequest, Role};
    use crate::repos::{component, user};

    async fn test_pool() -> SqlitePool {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();
        pool
    }

    async fn seed_user(pool: &SqlitePool, login: &str, role: Option<Role>) -> i64 {
        let req = CreateUserRequest {
            login: login.to_string(),
            display_name: format!("User {login}"),
            email: format!("{login}@example.com"),
            password: None,
            role,
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

    async fn seed_ticket(
        pool: &SqlitePool,
        owner_id: i64,
        component_id: i64,
        created_by: i64,
    ) -> i64 {
        let now = Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, created_by, created_at, updated_at)
             VALUES ('bug', 'Test ticket', 'new', 'P3', ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(owner_id)
        .bind(component_id)
        .bind(created_by)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn add_cc(pool: &SqlitePool, ticket_id: i64, user_id: i64) {
        sqlx::query("INSERT INTO ticket_cc (ticket_id, user_id) VALUES (?, ?)")
            .bind(ticket_id)
            .bind(user_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn mute_ticket(pool: &SqlitePool, user_id: i64, ticket_id: i64) {
        sqlx::query("INSERT INTO notification_mutes (user_id, ticket_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(ticket_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn deactivate_user(pool: &SqlitePool, user_id: i64) {
        sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn count_pending(pool: &SqlitePool) -> i64 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pending_notifications")
            .fetch_one(pool)
            .await
            .unwrap();
        count
    }

    async fn get_pending_rows(pool: &SqlitePool) -> Vec<crate::models::PendingNotificationRow> {
        sqlx::query_as("SELECT * FROM pending_notifications ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn noop_when_smtp_disabled() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, owner).await;

        let producer = NotificationProducer::new(pool.clone(), 120, false);
        producer
            .emit(
                ticket_id,
                NotifEvent::TicketCreated,
                owner,
                serde_json::json!({"actor": "owner"}),
            )
            .await
            .unwrap();

        assert_eq!(count_pending(&pool).await, 0);
    }

    #[tokio::test]
    async fn ticket_created_notifies_component_owner() {
        let pool = test_pool().await;
        let creator = seed_user(&pool, "creator", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, creator, comp_id, creator).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(
                ticket_id,
                NotifEvent::TicketCreated,
                creator,
                serde_json::json!({"actor": "creator"}),
            )
            .await
            .unwrap();

        let rows = get_pending_rows(&pool).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].user_id, comp_owner);
        assert_eq!(rows[0].ticket_id, ticket_id);
        assert_eq!(rows[0].event_type, "ticket_created");
        assert!(rows[0].payload.is_some());
    }

    #[tokio::test]
    async fn self_exclusion_removes_actor() {
        let pool = test_pool().await;
        let user_a = seed_user(&pool, "userA", None).await;
        let comp_id = seed_component(&pool, "Comp", user_a).await;
        // user_a is both component owner and creator — should be excluded.
        let ticket_id = seed_ticket(&pool, user_a, comp_id, user_a).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(
                ticket_id,
                NotifEvent::TicketCreated,
                user_a,
                serde_json::json!({"actor": "userA"}),
            )
            .await
            .unwrap();

        assert_eq!(count_pending(&pool).await, 0);
    }

    #[tokio::test]
    async fn comment_added_notifies_owner_and_cc() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let cc_user = seed_user(&pool, "cc_user", None).await;
        let commenter = seed_user(&pool, "commenter", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, commenter).await;
        add_cc(&pool, ticket_id, cc_user).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(
                ticket_id,
                NotifEvent::CommentAdded,
                commenter,
                serde_json::json!({"actor": "commenter"}),
            )
            .await
            .unwrap();

        let rows = get_pending_rows(&pool).await;
        assert_eq!(rows.len(), 2);
        let user_ids: HashSet<i64> = rows.iter().map(|r| r.user_id).collect();
        assert!(user_ids.contains(&owner));
        assert!(user_ids.contains(&cc_user));
        assert!(!user_ids.contains(&commenter));
    }

    #[tokio::test]
    async fn muted_users_excluded() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let actor = seed_user(&pool, "actor", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, actor).await;

        // Owner mutes the ticket.
        mute_ticket(&pool, owner, ticket_id).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(
                ticket_id,
                NotifEvent::StatusChanged,
                actor,
                serde_json::json!({"actor": "actor"}),
            )
            .await
            .unwrap();

        assert_eq!(count_pending(&pool).await, 0);
    }

    #[tokio::test]
    async fn inactive_users_excluded() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let actor = seed_user(&pool, "actor", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, actor).await;

        // Deactivate the owner.
        deactivate_user(&pool, owner).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(
                ticket_id,
                NotifEvent::StatusChanged,
                actor,
                serde_json::json!({"actor": "actor"}),
            )
            .await
            .unwrap();

        assert_eq!(count_pending(&pool).await, 0);
    }

    #[tokio::test]
    async fn milestone_added_notifies_ticket_owner() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let actor = seed_user(&pool, "actor", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, actor).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(
                ticket_id,
                NotifEvent::MilestoneAdded,
                actor,
                serde_json::json!({"actor": "actor", "milestone_name": "v1.0"}),
            )
            .await
            .unwrap();

        let rows = get_pending_rows(&pool).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].user_id, owner);
        assert_eq!(rows[0].event_type, "milestone_added");
    }

    #[tokio::test]
    async fn send_after_uses_delay() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let actor = seed_user(&pool, "actor", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, actor).await;

        let delay = 300;
        let producer = NotificationProducer::new(pool.clone(), delay, true);
        let before = Utc::now();
        producer
            .emit(
                ticket_id,
                NotifEvent::StatusChanged,
                actor,
                serde_json::json!({"actor": "actor"}),
            )
            .await
            .unwrap();

        let rows = get_pending_rows(&pool).await;
        assert_eq!(rows.len(), 1);
        let expected_min = before + chrono::Duration::seconds(delay);
        assert!(rows[0].send_after >= expected_min);
    }

    #[tokio::test]
    async fn payload_stored_as_json() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let actor = seed_user(&pool, "actor", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, actor).await;

        let payload = serde_json::json!({
            "actor": "actor",
            "old_status": "new",
            "new_status": "in_progress"
        });

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(ticket_id, NotifEvent::StatusChanged, actor, payload.clone())
            .await
            .unwrap();

        let rows = get_pending_rows(&pool).await;
        let stored: serde_json::Value =
            serde_json::from_str(rows[0].payload.as_deref().unwrap()).unwrap();
        assert_eq!(stored, payload);
    }

    #[tokio::test]
    async fn deduplication_across_roles() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let actor = seed_user(&pool, "actor", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, actor).await;
        // Owner is also on CC list.
        add_cc(&pool, ticket_id, owner).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit(
                ticket_id,
                NotifEvent::CommentAdded,
                actor,
                serde_json::json!({"actor": "actor"}),
            )
            .await
            .unwrap();

        // Owner should appear only once despite being owner + CC.
        let rows = get_pending_rows(&pool).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].user_id, owner);
    }

    #[tokio::test]
    async fn emit_with_mentions_adds_mentioned_users() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let commenter = seed_user(&pool, "commenter", None).await;
        let mentioned = seed_user(&pool, "mentioned", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, commenter).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        producer
            .emit_with_mentions(
                ticket_id,
                NotifEvent::CommentAdded,
                commenter,
                serde_json::json!({"actor": "commenter"}),
                &[mentioned],
            )
            .await
            .unwrap();

        let rows = get_pending_rows(&pool).await;
        assert_eq!(rows.len(), 2);
        let user_ids: HashSet<i64> = rows.iter().map(|r| r.user_id).collect();
        assert!(user_ids.contains(&owner));
        assert!(user_ids.contains(&mentioned));
        assert!(!user_ids.contains(&commenter));
    }

    #[tokio::test]
    async fn emit_with_mentions_deduplicates_with_owner() {
        let pool = test_pool().await;
        let owner = seed_user(&pool, "owner", None).await;
        let commenter = seed_user(&pool, "commenter", None).await;
        let comp_owner = seed_user(&pool, "comp_owner", None).await;
        let comp_id = seed_component(&pool, "Comp", comp_owner).await;
        let ticket_id = seed_ticket(&pool, owner, comp_id, commenter).await;

        let producer = NotificationProducer::new(pool.clone(), 120, true);
        // Mention the owner who is already a recipient.
        producer
            .emit_with_mentions(
                ticket_id,
                NotifEvent::CommentAdded,
                commenter,
                serde_json::json!({"actor": "commenter"}),
                &[owner],
            )
            .await
            .unwrap();

        // Owner should appear only once.
        let rows = get_pending_rows(&pool).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].user_id, owner);
    }
}
