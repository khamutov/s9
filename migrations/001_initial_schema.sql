-- 001_initial_schema.sql
-- Full S9 relational schema. Tables ordered by FK dependency.

-----------------------------------------------------------------------
-- 1. users
-----------------------------------------------------------------------
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

-----------------------------------------------------------------------
-- 2. sessions
-----------------------------------------------------------------------
CREATE TABLE sessions (
    id         TEXT    PRIMARY KEY,  -- random token (32-byte hex)
    user_id    INTEGER NOT NULL REFERENCES users(id),
    expires_at TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-----------------------------------------------------------------------
-- 3. components (hierarchical tree with materialized path)
-----------------------------------------------------------------------
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

-----------------------------------------------------------------------
-- 4. milestones
-----------------------------------------------------------------------
CREATE TABLE milestones (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    description TEXT,
    due_date    TEXT,   -- ISO 8601 date, nullable
    status      TEXT    NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-----------------------------------------------------------------------
-- 5. tickets
-----------------------------------------------------------------------
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

-- Cursor pagination index
CREATE INDEX idx_tickets_cursor ON tickets(updated_at, id);

-----------------------------------------------------------------------
-- 6. ticket_cc
-----------------------------------------------------------------------
CREATE TABLE ticket_cc (
    ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    user_id   INTEGER NOT NULL REFERENCES users(id),
    PRIMARY KEY (ticket_id, user_id)
);

CREATE INDEX idx_ticket_cc_user_id ON ticket_cc(user_id);

-----------------------------------------------------------------------
-- 7. ticket_milestones
-----------------------------------------------------------------------
CREATE TABLE ticket_milestones (
    ticket_id    INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    milestone_id INTEGER NOT NULL REFERENCES milestones(id) ON DELETE CASCADE,
    PRIMARY KEY (ticket_id, milestone_id)
);

CREATE INDEX idx_ticket_milestones_milestone_id ON ticket_milestones(milestone_id);

-----------------------------------------------------------------------
-- 8. comments
-----------------------------------------------------------------------
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

-----------------------------------------------------------------------
-- 9. comment_edits
-----------------------------------------------------------------------
CREATE TABLE comment_edits (
    id         INTEGER PRIMARY KEY,
    comment_id INTEGER NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    old_body   TEXT    NOT NULL,
    edited_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_comment_edits_comment_id ON comment_edits(comment_id);

-----------------------------------------------------------------------
-- 10. attachments
-----------------------------------------------------------------------
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

-----------------------------------------------------------------------
-- 11. comment_attachments
-----------------------------------------------------------------------
CREATE TABLE comment_attachments (
    comment_id    INTEGER NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    attachment_id INTEGER NOT NULL REFERENCES attachments(id),
    PRIMARY KEY (comment_id, attachment_id)
);

CREATE INDEX idx_comment_attachments_attachment_id ON comment_attachments(attachment_id);

-----------------------------------------------------------------------
-- 12. notification_mutes
-----------------------------------------------------------------------
CREATE TABLE notification_mutes (
    user_id   INTEGER NOT NULL REFERENCES users(id),
    ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, ticket_id)
);

-----------------------------------------------------------------------
-- 13. pending_notifications
-----------------------------------------------------------------------
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

-----------------------------------------------------------------------
-- 14. password_resets (from auth DD §13.1)
-----------------------------------------------------------------------
CREATE TABLE password_resets (
    id         INTEGER PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id),
    token      TEXT    NOT NULL UNIQUE,  -- SHA-256 of actual token
    expires_at TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_password_resets_token ON password_resets(token);
CREATE INDEX idx_password_resets_user_id ON password_resets(user_id);
