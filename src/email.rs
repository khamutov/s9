//! Email sender and notification worker per DD 0.6 §6, §10, §12.
//!
//! Provides SMTP transport construction, email template rendering, the
//! background notification worker, and a direct password-reset sender.

use std::sync::Arc;

use chrono::Utc;
use lettre::message::{Mailbox, MultiPart, SinglePart, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::SqlitePool;
use tokio_util::sync::CancellationToken;

use crate::config::SmtpConfig;

/// Shared handle to the SMTP mailer plus metadata needed for building emails.
#[derive(Clone)]
pub struct EmailSender {
    inner: Arc<EmailSenderInner>,
}

struct EmailSenderInner {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    base_url: String,
}

impl EmailSender {
    /// Build an `EmailSender` from the SMTP configuration.
    ///
    /// Returns an error if the TLS mode is invalid or the from-address cannot be parsed.
    pub fn from_config(cfg: &SmtpConfig) -> Result<Self, anyhow::Error> {
        let transport = build_transport(cfg)?;
        let from: Mailbox = format!("S9 <{}>", cfg.from)
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid S9_SMTP_FROM address: {e}"))?;
        Ok(Self {
            inner: Arc::new(EmailSenderInner {
                transport,
                from,
                base_url: cfg.base_url.trim_end_matches('/').to_string(),
            }),
        })
    }

    /// Send a password reset email immediately (bypasses the notification queue).
    ///
    /// Per DD 0.6 §13.4 this is called directly from the password-reset handler.
    #[allow(dead_code)] // Called from password-reset handler (future task).
    pub async fn send_password_reset(
        &self,
        to_email: &str,
        login: &str,
        token: &str,
    ) -> Result<(), anyhow::Error> {
        let to: Mailbox = to_email
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid recipient address: {e}"))?;

        let base = &self.inner.base_url;
        let reset_url = format!("{base}/reset-password?token={token}");

        let html = format!(
            r#"<div style="font-family: sans-serif; max-width: 600px; margin: 0 auto;">
  <p>A password reset was requested for your account ({login}).</p>
  <p>
    <a href="{reset_url}"
       style="display: inline-block; padding: 10px 20px; background: #0066cc;
              color: #fff; text-decoration: none; border-radius: 4px;">
      Reset Password
    </a>
  </p>
  <p>This link expires in 1 hour.</p>
  <p style="font-size: 12px; color: #666;">
    If you did not request this, you can safely ignore this email.
  </p>
</div>"#
        );

        let text = format!(
            "A password reset was requested for your account ({login}).\n\n\
             Reset your password: {reset_url}\n\n\
             This link expires in 1 hour.\n\n\
             If you did not request this, you can safely ignore this email."
        );

        let email = Message::builder()
            .from(self.inner.from.clone())
            .to(to)
            .subject("[S9] Password reset")
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    ),
            )?;

        self.inner.transport.send(email).await?;
        Ok(())
    }

    /// Send a batched notification email for a single (user, ticket) group.
    async fn send_notification(
        &self,
        to_email: &str,
        ticket_id: i64,
        ticket_title: &str,
        events: &[NotificationEvent],
    ) -> Result<(), anyhow::Error> {
        let to: Mailbox = to_email
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid recipient address: {e}"))?;

        let (html, text) =
            build_notification_email(&self.inner.base_url, ticket_id, ticket_title, events);

        let email = Message::builder()
            .from(self.inner.from.clone())
            .to(to)
            .subject(format!("[S9] #{ticket_id}: {ticket_title}"))
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    ),
            )?;

        self.inner.transport.send(email).await?;
        Ok(())
    }
}

/// A single event within a batched notification email.
struct NotificationEvent {
    event_type: String,
    payload: serde_json::Value,
}

