# Design Document: Email Notifications

| Field        | Value                                              |
|--------------|----------------------------------------------------|
| Status       | Draft                                              |
| Author       | khamutov, Claude co-authored                       |
| Last updated | 2026-03-06                                         |
| PRD ref      | 1. Initial PRD, §7                                 |
| Depends on   | DD 0.1 (Database Schema), DD 0.3 (Auth & Sessions), DD 0.4 (Endpoint Schema), DD 0.8 (Build Pipeline) |

---

## 1. Context and Scope

S9 is a Rust/axum + React bug tracker shipped as a single embedded binary. Several prior design documents established the foundations that the notification system builds on:

- **DD 0.1 §7.12–7.13:** Defined the `notification_mutes` and `pending_notifications` tables.
- **DD 0.3 §13.2:** Defined the password reset flow and deferred the email template to this DD (open question #3).
- **DD 0.4 §4:** Listed "Email notification payloads" as a non-goal, deferred to this DD. §13 defined the mute/unmute endpoints.
- **DD 0.8 §10:** Defined the startup sequence (notification worker needs to be added). §12: Docker runtime image includes CA certificates for TLS.
- **PRD §7:** Specifies six event types, recipient mapping, batching with configurable delay, SMTP configuration, per-ticket mute.

This document specifies the full email notification design: SMTP configuration, event production, recipient resolution, batching algorithm, background worker, email templates (including password reset), retry strategy, and graceful degradation. It unblocks:

- **4.1** Notification event producer
- **4.2** Email sender (SMTP/lettre)
- **4.3** Notification batching (2-min delay)
- **4.4** Per-ticket mute preferences
- **4.5** @mention parsing in comments

## 2. Problem Statement

Before writing any notification code we need to decide:

- How SMTP is configured and what happens when it is not configured.
- The event producer pattern: how API handlers emit notification events.
- Recipient resolution: who receives notifications for each event type.
- Batching algorithm: how rapid changes are collapsed into a single email.
- Background worker design: polling, sending, error recovery.
- Email templates for all notification types.
- Password reset email template (resolves DD 0.3 open question #3).
- Retry and expiry strategy for failed sends.

## 3. Goals

- Configure SMTP via environment variables, consistent with the OIDC configuration pattern (DD 0.3 §10.1).
- Define event-to-recipient mapping for all six PRD §7.1 event types.
- Design batching on top of the existing `pending_notifications` schema (DD 0.1 §7.13).
- Provide email templates for all notification types and the password reset flow.
- System works without SMTP configured — email is optional, all other features remain functional.

## 4. Non-goals

- Inbound email processing (PRD §7.2: not in scope for v1).
- Rich HTML templates with CSS frameworks or template engines.
- Delivery tracking or read receipts.
- Per-event-type user preferences (beyond per-ticket mute from DD 0.4).
- Push notifications or webhooks.

## 5. SMTP Configuration

### Option A: Environment variables `[selected]`

Configure SMTP via env vars, consistent with how OIDC is configured (DD 0.3 §10.1).

**Pros:**
- Consistent with existing configuration pattern.
- Credentials stay out of the database.
- Easy to configure in Docker/container environments.

**Cons:**
- Requires restart to change settings.

### Option B: Database-stored settings `[rejected]`

Store SMTP credentials in a settings table, configurable via admin API.

**Pros:**
- Runtime reconfiguration without restart.

**Cons:**
- Credentials stored in SQLite — less secure than environment variables.
- Adds complexity: admin API, settings table, credential encryption.
- Inconsistent with the OIDC configuration pattern.

**Decision:** Option A. Environment variables are consistent with the established pattern and keep credentials out of the database.

### 5.1 Environment variables

| Variable                 | Required | Default    | Description                                              |
|--------------------------|----------|------------|----------------------------------------------------------|
| `S9_SMTP_HOST`           | No       | *(unset)*  | SMTP server hostname. When unset, email is disabled.     |
| `S9_SMTP_PORT`           | No       | `587`      | SMTP server port.                                        |
| `S9_SMTP_USERNAME`       | No       | *(unset)*  | SMTP authentication username.                            |
| `S9_SMTP_PASSWORD`       | No       | *(unset)*  | SMTP authentication password.                            |
| `S9_SMTP_TLS`            | No       | `starttls` | TLS mode: `none`, `starttls`, or `tls`.                  |
| `S9_SMTP_FROM`           | Yes*     | —          | Sender email address. *Required when `S9_SMTP_HOST` is set. |
| `S9_BASE_URL`            | Yes*     | —          | Base URL for links in emails (e.g. `https://bugs.example.com`). *Required when `S9_SMTP_HOST` is set. |
| `S9_NOTIFICATION_DELAY`  | No       | `120`      | Seconds to delay before sending notifications (batching window). |

Validation at startup:
- If `S9_SMTP_HOST` is set, `S9_SMTP_FROM` and `S9_BASE_URL` must also be set. Fail with a clear error message otherwise.
- If `S9_SMTP_TLS` is `none`, log a warning: "SMTP connection is unencrypted."

## 6. SMTP Crate

### Option A: `lettre` `[selected]`

The de-facto Rust SMTP library. Async tokio transport, STARTTLS and implicit TLS, connection pooling, MIME multipart builder.

**Pros:**
- Mature, well-maintained, widely used in the Rust ecosystem.
- Native async/tokio support (`AsyncSmtpTransport`).
- Built-in `MultiPart` builder for HTML + plain text emails.
- STARTTLS, implicit TLS, and plaintext transports.

**Cons:**
- None significant for this use case.

### Option B: Raw TCP/SMTP implementation `[rejected]`

Implement SMTP protocol directly over TCP.

**Pros:**
- No external dependency.

**Cons:**
- Reinvents `lettre` — TLS negotiation, AUTH mechanisms, MIME encoding.
- Significant implementation and maintenance burden.

**Decision:** Option A. `lettre` is the standard choice. TASKS.md already references it for task 4.2.

## 7. Event Producer

### 7.1 `NotificationProducer`

A `NotificationProducer` service is called from API handlers after successful mutations. It receives the event details and inserts rows into `pending_notifications`.

```rust
/// Produces notification events by inserting rows into pending_notifications.
pub struct NotificationProducer {
    pool: SqlitePool,
    delay_seconds: i64,
    smtp_enabled: bool,
}
```

Handlers call `NotificationProducer` after committing the mutation. If SMTP is disabled (`smtp_enabled = false`), the producer is a no-op — no rows are inserted (§15).

### 7.2 Event types

Six event types from PRD §7.1:

| Event Type          | Trigger                                   |
|---------------------|-------------------------------------------|
| `ticket_created`    | `POST /api/tickets`                       |
| `comment_added`     | `POST /api/tickets/:id/comments`          |
| `status_changed`    | `PATCH /api/tickets/:id` with status      |
| `priority_changed`  | `PATCH /api/tickets/:id` with priority    |
| `owner_changed`     | `PATCH /api/tickets/:id` with owner       |
| `milestone_added`   | `PATCH /api/tickets/:id` with milestone   |

A single `PATCH /api/tickets/:id` request can change multiple fields simultaneously (e.g., status + priority). The producer emits one event per changed field, each with its own `pending_notifications` row.

### 7.3 Producer interface

```rust
impl NotificationProducer {
    /// Record a notification event. `actor_id` is the user who performed the action
    /// (excluded from recipients). `payload` is JSON metadata for the email template.
    pub async fn emit(
        &self,
        ticket_id: i64,
        event_type: &str,
        actor_id: i64,
        payload: serde_json::Value,
    ) -> Result<()>;
}
```

The `emit` method performs recipient resolution (§8) and inserts one row per recipient into `pending_notifications` with `send_after = now() + delay`.

## 8. Recipient Resolution

### 8.1 Recipient mapping

Per PRD §7.1:

| Event Type          | Raw Recipients                                        |
|---------------------|-------------------------------------------------------|
| `ticket_created`    | Component owner                                       |
| `comment_added`     | Ticket owner, CC list, mentioned users (`@login`)     |
| `status_changed`    | Ticket owner, CC list                                 |
| `priority_changed`  | Ticket owner, CC list                                 |
| `owner_changed`     | Old owner, new owner, CC list                         |
| `milestone_added`   | Ticket owner                                          |

### 8.2 Resolution algorithm

For each event, the producer:

1. **Compute raw recipients** per the mapping table above (query ticket, component, CC list as needed).
2. **Self-exclusion:** Remove the actor (the user who triggered the event).
3. **Deduplication:** Collect into a `HashSet<i64>` to remove duplicates (e.g., actor is also on CC list).
4. **Mute check:** Query `notification_mutes` and remove users who have muted this ticket.
5. **Active user check:** Skip users with `is_active = 0`.
6. **Insert:** One `pending_notifications` row per remaining user, with `send_after = now() + delay`.

Resolution happens at event production time (not at send time) so that the recipient list reflects the state when the event occurred. If a user mutes a ticket after an event but before the email is sent, they still receive that email — the mute applies to future events only.

## 9. Batching Strategy

### 9.1 Design

Batching uses the `send_after` column on `pending_notifications` (DD 0.1 §7.13):

- On insertion: `send_after = now() + S9_NOTIFICATION_DELAY` (default 120 seconds).
- The background worker (§10) polls for ready notifications grouped by `(user_id, ticket_id)`.
- All events for the same user and ticket within the window are combined into a single email.

### 9.2 Fixed window

The batching window is **fixed** — it is not extended when new events arrive for the same `(user_id, ticket_id)` pair. Each event gets its own `send_after` timestamp. The worker query groups all rows where `send_after <= now()`, so naturally accumulating events arrive in the same batch as long as they occur within the delay window.

This avoids indefinite delays during rapid editing while still collapsing most related changes.

### 9.3 Worker query

```sql
SELECT user_id, ticket_id, group_concat(id) AS notification_ids,
       group_concat(event_type) AS event_types,
       group_concat(payload) AS payloads
FROM pending_notifications
WHERE send_after <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
GROUP BY user_id, ticket_id
ORDER BY min(created_at);
```

After sending each email, delete the processed rows:

```sql
DELETE FROM pending_notifications WHERE id IN (:ids);
```

## 10. Background Worker

### 10.1 Design

A tokio task spawned at startup, following the same pattern as the session cleanup task (DD 0.3 §7.4) and orphan attachment cleanup (DD 0.5 §8).

```rust
/// Background task that polls pending_notifications and sends batched emails.
pub async fn notification_worker(
    pool: SqlitePool,
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: Address,
    base_url: String,
    cancel: CancellationToken,
);
```

### 10.2 Behavior

- **Tick interval:** 30 seconds.
- Each tick: run the batching query (§9.3), build and send emails, delete processed rows.
- Respects `CancellationToken` for graceful shutdown (DD 0.8 §10 pattern).
- **When SMTP is disabled:** the worker still runs but only performs cleanup — purging rows older than 24 hours (§11.3). This handles edge cases where rows were inserted before SMTP was disabled.

## 11. Error Handling and Retry

### Option A: Natural retry via tick + age-based expiry `[selected]`

On SMTP failure, skip the current tick. The rows remain in `pending_notifications` and are retried on the next tick (30 seconds later). Rows older than 24 hours are purged regardless.

**Pros:**
- Simple — no per-row retry counters or backoff state.
- Naturally handles transient SMTP outages (server restart, network blip).
- 24-hour expiry prevents unbounded queue growth.

**Cons:**
- Fixed 30-second retry interval (not exponential).

### Option B: Exponential backoff per notification `[rejected]`

Track retry count and next-retry-at per row. Apply exponential backoff with jitter.

**Pros:**
- More sophisticated retry behavior.

**Cons:**
- Overengineered for this use case — notifications are best-effort.
- Requires additional columns on `pending_notifications`.

**Decision:** Option A. Notifications are best-effort. Natural retry with 24-hour expiry is sufficient.

### 11.1 SMTP transport failure

If the SMTP connection fails (timeout, refused, TLS error), log the error and skip the entire tick. All pending rows remain for the next tick.

### 11.2 Per-recipient failure

If a specific email fails to send (bad address, mailbox full), log a warning and delete the notification rows for that recipient. Do not retry — the address is likely persistently invalid.

### 11.3 Expiry cleanup

Run on every tick, after the send pass:

```sql
DELETE FROM pending_notifications
WHERE created_at < strftime('%Y-%m-%dT%H:%M:%fZ', datetime('now', '-24 hours'));
```

## 12. Email Format and Templates

### Option A: Plain text only `[rejected]`

Send notifications as plain text emails.

**Pros:**
- Simplest to implement.
- Universal rendering.

**Cons:**
- No clickable links guaranteed (many clients auto-link, but not all).
- No visual distinction between sections.

### Option B: Multipart HTML + plain text `[selected]`

Send emails as `multipart/alternative` with both HTML and plain text parts using `lettre`'s `MultiPart` builder.

**Pros:**
- Clickable links in HTML part.
- Minimal visual structure (headers, dividers) for readability.
- Plain text fallback for clients that prefer it.

**Cons:**
- Slightly more implementation work.

**Decision:** Option B. Multipart emails ensure clickable links while providing a plain text fallback. HTML uses inline styles only — no external CSS, no images.

### 12.1 Subject line

```
[S9] #{ticket_id}: {ticket_title}
```

Example: `[S9] #42: Login button not responding on mobile`

The `[S9]` prefix enables mail filtering rules.

### 12.2 HTML body structure

```html
<div style="font-family: sans-serif; max-width: 600px; margin: 0 auto;">
  <p>Changes to <a href="{base_url}/tickets/{ticket_id}">#{ticket_id}: {ticket_title}</a>:</p>
  <ul>
    <!-- one <li> per event in the batch -->
    <li><strong>Status changed</strong> from <em>new</em> to <em>in_progress</em> by alex</li>
    <li><strong>Priority changed</strong> from <em>medium</em> to <em>high</em> by alex</li>
  </ul>
  <hr style="border: none; border-top: 1px solid #ddd;" />
  <p style="font-size: 12px; color: #666;">
    <a href="{base_url}/tickets/{ticket_id}">View ticket</a> ·
    To stop receiving emails for this ticket, mute it from the ticket page.
  </p>
</div>
```

### 12.3 Plain text body structure

```
Changes to #{ticket_id}: {ticket_title}

- Status changed from new to in_progress by alex
- Priority changed from medium to high by alex

View ticket: {base_url}/tickets/{ticket_id}
To stop receiving emails for this ticket, mute it from the ticket page.
```

### 12.4 Event detail lines

| Event Type          | Detail Line                                                    |
|---------------------|----------------------------------------------------------------|
| `ticket_created`    | Ticket created by {actor}                                      |
| `comment_added`     | New comment by {actor}                                         |
| `status_changed`    | Status changed from {old} to {new} by {actor}                  |
| `priority_changed`  | Priority changed from {old} to {new} by {actor}                |
| `owner_changed`     | Owner changed from {old_owner} to {new_owner} by {actor}       |
| `milestone_added`   | Added to milestone {milestone_name} by {actor}                 |

### 12.5 Template implementation

No template engine crate. The template set is small and fixed — `format!()` macros and string concatenation are sufficient. Templates are Rust functions:

```rust
fn build_notification_email(
    base_url: &str,
    ticket_id: i64,
    ticket_title: &str,
    events: &[NotificationEvent],
) -> (String, String)  // (html_body, text_body)
```

## 13. Password Reset Email Template

Resolves DD 0.3 open question #3.

### 13.1 Subject

```
[S9] Password reset
```

### 13.2 HTML body

```html
<div style="font-family: sans-serif; max-width: 600px; margin: 0 auto;">
  <p>A password reset was requested for your account ({login}).</p>
  <p>
    <a href="{base_url}/reset-password?token={token}"
       style="display: inline-block; padding: 10px 20px; background: #0066cc;
              color: #fff; text-decoration: none; border-radius: 4px;">
      Reset Password
    </a>
  </p>
  <p>This link expires in 1 hour.</p>
  <p style="font-size: 12px; color: #666;">
    If you did not request this, you can safely ignore this email.
  </p>
</div>
```

### 13.3 Plain text body

```
A password reset was requested for your account ({login}).

Reset your password: {base_url}/reset-password?token={token}

This link expires in 1 hour.

If you did not request this, you can safely ignore this email.
```

### 13.4 Sending behavior

Password reset emails are **sent immediately** — they bypass the batching queue entirely because they are time-sensitive (1-hour token expiry per DD 0.3 §13.2).

The password reset handler calls the SMTP transport directly:

```rust
/// Send a password reset email. Bypasses the notification queue.
pub async fn send_password_reset(
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    from: &Address,
    base_url: &str,
    to_email: &str,
    login: &str,
    token: &str,
) -> Result<()>;
```

If SMTP is not configured: log a warning and return `Ok(())`. The `POST /api/auth/password-reset/request` handler returns 200 regardless (per DD 0.3 §13.2 anti-enumeration design).

## 14. Event Metadata Storage

### Option A: Look up current state at send time `[rejected]`

When sending the email, query the ticket for its current state to populate the template.

**Pros:**
- No additional storage needed.

**Cons:**
- Loses change details — cannot show "status changed from X to Y" because intermediate states are lost by send time.
- Race condition: state may have changed again between event and send.

### Option B: JSON `payload` column on `pending_notifications` `[selected]`

Store event metadata (old/new values, actor login) as JSON in a new `payload` column.

**Pros:**
- Preserves exact change details for the email template.
- Decouples email rendering from current ticket state.

**Cons:**
- Additional column on `pending_notifications`.

**Decision:** Option B. Accurate change details in emails are worth the extra column.

### 14.1 Schema addition

```sql
ALTER TABLE pending_notifications ADD COLUMN payload TEXT;
```

The column is nullable (JSON text). Added as a migration alongside the existing `pending_notifications` table.

### 14.2 Payload examples

**`status_changed`:**
```json
{"actor": "alex", "old_status": "new", "new_status": "in_progress"}
```

**`owner_changed`:**
```json
{"actor": "alex", "old_owner": "bob", "new_owner": "carol"}
```

**`comment_added`:**
```json
{"actor": "alex"}
```

**`ticket_created`:**
```json
{"actor": "alex"}
```

**`priority_changed`:**
```json
{"actor": "alex", "old_priority": "medium", "new_priority": "high"}
```

**`milestone_added`:**
```json
{"actor": "alex", "milestone_name": "v1.0"}
```

## 15. Graceful Degradation

When `S9_SMTP_HOST` is not set:

| Component                    | Behavior                                                       |
|------------------------------|----------------------------------------------------------------|
| `NotificationProducer`       | No-op — does not insert rows into `pending_notifications`.     |
| Mute endpoints               | Work normally — operate on `notification_mutes`, independent of email. |
| Password reset               | Returns 200 but sends no email. Logs warning at `WARN` level. |
| Background worker            | Runs for cleanup only — purges any stale rows older than 24h.  |
| Startup                      | Logs `INFO`: "SMTP not configured, email notifications disabled." |

This ensures S9 is fully functional without email. Operators can enable email later by setting the env vars and restarting.

## 16. SSE Integration

SSE (Server-Sent Events) and email are independent notification channels. Both are triggered from the same API handlers but serve different purposes:

| Aspect     | SSE                                     | Email                                          |
|------------|------------------------------------------|-------------------------------------------------|
| Delivery   | Real-time broadcast to connected clients | Batched, delayed delivery                       |
| Recipients | All connected sessions                   | Role-based per PRD §7.1, mutable per-ticket     |
| State      | Stateless (fire and forget)              | Queued in `pending_notifications`               |
| Dependency | DD 0.4 §8                               | This DD                                         |

API handlers invoke both: broadcast SSE event and call `NotificationProducer::emit`. The two paths share no state or logic.

## 17. Security Considerations

- **SMTP credentials:** Stored in environment variables only. Never in the database, never in API responses, never logged.
- **Email content:** Limited to information the recipient can already see in the UI. No private data from other users' contexts.
- **TLS:** STARTTLS is the default. If `S9_SMTP_TLS=none`, log a warning: "SMTP connection is unencrypted. Set S9_SMTP_TLS=starttls or tls for production use."
- **Anti-enumeration:** Password reset always returns 200, whether email exists or not (per DD 0.3 §13.2).
- **No per-user rate limit in v1:** Batching provides a natural throttle — at most one email per ticket per delay window per user. This is sufficient for v1 scale.

## 18. Schema Additions

Single addition to the existing `pending_notifications` table (DD 0.1 §7.13):

```sql
ALTER TABLE pending_notifications ADD COLUMN payload TEXT;
```

**Summary of notification-related tables:**

| Table                   | Defined in       | Purpose                              |
|-------------------------|------------------|--------------------------------------|
| `pending_notifications` | DD 0.1 §7.13    | Queued notification events           |
| `notification_mutes`    | DD 0.1 §7.12    | Per-ticket mute preferences          |
| `payload` column        | This DD §14.1    | JSON event metadata on pending_notifications |

## 19. Startup Sequence Update

Add a new step to the startup sequence defined in DD 0.8 §10, after step 7 (orphan attachment cleanup):

```
1. Parse CLI args and env vars.
2. Create data directory if it doesn't exist.
3. Open SQLite connection pool.
4. Run pending migrations.
5. Clean stale temp attachment files (per DD 0.5 §10).
6. Start session cleanup background task (per DD 0.3 §9).
7. Start orphan attachment cleanup background task (per DD 0.5 §8).
8. Start notification worker background task (per this DD §10).   ← NEW
9. Build axum router (API routes + static file fallback).
10. Bind to --listen address and serve.
```

The notification worker starts regardless of whether SMTP is configured — it handles cleanup in either case (§10.2).

## 20. Open Questions

1. **SMTP status display in admin panel.** Should the admin panel show whether SMTP is configured and last send status? Recommendation: defer to frontend task 5.17.
2. **Digest mode.** Should users be able to opt into a daily digest instead of per-event emails? Not in v1.
3. **Notification history/audit log.** Should sent notifications be logged for debugging? Not in v1 — rely on tracing logs from the worker.
4. **Reply-To header.** Should emails set a Reply-To that routes back into S9? Not in v1 — inbound email is out of scope (PRD §7.2).
