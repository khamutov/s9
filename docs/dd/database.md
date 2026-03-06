# Design Document: Database Schema & Storage Engine

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD               |

---

## 1. Context and Scope

S9 is a Rust/axum + React bug tracker shipped as a single embedded binary. The API contract DD chose JSON REST + SSE. This document decides the storage engine, defines the full relational schema, and specifies the migration and pagination strategies. It unblocks:

- **0.2** Full-Text Search DD (needs to know the engine and FTS approach)
- **0.4** Endpoint Schema DD (needs table shapes for request/response design)
- **2.1** DB connection pool (needs engine choice)
- **2.3** Schema migrations (needs the DDL)

## 2. Problem Statement

Before writing any backend code we need to decide:

- Which database engine to use and how to connect to it.
- The full relational schema covering all PRD entities.
- How to represent hierarchical components efficiently.
- The pagination strategy for ticket listings (open question #3 from the API contract DD).
- How migrations are authored, stored, and executed.

## 3. Goals

- Choose a database engine consistent with the "single binary, no external services" deployment model.
- Define CREATE TABLE statements for every PRD entity with correct types, constraints, and indexes.
- Support all filter micro-syntax queries from PRD section 4.2 without full table scans.
- Resolve the pagination open question from the API contract DD.
- Provide a placeholder for FTS5 so the search DD can build on it.

## 4. Non-goals

- Full-text search query design (deferred to DD 0.2).
- Authentication flow and session semantics (deferred to DD 0.3).
- Attachment filesystem layout (deferred to DD 0.5).
- Horizontal scaling or replication.

## 5. Options Considered

### Option A: PostgreSQL `[rejected]`

Run PostgreSQL as an external service.

**Pros:**
- Mature, feature-rich (JSONB, array types, full-text search via tsvector).
- Excellent concurrent write throughput.
- Well-supported by sqlx with compile-time query checking.

**Cons:**
- **Requires a separate process.** Contradicts the PRD principle: "single binary deployment — no external services required beyond a database." Users would need to install, configure, and maintain PostgreSQL.
- Adds operational complexity (backups, upgrades, connection strings, user management).
- Docker-based deployment becomes multi-container (docker-compose or similar).
- Overkill for a single-user or small-team bug tracker with low write volume.

### Option B: SQLite `[selected]`

Embed SQLite via `sqlx::SqlitePool`.

**Pros:**
- **Zero external dependencies.** The database is a single file. Deployment is `./s9 --listen :8080` and nothing else.
- WAL mode provides concurrent reads with a single writer — sufficient for a bug tracker's write volume.
- FTS5 is built into SQLite and covers full-text search needs.
- `sqlx` supports SQLite with compile-time query checking.
- Backup is copying a file. Migration to PostgreSQL later is possible if ever needed.
- Battle-tested in similar tools (Fossil, Redmine-lite, various wikis).

**Cons:**
- Single-writer limits write throughput (irrelevant at bug-tracker scale — hundreds of writes/day, not thousands/second).
- No native array or JSONB types (worked around with join tables).
- Network-attached storage (NFS) is not recommended (local disk only).

## 6. Decision

**Option B — SQLite** with WAL mode, accessed via `sqlx::SqlitePool`.

The PRD says "single binary, no external services required beyond a database." SQLite needs no separate process; the database is a single file co-located with the binary. WAL mode gives concurrent reads alongside a single writer, which is more than sufficient for a bug tracker. FTS5 provides built-in full-text search.

### Connection Configuration

```rust
let pool = SqlitePoolOptions::new()
    .max_connections(8)          // read connections
    .connect("sqlite://s9.db?mode=rwc").await?;

// Enable WAL mode and recommended pragmas
sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await?;
sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;
sqlx::query("PRAGMA busy_timeout = 5000").execute(&pool).await?;
sqlx::query("PRAGMA synchronous = NORMAL").execute(&pool).await?;
```

Write operations use a dedicated single connection (or serialize through application-level locking) to avoid `SQLITE_BUSY` contention on mutations.

## 7. Schema

All timestamps are stored as RFC 3339 TEXT (SQLite has no native datetime type; TEXT is the recommended approach for sqlx compatibility and human readability). IDs are `INTEGER PRIMARY KEY` which is aliased to SQLite's rowid for optimal performance.

### 7.1 users

```sql
CREATE TABLE users (
    id            INTEGER PRIMARY KEY,
    login         TEXT    NOT NULL UNIQUE,
    display_name  TEXT    NOT NULL,
    email         TEXT    NOT NULL,
    password_hash TEXT,                        -- NULL for OIDC-only users
    role          TEXT    NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'user')),
    oidc_sub      TEXT    UNIQUE,              -- external IdP subject identifier
    is_active     INTEGER NOT NULL DEFAULT 1,  -- boolean: 1 active, 0 deactivated
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 7.2 sessions

```sql
CREATE TABLE sessions (
    id         TEXT    PRIMARY KEY,  -- random token (e.g. 32-byte hex)
    user_id    INTEGER NOT NULL REFERENCES users(id),
    expires_at TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
```

### 7.3 components

Components form an arbitrarily deep tree. The schema uses both `parent_id` (for referential integrity) and a materialized `path` (for efficient prefix queries).

```sql
CREATE TABLE components (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    parent_id  INTEGER REFERENCES components(id),
    path       TEXT    NOT NULL UNIQUE,  -- e.g. '/Platform/Networking/DNS/'
    owner_id   INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(parent_id, name)
);

CREATE INDEX idx_components_parent_id ON components(parent_id);
CREATE INDEX idx_components_path ON components(path);
CREATE INDEX idx_components_owner_id ON components(owner_id);
```

**Path convention:** Leading and trailing `/`, e.g. `/Platform/Networking/DNS/`. A root component "Platform" has path `/Platform/`.

**Querying descendants:** `WHERE path LIKE '/Platform/Networking/%'` — the `idx_components_path` index supports this prefix scan.

**Rename/move operation:** Within a single transaction (safe under SQLite's single-writer model):

```sql
-- 1. Update the component's own path
UPDATE components SET path = :new_path, updated_at = :now WHERE id = :id;
-- 2. Batch-update all descendants
UPDATE components
SET path = :new_prefix || substr(path, length(:old_prefix) + 1),
    updated_at = :now
WHERE path LIKE :old_prefix || '%' AND id != :id;
```

### 7.4 tickets

```sql
CREATE TABLE tickets (
    id               INTEGER PRIMARY KEY,
    type             TEXT    NOT NULL CHECK (type IN ('bug', 'feature')),
    title            TEXT    NOT NULL,
    status           TEXT    NOT NULL DEFAULT 'new'
                             CHECK (status IN ('new', 'in_progress', 'verify', 'done')),
    priority         TEXT    NOT NULL DEFAULT 'P3'
                             CHECK (priority IN ('P0', 'P1', 'P2', 'P3', 'P4', 'P5')),
    owner_id         INTEGER NOT NULL REFERENCES users(id),
    component_id     INTEGER NOT NULL REFERENCES components(id),
    estimation_hours REAL,   -- NULL if not estimated; stored in hours
    created_by       INTEGER NOT NULL REFERENCES users(id),
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Filter indexes (PRD 4.2 micro-syntax)
CREATE INDEX idx_tickets_owner_id ON tickets(owner_id);
CREATE INDEX idx_tickets_component_id ON tickets(component_id);
CREATE INDEX idx_tickets_status ON tickets(status);
CREATE INDEX idx_tickets_priority ON tickets(priority);
CREATE INDEX idx_tickets_type ON tickets(type);
CREATE INDEX idx_tickets_created_by ON tickets(created_by);
CREATE INDEX idx_tickets_created_at ON tickets(created_at);

-- Cursor pagination index (section 9)
CREATE INDEX idx_tickets_cursor ON tickets(updated_at, id);
```

### 7.5 ticket_cc

```sql
CREATE TABLE ticket_cc (
    ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    user_id   INTEGER NOT NULL REFERENCES users(id),
    PRIMARY KEY (ticket_id, user_id)
);

CREATE INDEX idx_ticket_cc_user_id ON ticket_cc(user_id);
```

### 7.6 milestones

```sql
CREATE TABLE milestones (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    description TEXT,
    due_date    TEXT,   -- ISO 8601 date, nullable
    status      TEXT    NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 7.7 ticket_milestones

```sql
CREATE TABLE ticket_milestones (
    ticket_id    INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    milestone_id INTEGER NOT NULL REFERENCES milestones(id) ON DELETE CASCADE,
    PRIMARY KEY (ticket_id, milestone_id)
);

CREATE INDEX idx_ticket_milestones_milestone_id ON ticket_milestones(milestone_id);
```

### 7.8 comments

Comment `#0` is the ticket description. Subsequent comments are numbered sequentially.

```sql
CREATE TABLE comments (
    id         INTEGER PRIMARY KEY,
    ticket_id  INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    number     INTEGER NOT NULL,  -- 0 = description, 1+ = replies
    author_id  INTEGER NOT NULL REFERENCES users(id),
    body       TEXT    NOT NULL,  -- Markdown
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(ticket_id, number)
);

CREATE INDEX idx_comments_ticket_id ON comments(ticket_id);
CREATE INDEX idx_comments_author_id ON comments(author_id);
```

**Number assignment:** Within a transaction, `SELECT COALESCE(MAX(number), -1) + 1 FROM comments WHERE ticket_id = ?`. Safe under SQLite's single-writer model — no concurrent inserts can race.

### 7.9 comment_edits

Each edit preserves the previous body for audit trail (PRD: "editing a comment creates a visible edit history").

```sql
CREATE TABLE comment_edits (
    id         INTEGER PRIMARY KEY,
    comment_id INTEGER NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    old_body   TEXT    NOT NULL,
    edited_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_comment_edits_comment_id ON comment_edits(comment_id);
```

### 7.10 attachments

Attachment files are stored on the filesystem in a content-addressable layout (SHA-256). This table stores metadata only. The filesystem layout is defined in DD 0.5.

```sql
CREATE TABLE attachments (
    id            INTEGER PRIMARY KEY,
    sha256        TEXT    NOT NULL,
    original_name TEXT    NOT NULL,
    mime_type     TEXT    NOT NULL,
    size_bytes    INTEGER NOT NULL,
    uploader_id   INTEGER NOT NULL REFERENCES users(id),
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_attachments_sha256 ON attachments(sha256);
```

### 7.11 comment_attachments

Join table linking attachments to comments. An attachment may be referenced by multiple comments (e.g. if the same file is pasted again).

```sql
CREATE TABLE comment_attachments (
    comment_id    INTEGER NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    attachment_id INTEGER NOT NULL REFERENCES attachments(id),
    PRIMARY KEY (comment_id, attachment_id)
);

CREATE INDEX idx_comment_attachments_attachment_id ON comment_attachments(attachment_id);
```

### 7.12 notification_mutes

Per-ticket notification mute (PRD 7.1: "Users can mute notifications per-ticket").

```sql
CREATE TABLE notification_mutes (
    user_id   INTEGER NOT NULL REFERENCES users(id),
    ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, ticket_id)
);
```

### 7.13 pending_notifications

Queue for batching email notifications (PRD 7.1: "batched with a configurable delay, default 2 minutes").

```sql
CREATE TABLE pending_notifications (
    id         INTEGER PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id),
    ticket_id  INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    event_type TEXT    NOT NULL,  -- 'ticket_created', 'comment_added', 'status_changed', etc.
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    send_after TEXT    NOT NULL   -- earliest time this notification may be sent
);

CREATE INDEX idx_pending_notifications_send_after ON pending_notifications(send_after);
CREATE INDEX idx_pending_notifications_user_ticket ON pending_notifications(user_id, ticket_id);
```

### 7.14 FTS5 Virtual Table

Full-text search over ticket titles and comment bodies. One row per ticket: `title` is the ticket title, `body` is all comment bodies concatenated. The full design (sync strategy, query translation, ranking) is specified in DD 0.2 (Full-Text Search).

```sql
CREATE VIRTUAL TABLE tickets_fts USING fts5(
    title,
    body,
    content='',
    contentless_delete=1,
    tokenize='porter unicode61 remove_diacritics 2',
    prefix='2,3'
);
```

The FTS index is managed at the application level (not triggers) because `body` is an aggregate across all comments for a ticket. See DD 0.2 §8 for sync operations.

## 8. Indexing Strategy

Indexes are designed to support every filter in PRD section 4.2 without full table scans:

| PRD filter            | Query pattern                                     | Supporting index                      |
|-----------------------|---------------------------------------------------|---------------------------------------|
| `owner:login`         | JOIN users, WHERE tickets.owner_id = ?             | `idx_tickets_owner_id`               |
| `cc:login`            | JOIN ticket_cc, WHERE ticket_cc.user_id = ?        | `idx_ticket_cc_user_id`              |
| `status:value`        | WHERE status = ?                                   | `idx_tickets_status`                 |
| `priority:value`      | WHERE priority = ?                                 | `idx_tickets_priority`               |
| `type:value`          | WHERE type = ?                                     | `idx_tickets_type`                   |
| `component:path`      | JOIN components, WHERE path LIKE '/Platform/%'     | `idx_components_path`                |
| `milestone:name`      | JOIN ticket_milestones + milestones                | `idx_ticket_milestones_milestone_id` |
| `is:open` / `is:closed` | WHERE status != 'done' / WHERE status = 'done'  | `idx_tickets_status`                 |
| `created:range`       | WHERE created_at > ? / created_at < ?              | `idx_tickets_created_at`             |
| `updated:range`       | WHERE updated_at > ? / updated_at < ?              | `idx_tickets_cursor` (leftmost col)  |
| `estimation:range`    | WHERE estimation_hours > ?                         | Sequential scan (rare filter, small table) |
| `has:estimation`      | WHERE estimation_hours IS NOT NULL                 | Sequential scan (rare filter)        |
| `has:milestone`       | EXISTS (SELECT 1 FROM ticket_milestones ...)       | `ticket_milestones` PK               |
| free text             | FTS5 MATCH query                                   | `tickets_fts` virtual table          |

For `estimation` and `has:estimation` filters: these are uncommon filters on a small table. A partial index can be added later if profiling shows a need.

## 9. Pagination Design

**Decision: Cursor-based pagination.** This resolves open question #3 from the API contract DD.

### Why cursor-based

Offset-based pagination (`?page=2&per_page=50`) breaks when rows are inserted or updated between page fetches — items shift, causing duplicates or gaps. A bug tracker's ticket list is sorted by `updated_at` and changes frequently. Cursor-based pagination is stable under concurrent updates.

### Cursor encoding

The cursor is a composite of `(updated_at, id)`, base64url-encoded:

```
cursor = base64url("2026-03-06T14:30:00.000Z,42")
```

The `id` tiebreaker ensures deterministic ordering when multiple tickets share the same `updated_at`.

### Query pattern

```sql
-- First page (no cursor)
SELECT * FROM tickets
ORDER BY updated_at DESC, id DESC
LIMIT :page_size + 1;  -- fetch one extra to detect "has next page"

-- Subsequent pages (with cursor)
SELECT * FROM tickets
WHERE (updated_at, id) < (:cursor_updated_at, :cursor_id)
ORDER BY updated_at DESC, id DESC
LIMIT :page_size + 1;
```

The `idx_tickets_cursor` index on `(updated_at, id)` makes this an index-only scan for the ordering columns.

### Response shape

```json
{
  "items": [...],
  "next_cursor": "MjAyNi0wMy0wNlQxNDozMDowMC4wMDBaLDQy",
  "has_more": true
}
```

Default page size: 50. Maximum: 200.

## 10. Migration Strategy

### Tool: sqlx migrations

Migrations are embedded SQL files in the `migrations/` directory, executed by sqlx at application startup.

```
migrations/
  001_initial_schema.sql
  002_fts5_setup.sql
  ...
```

sqlx tracks applied migrations in the `_sqlx_migrations` table (created automatically).

### Policy

- **Forward-only.** No down migrations. Rolling back a schema change means deploying a new forward migration that reverses it. This is simpler and safer — down migrations are rarely tested and often wrong.
- **Each migration is a single transaction.** SQLite supports transactional DDL, so a failed migration leaves the database unchanged.
- **Migrations run at startup.** The application checks for pending migrations on boot and applies them before accepting requests. For a single-binary deployment, this eliminates the need for a separate migration command.

### Initial migration

The first migration (`001_initial_schema.sql`) contains all CREATE TABLE and CREATE INDEX statements from section 7, executed in dependency order.

## 11. Open Questions

1. **FTS5 sync strategy.** Should the FTS index be updated via SQLite triggers or application-level inserts? Triggers are simpler but less flexible. Deferred to DD 0.2.
2. ~~**Session storage alternative.**~~ Resolved in DD 0.3 (Auth & Sessions): database-backed sessions selected. Server-side revocation is required for logout and user deactivation; SQLite point-lookups are sub-millisecond. The `sessions` table (§7.2) is used as-is. DD 0.3 also adds a `password_resets` table for the password reset flow.
3. **Attachment deduplication.** Should `attachments.sha256` have a UNIQUE constraint to deduplicate identical files? Deferred to DD 0.5.