/// Build the SMTP transport from configuration per DD 0.6 §5, §6.
fn build_transport(cfg: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>, anyhow::Error> {
    let builder = match cfg.tls.as_str() {
        "none" => {
            tracing::warn!(
                "SMTP connection is unencrypted. Set S9_SMTP_TLS=starttls or tls for production use."
            );
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
        }
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
            .map_err(|e| anyhow::anyhow!("SMTP STARTTLS relay error: {e}"))?,
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)
            .map_err(|e| anyhow::anyhow!("SMTP TLS relay error: {e}"))?,
        other => anyhow::bail!("invalid S9_SMTP_TLS value: {other} (expected none, starttls, tls)"),
    };

    let mut builder = builder.port(cfg.port);

    if let (Some(user), Some(pass)) = (&cfg.username, &cfg.password) {
        builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
    }

    Ok(builder.build())
}

/// Render multipart notification email bodies per DD 0.6 §12.
fn build_notification_email(
    base_url: &str,
    ticket_id: i64,
    ticket_title: &str,
    events: &[NotificationEvent],
) -> (String, String) {
    let ticket_url = format!("{base_url}/tickets/{ticket_id}");

    // Build event detail lines per §12.4.
    let mut html_items = String::new();
    let mut text_items = String::new();

    for evt in events {
        let line = format_event_line(&evt.event_type, &evt.payload);
        html_items.push_str(&format!("    <li>{line}</li>\n"));
        text_items.push_str(&format!("- {line}\n"));
    }

    let html = format!(
        r#"<div style="font-family: sans-serif; max-width: 600px; margin: 0 auto;">
  <p>Changes to <a href="{ticket_url}">#{ticket_id}: {ticket_title}</a>:</p>
  <ul>
{html_items}  </ul>
  <hr style="border: none; border-top: 1px solid #ddd;" />
  <p style="font-size: 12px; color: #666;">
    <a href="{ticket_url}">View ticket</a> &middot;
    To stop receiving emails for this ticket, mute it from the ticket page.
  </p>
</div>"#
    );

    let text = format!(
        "Changes to #{ticket_id}: {ticket_title}\n\n\
         {text_items}\n\
         View ticket: {ticket_url}\n\
         To stop receiving emails for this ticket, mute it from the ticket page."
    );

    (html, text)
}

/// Format a single event detail line per DD 0.6 §12.4.
fn format_event_line(event_type: &str, payload: &serde_json::Value) -> String {
    let actor = payload
        .get("actor")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match event_type {
        "ticket_created" => format!("Ticket created by {actor}"),
        "comment_added" => format!("New comment by {actor}"),
        "status_changed" => {
            let old = payload
                .get("old_status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let new = payload
                .get("new_status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("Status changed from {old} to {new} by {actor}")
        }
        "priority_changed" => {
            let old = payload
                .get("old_priority")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let new = payload
                .get("new_priority")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("Priority changed from {old} to {new} by {actor}")
        }
        "owner_changed" => {
            let old = payload
                .get("old_owner")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let new = payload
                .get("new_owner")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("Owner changed from {old} to {new} by {actor}")
        }
        "milestone_added" => {
            let name = payload
                .get("milestone_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("Added to milestone {name} by {actor}")
        }
        other => format!("{other} by {actor}"),
    }
}

/// A row returned by the batching query, grouping notifications per (user, ticket).
#[derive(Debug, sqlx::FromRow)]
struct BatchedGroup {
    user_id: i64,
    ticket_id: i64,
    notification_ids: String,
    event_types: String,
    payloads: String,
}

/// Background notification worker per DD 0.6 §10.
///
/// Polls `pending_notifications` every 30 seconds, groups by `(user_id, ticket_id)`,
/// sends batched emails, and deletes processed rows. Also purges rows older than 24h.
pub async fn notification_worker(
    pool: SqlitePool,
    sender: Option<EmailSender>,
    cancel: CancellationToken,
) {
    let tick = std::time::Duration::from_secs(30);
    tracing::info!("notification worker started (tick interval: 30s)");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("notification worker shutting down");
                return;
            }
            _ = tokio::time::sleep(tick) => {}
        }

        // Purge expired rows (>24h) regardless of SMTP status per §11.3.
        if let Err(e) = purge_expired(&pool).await {
            tracing::error!("notification worker: failed to purge expired rows: {e}");
        }

        // Only attempt sending if we have an email sender.
        let Some(ref sender) = sender else {
            continue;
        };

        if let Err(e) = process_pending(&pool, sender).await {
            tracing::error!("notification worker: send pass failed: {e}");
            // Per §11.1: skip this tick, rows remain for next retry.
        }
    }
}

/// Process all ready notification batches.
async fn process_pending(pool: &SqlitePool, sender: &EmailSender) -> Result<(), anyhow::Error> {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();

    // Use ASCII Record Separator (0x1E) as group_concat delimiter because
    // JSON payloads contain commas that would corrupt comma-delimited splitting.
    let groups: Vec<BatchedGroup> = sqlx::query_as(
        "SELECT user_id, ticket_id, group_concat(id) AS notification_ids,
                group_concat(event_type, char(30)) AS event_types,
                group_concat(payload, char(30)) AS payloads
         FROM pending_notifications
         WHERE send_after <= ?
         GROUP BY user_id, ticket_id
         ORDER BY min(created_at)",
    )
    .bind(&now)
    .fetch_all(pool)
    .await?;

    if groups.is_empty() {
        return Ok(());
    }

    tracing::debug!("notification worker: processing {} batches", groups.len());

    for group in &groups {
        // Look up recipient email.
        let recipient: Option<(String,)> =
            sqlx::query_as("SELECT email FROM users WHERE id = ? AND is_active = 1")
                .bind(group.user_id)
                .fetch_optional(pool)
                .await?;
        let Some((email,)) = recipient else {
            // User inactive or deleted — discard notifications.
            delete_notification_ids(pool, &group.notification_ids).await?;
            continue;
        };

        if email.is_empty() {
            delete_notification_ids(pool, &group.notification_ids).await?;
            continue;
        }

        // Look up ticket title.
        let ticket_title: String = sqlx::query_scalar("SELECT title FROM tickets WHERE id = ?")
            .bind(group.ticket_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| format!("Ticket #{}", group.ticket_id));

        // Parse event list (split on ASCII Record Separator 0x1E).
        let event_types: Vec<&str> = group.event_types.split('\x1E').collect();
        let payloads_raw: Vec<&str> = group.payloads.split('\x1E').collect();
        let events: Vec<NotificationEvent> = event_types
            .into_iter()
            .zip(payloads_raw)
            .map(|(et, p)| NotificationEvent {
                event_type: et.to_string(),
                payload: serde_json::from_str(p)
                    .unwrap_or(serde_json::Value::Object(Default::default())),
            })
            .collect();

        match sender
            .send_notification(&email, group.ticket_id, &ticket_title, &events)
            .await
        {
            Ok(()) => {
                tracing::debug!(
                    "notification worker: sent email to {} for ticket #{}",
                    email,
                    group.ticket_id
                );
                delete_notification_ids(pool, &group.notification_ids).await?;
            }
            Err(e) => {
                // Per §11.2: log warning and delete (likely persistently invalid address).
                tracing::warn!("notification worker: failed to send to {}: {e}", email);
                delete_notification_ids(pool, &group.notification_ids).await?;
            }
        }
    }

    Ok(())
}

/// Delete notification rows by comma-separated IDs.
async fn delete_notification_ids(pool: &SqlitePool, ids_csv: &str) -> Result<(), sqlx::Error> {
    // IDs come from group_concat — they are numeric, safe to interpolate.
    let sql = format!("DELETE FROM pending_notifications WHERE id IN ({ids_csv})");
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

/// Purge notifications older than 24 hours per DD 0.6 §11.3.
async fn purge_expired(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM pending_notifications
         WHERE created_at < strftime('%Y-%m-%dT%H:%M:%fZ', datetime('now', '-24 hours'))",
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_event_ticket_created() {
        let payload = serde_json::json!({"actor": "alice"});
        assert_eq!(
            format_event_line("ticket_created", &payload),
            "Ticket created by alice"
        );
    }

    #[test]
    fn format_event_comment_added() {
        let payload = serde_json::json!({"actor": "bob"});
        assert_eq!(
            format_event_line("comment_added", &payload),
            "New comment by bob"
        );
    }

    #[test]
    fn format_event_status_changed() {
        let payload =
            serde_json::json!({"actor": "alice", "old_status": "new", "new_status": "in_progress"});
        assert_eq!(
            format_event_line("status_changed", &payload),
            "Status changed from new to in_progress by alice"
        );
    }

    #[test]
    fn format_event_priority_changed() {
        let payload =
            serde_json::json!({"actor": "alice", "old_priority": "P3", "new_priority": "P1"});
        assert_eq!(
            format_event_line("priority_changed", &payload),
            "Priority changed from P3 to P1 by alice"
        );
    }

    #[test]
    fn format_event_owner_changed() {
        let payload =
            serde_json::json!({"actor": "alice", "old_owner": "bob", "new_owner": "carol"});
        assert_eq!(
            format_event_line("owner_changed", &payload),
            "Owner changed from bob to carol by alice"
        );
    }

    #[test]
    fn format_event_milestone_added() {
        let payload = serde_json::json!({"actor": "alice", "milestone_name": "v1.0"});
        assert_eq!(
            format_event_line("milestone_added", &payload),
            "Added to milestone v1.0 by alice"
        );
    }

    #[test]
    fn build_notification_email_html_and_text() {
        let events = vec![
            NotificationEvent {
                event_type: "status_changed".to_string(),
                payload: serde_json::json!({"actor": "alice", "old_status": "new", "new_status": "in_progress"}),
            },
            NotificationEvent {
                event_type: "priority_changed".to_string(),
                payload: serde_json::json!({"actor": "alice", "old_priority": "P3", "new_priority": "P1"}),
            },
        ];

        let (html, text) =
            build_notification_email("https://bugs.example.com", 42, "Login broken", &events);

        assert!(html.contains("https://bugs.example.com/tickets/42"));
        assert!(html.contains("#42: Login broken"));
        assert!(html.contains("Status changed from new to in_progress by alice"));
        assert!(html.contains("Priority changed from P3 to P1 by alice"));

        assert!(text.contains("Changes to #42: Login broken"));
        assert!(text.contains("View ticket: https://bugs.example.com/tickets/42"));
    }

    #[test]
    fn build_notification_email_single_event() {
        let events = vec![NotificationEvent {
            event_type: "comment_added".to_string(),
            payload: serde_json::json!({"actor": "bob"}),
        }];

        let (html, text) = build_notification_email("https://s9.test", 7, "Fix typo", &events);

        assert!(html.contains("New comment by bob"));
        assert!(text.contains("New comment by bob"));
        assert!(text.contains("#7: Fix typo"));
    }

    /// Seed a user and component+ticket for FK-valid pending_notifications inserts.
    async fn seed_test_data(pool: &SqlitePool) -> (i64, i64) {
        use crate::models::{CreateComponentRequest, CreateUserRequest};
        use crate::repos::{component, user};

        let user_id = user::create(
            pool,
            &CreateUserRequest {
                login: "testuser".to_string(),
                display_name: "Test User".to_string(),
                email: "test@example.com".to_string(),
                password: None,
                role: None,
            },
            None,
        )
        .await
        .unwrap()
        .id;

        let comp_id = component::create(
            pool,
            &CreateComponentRequest {
                name: "TestComp".to_string(),
                parent_id: None,
                slug: Some("TC".to_string()),
                owner_id: user_id,
            },
        )
        .await
        .unwrap()
        .id;

        let now = Utc::now();
        let ticket_id: i64 = sqlx::query_scalar(
            "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, created_by, created_at, updated_at)
             VALUES ('bug', 'Test', 'new', 'P3', ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user_id)
        .bind(comp_id)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap();

        (user_id, ticket_id)
    }

    #[tokio::test]
    async fn worker_purges_expired_rows() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();
        let (user_id, ticket_id) = seed_test_data(&pool).await;

        sqlx::query(
            "INSERT INTO pending_notifications (user_id, ticket_id, event_type, payload, created_at, send_after)
             VALUES (?, ?, 'test', '{}', datetime('now', '-25 hours'), datetime('now', '-25 hours'))",
        )
        .bind(user_id)
        .bind(ticket_id)
        .execute(&pool)
        .await
        .unwrap();

        purge_expired(&pool).await.unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pending_notifications")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    /// Helper: seed a second user for batching tests.
    async fn seed_second_user(pool: &SqlitePool) -> i64 {
        use crate::models::CreateUserRequest;
        use crate::repos::user;

        user::create(
            pool,
            &CreateUserRequest {
                login: "user2".to_string(),
                display_name: "User Two".to_string(),
                email: "user2@example.com".to_string(),
                password: None,
                role: None,
            },
            None,
        )
        .await
        .unwrap()
        .id
    }

    /// Helper: seed a second ticket for batching tests.
    async fn seed_second_ticket(pool: &SqlitePool, user_id: i64, comp_id: i64) -> i64 {
        let now = Utc::now();
        sqlx::query_scalar(
            "INSERT INTO tickets (type, title, status, priority, owner_id, component_id, created_by, created_at, updated_at)
             VALUES ('bug', 'Second ticket', 'new', 'P3', ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user_id)
        .bind(comp_id)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// Insert a pending notification with configurable send_after.
    async fn insert_pending(
        pool: &SqlitePool,
        user_id: i64,
        ticket_id: i64,
        event_type: &str,
        payload: &str,
        send_after_offset_secs: i64,
    ) {
        let now = Utc::now();
        let send_after = now + chrono::Duration::seconds(send_after_offset_secs);
        sqlx::query(
            "INSERT INTO pending_notifications (user_id, ticket_id, event_type, payload, created_at, send_after)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(ticket_id)
        .bind(event_type)
        .bind(payload)
        .bind(now)
        .bind(send_after)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn batching_groups_by_user_and_ticket() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();
        let (user_id, ticket_id) = seed_test_data(&pool).await;

        // Insert two ready events for the same (user, ticket) with JSON payloads containing commas.
        insert_pending(
            &pool,
            user_id,
            ticket_id,
            "status_changed",
            r#"{"actor":"alice","old_status":"new","new_status":"in_progress"}"#,
            -10, // already past send_after
        )
        .await;
        insert_pending(
            &pool,
            user_id,
            ticket_id,
            "priority_changed",
            r#"{"actor":"alice","old_priority":"P3","new_priority":"P1"}"#,
            -5,
        )
        .await;

        // Run the batching query directly.
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
        let groups: Vec<BatchedGroup> = sqlx::query_as(
            "SELECT user_id, ticket_id, group_concat(id) AS notification_ids,
                    group_concat(event_type, char(30)) AS event_types,
                    group_concat(payload, char(30)) AS payloads
             FROM pending_notifications
             WHERE send_after <= ?
             GROUP BY user_id, ticket_id
             ORDER BY min(created_at)",
        )
        .bind(&now)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            groups.len(),
            1,
            "two events for same (user, ticket) should batch into one group"
        );
        let group = &groups[0];
        assert_eq!(group.user_id, user_id);
        assert_eq!(group.ticket_id, ticket_id);

        // Verify event_types parse correctly with RS separator.
        let event_types: Vec<&str> = group.event_types.split('\x1E').collect();
        assert_eq!(event_types, vec!["status_changed", "priority_changed"]);

        // Verify payloads parse correctly (JSON with commas must not be corrupted).
        let payloads: Vec<&str> = group.payloads.split('\x1E').collect();
        assert_eq!(payloads.len(), 2);
        let p0: serde_json::Value = serde_json::from_str(payloads[0]).unwrap();
        assert_eq!(p0["old_status"], "new");
        assert_eq!(p0["new_status"], "in_progress");
        let p1: serde_json::Value = serde_json::from_str(payloads[1]).unwrap();
        assert_eq!(p1["old_priority"], "P3");
    }

    #[tokio::test]
    async fn batching_separates_different_tickets() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();
        let (user_id, ticket_id) = seed_test_data(&pool).await;

        // Get the component ID for the second ticket.
        let comp_id: i64 = sqlx::query_scalar("SELECT component_id FROM tickets WHERE id = ?")
            .bind(ticket_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let ticket_id_2 = seed_second_ticket(&pool, user_id, comp_id).await;

        // Insert one event per ticket, both ready.
        insert_pending(
            &pool,
            user_id,
            ticket_id,
            "comment_added",
            r#"{"actor":"bob"}"#,
            -10,
        )
        .await;
        insert_pending(
            &pool,
            user_id,
            ticket_id_2,
            "ticket_created",
            r#"{"actor":"bob"}"#,
            -10,
        )
        .await;

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
        let groups: Vec<BatchedGroup> = sqlx::query_as(
            "SELECT user_id, ticket_id, group_concat(id) AS notification_ids,
                    group_concat(event_type, char(30)) AS event_types,
                    group_concat(payload, char(30)) AS payloads
             FROM pending_notifications
             WHERE send_after <= ?
             GROUP BY user_id, ticket_id
             ORDER BY min(created_at)",
        )
        .bind(&now)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            groups.len(),
            2,
            "events for different tickets must not be grouped together"
        );
    }

    #[tokio::test]
    async fn batching_respects_send_after_window() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();
        let (user_id, ticket_id) = seed_test_data(&pool).await;

        // One event past its send_after, one still in the future.
        insert_pending(
            &pool,
            user_id,
            ticket_id,
            "comment_added",
            r#"{"actor":"a"}"#,
            -10,
        )
        .await;
        insert_pending(
            &pool,
            user_id,
            ticket_id,
            "status_changed",
            r#"{"actor":"a"}"#,
            300,
        )
        .await;

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
        let groups: Vec<BatchedGroup> = sqlx::query_as(
            "SELECT user_id, ticket_id, group_concat(id) AS notification_ids,
                    group_concat(event_type, char(30)) AS event_types,
                    group_concat(payload, char(30)) AS payloads
             FROM pending_notifications
             WHERE send_after <= ?
             GROUP BY user_id, ticket_id
             ORDER BY min(created_at)",
        )
        .bind(&now)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(groups.len(), 1);
        // Only the ready event should be in the batch.
        let event_types: Vec<&str> = groups[0].event_types.split('\x1E').collect();
        assert_eq!(event_types, vec!["comment_added"]);

        // The future event should still be in the table.
        let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pending_notifications")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 2, "future event must remain pending");
    }

    #[tokio::test]
    async fn batching_groups_different_users_separately() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();
        let (user_id, ticket_id) = seed_test_data(&pool).await;
        let user_id_2 = seed_second_user(&pool).await;

        // Same ticket, different users.
        insert_pending(
            &pool,
            user_id,
            ticket_id,
            "comment_added",
            r#"{"actor":"x"}"#,
            -10,
        )
        .await;
        insert_pending(
            &pool,
            user_id_2,
            ticket_id,
            "comment_added",
            r#"{"actor":"x"}"#,
            -10,
        )
        .await;

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
        let groups: Vec<BatchedGroup> = sqlx::query_as(
            "SELECT user_id, ticket_id, group_concat(id) AS notification_ids,
                    group_concat(event_type, char(30)) AS event_types,
                    group_concat(payload, char(30)) AS payloads
             FROM pending_notifications
             WHERE send_after <= ?
             GROUP BY user_id, ticket_id
             ORDER BY min(created_at)",
        )
        .bind(&now)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            groups.len(),
            2,
            "same ticket for different users must produce separate batches"
        );
    }

    #[tokio::test]
    async fn delete_notification_ids_removes_rows() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();
        let (user_id, ticket_id) = seed_test_data(&pool).await;

        let now = Utc::now();
        for _ in 0..3 {
            sqlx::query(
                "INSERT INTO pending_notifications (user_id, ticket_id, event_type, payload, created_at, send_after)
                 VALUES (?, ?, 'test', '{}', ?, ?)",
            )
            .bind(user_id)
            .bind(ticket_id)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        }

        delete_notification_ids(&pool, "1,2").await.unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pending_notifications")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
