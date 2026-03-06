# Product Requirements Document: S9 (Sector Nine)

**A minimal, brutal bug tracking system.**

| Field        | Value                  |
|--------------|------------------------|
| Version      | 0.1                    |
| Status       | Draft                  |
| Last updated | 2026-03-06             |

---

## 1. Purpose

Defect is a web-based bug tracking system built for engineers who want to track bugs and features without the overhead of project management tooling. No kanban boards, no sprints, no Gantt charts. One binary, one purpose: know what is broken, who owns it, and when it ships.

## 2. Principles

- **Simplicity over flexibility.** Every feature that can be cut, is cut. Configuration is minimal, opinions are strong.
- **Text-first.** Markdown everywhere. Micro-syntax for linking and referencing. Keyboard-friendly, mouse-optional.
- **Single binary deployment.** The server binary embeds the web frontend. No external services required beyond a database. Ship it, run it, done.
- **No methodology enforcement.** This is not a project management system. There are no workflows, no automations, no custom fields. It tracks defects.

## 3. Core Concepts

### 3.1 Components

Components form a tree hierarchy that replaces the traditional "project" concept. Every ticket belongs to exactly one component.

- Components are organized as an arbitrarily deep tree (e.g. `Platform / Networking / DNS`).
- Each component has a single **owner** (a user).
- Any authenticated user can create tickets and comment in any component regardless of ownership.
- Components can be created and managed by administrators.

### 3.2 Tickets

A ticket represents either a **bug** or a **feature**. There are no other ticket types and no subtasks.

#### Fields

| Field        | Type                              | Required | Notes                                            |
|--------------|-----------------------------------|----------|--------------------------------------------------|
| ID           | Auto-increment integer            | Auto     | Globally unique, monotonically increasing.       |
| Type         | Enum: `bug`, `feature`            | Yes      | Set at creation, can be changed later.           |
| Title        | Plain text, max 256 chars         | Yes      |                                                  |
| Status       | Enum (see below)                  | Yes      | Default: `new`.                                  |
| Priority     | Enum: `P0`–`P5`                   | Yes      | Default: `P3`. `P0` is critical, `P5` is trivial.|
| Owner        | User reference                    | Yes      | The person responsible for resolution.           |
| CC           | List of user references           | No       | Additional people to notify.                     |
| Component    | Component reference               | Yes      | Exactly one.                                     |
| Milestones   | List of milestone references      | No       | A ticket may belong to zero or more milestones.  |
| Estimation   | Duration string                   | No       | Time-based. See Estimation section.              |
| Created at   | Timestamp                         | Auto     |                                                  |
| Updated at   | Timestamp                         | Auto     |                                                  |
| Created by   | User reference                    | Auto     |                                                  |

#### Statuses

Statuses are predefined and not configurable.

| Status        | Meaning                                                     |
|---------------|-------------------------------------------------------------|
| `new`         | Ticket has been filed but work has not started.              |
| `in_progress` | Actively being worked on.                                   |
| `verify`      | Fix or implementation is complete, awaiting verification.    |
| `done`        | Resolved and verified. Terminal state.                      |

Valid transitions: any status may transition to any other status. The system does not enforce a workflow.

#### Estimation

Estimation is optional and time-based. Accepted units:

| Unit | Meaning | Example |
|------|---------|---------|
| `h`  | Hours   | `4h`    |
| `d`  | Days    | `2d`    |
| `w`  | Weeks   | `1w`    |

Stored internally as hours (`1d = 8h`, `1w = 5d = 40h`). Displayed in the most readable unit.

### 3.3 Comments

The description of a ticket is its first comment (comment `#0`). All subsequent comments are replies in a flat, chronological list. There is no threading.

- Comment body is **Markdown** (CommonMark).
- Comments support **file attachments and screenshots** (inline images via drag-and-drop or paste).
- Comments are append-only for regular users. Editing a comment creates a visible edit history. Deletion is administrator-only.
- Each comment has a sequential number within its ticket, starting at `0` (the description).

### 3.4 Milestones

A milestone is a named collection of tickets used for tracking progress toward a goal (e.g. a release, a deadline).

| Field       | Type             | Required | Notes                                    |
|-------------|------------------|----------|------------------------------------------|
| Name        | Plain text       | Yes      | e.g. `v2.4`, `Q3 Hardening`             |
| Description | Markdown         | No       |                                          |
| Due date    | Date             | No       |                                          |
| Status      | `open`, `closed` | Yes      | Default: `open`.                         |

- A ticket can be assigned to multiple milestones.
- A milestone shows aggregate progress: total tickets, tickets by status, total/remaining estimation.

## 4. Micro-syntax

### 4.1 In-comment references

| Syntax         | Resolves to                                           |
|----------------|-------------------------------------------------------|
| `comment#N`    | Link to comment N within the current ticket.          |
| `#ID`          | Link to ticket with the given ID.                     |
| `#ID/comment#N`| Link to comment N in ticket ID.                       |
| `@login`       | Mention user by login. Triggers notification.         |

References are parsed and rendered as hyperlinks. `@login` mentions are validated against existing users, invalid mentions render as plain text.

### 4.2 Search filter syntax

Ticket search and listing uses a Gerrit/Chromium-style filter micro-syntax. Filters are typed as free text in the search bar.

| Filter              | Example                  | Description                                  |
|---------------------|--------------------------|----------------------------------------------|
| `owner:login`       | `owner:alex`             | Tickets owned by user.                       |
| `cc:login`          | `cc:maria`               | Tickets where user is CC'd.                  |
| `status:value`      | `status:new`             | Filter by status.                            |
| `priority:value`    | `priority:P0`            | Filter by priority.                          |
| `type:value`        | `type:bug`               | Filter by ticket type.                       |
| `component:path`    | `component:Platform/DNS` | Filter by component (prefix match on path).  |
| `milestone:name`    | `milestone:v2.4`         | Filter by milestone name.                    |
| `is:open`           | `is:open`                | Tickets not in `done` status.                |
| `is:closed`         | `is:closed`              | Tickets in `done` status.                    |
| `created:range`     | `created:>2026-01-01`    | Filter by creation date.                     |
| `updated:range`     | `updated:<2026-03-01`    | Filter by last update date.                  |
| `estimation:range`  | `estimation:>2d`         | Filter by estimation.                        |
| `has:estimation`    | `has:estimation`         | Tickets with estimation set.                 |
| `has:milestone`     | `has:milestone`          | Tickets assigned to any milestone.           |
| free text           | `crash on startup`       | Full-text search across title and comments.  |

Multiple filters are combined with AND semantics. Use `OR` keyword for disjunction. Use `-` prefix to negate (e.g. `-status:done`). Quoted strings match exact phrases in full-text search.

## 5. Attachments

- Users can attach files and paste screenshots directly into the comment editor.
- Attachments are stored on the server filesystem in a content-addressable layout (SHA-256).
- Supported inline preview: images (PNG, JPEG, GIF, WebP). All other file types render as download links.
- Maximum file size: configurable, default 20 MB per file.
- Attachments are referenced from the Markdown body via standard image/link syntax. The editor inserts the reference automatically on upload.

## 6. Users and Authentication

### 6.1 Internal user management

The system ships with a built-in user store.

- Fields: login (unique), display name, email, password (bcrypt/argon2).
- Administrators can create, deactivate, and manage users.
- Password reset via email (requires email integration to be configured).

### 6.2 External authentication (OIDC)

- The system supports integration with an external identity provider via OpenID Connect.
- When OIDC is configured, users authenticate through the external provider. On first login, a local user record is auto-provisioned from OIDC claims (`sub`, `preferred_username`, `email`, `name`).
- OIDC and internal auth can coexist (configurable).

### 6.3 Roles

Two roles, not configurable:

| Role            | Capabilities                                                                                     |
|-----------------|--------------------------------------------------------------------------------------------------|
| **Administrator** | All user capabilities, plus: manage users, manage components, manage milestones, delete comments, system configuration. |
| **User**          | Create tickets in any component, comment on any ticket, edit own tickets, change ticket status/priority/owner/CC/milestone, upload attachments. |

## 7. Email Integration

Email integration is optional and used exclusively for notifications in v1.

### 7.1 Outbound notifications

The system sends email notifications on the following events:

| Event                     | Recipients                        |
|---------------------------|-----------------------------------|
| New ticket created        | Component owner.                  |
| Comment added             | Ticket owner, CC list, mentioned users (`@login`). |
| Status changed            | Ticket owner, CC list.            |
| Priority changed          | Ticket owner, CC list.            |
| Owner changed             | Old owner, new owner, CC list.    |
| Added to milestone        | Ticket owner.                     |

- Users can mute notifications per-ticket.
- Email is configured via SMTP settings (host, port, TLS, credentials).
- Notifications are batched with a configurable delay (default: 2 minutes) to collapse rapid successive changes into a single email.

### 7.2 Inbound email

Not in scope for v1. Reserved for future consideration (reply-to-comment via email).

## 8. User Interface

### 8.1 Design language

- **Modern flat design**, Swiss-inspired. Minimal chrome, generous whitespace, strong grid.
- **Typography-first.** System font stack: Inter or similar grotesque sans-serif. Monospace for code, IDs, and filter syntax. Clear typographic hierarchy with size and weight, not color.
- **Neutral palette.** Near-white backgrounds, near-black text, single accent color for interactive elements. Priority levels use a restrained color scale (not rainbow).
- **Dense but readable.** Ticket lists are compact tables with sortable columns, not cards. Information density over visual flair.
- **Inspired by Palantir Foundry / Blueprint.js aesthetic**: functional, data-rich, engineered feel.

### 8.2 Key views

| View               | Description                                                                                |
|---------------------|-------------------------------------------------------------------------------------------|
| **Ticket list**     | Filterable, sortable table. Columns: ID, type icon, title, status, priority, owner, component, updated. Filter bar at top with micro-syntax. |
| **Ticket detail**   | Title, metadata sidebar (status, priority, owner, CC, component, milestones, estimation), comment thread below. Inline editing for all metadata fields. |
| **Component tree**  | Sidebar or dedicated view showing hierarchical component structure. Click to filter ticket list by component. |
| **Milestone view**  | List of milestones with progress bars. Drill into milestone to see assigned tickets.       |
| **Admin panel**     | User management, component management, system settings (SMTP, OIDC, file storage).        |

### 8.3 Interaction patterns

- All metadata fields are inline-editable on the ticket detail page (click to edit, Enter to save, Escape to cancel).
- Comment editor is a Markdown textarea with live preview toggle. Supports drag-and-drop and clipboard paste for attachments.
- Filter bar supports autocomplete for filter keys and known values (usernames, component paths, milestone names).
- Keyboard navigation: `j`/`k` for list navigation, `Enter` to open, `c` to create ticket (when not in an input field).

## 9. Tech details

### 9.1 Deployment model

Single statically-linked binary. The compiled React frontend is embedded into the binary at build time (e.g. via `rust-embed` or `include_dir`). Run it, point it at a database, done.

```
s9 --listen 0.0.0.0:8080
```

### 9.2 Technology stack

| Layer     | Technology                                    |
|-----------|-----------------------------------------------|
| Backend   | Rust (axum)                                   |
| Frontend  | React + TypeScript                            |


## 10. Non-goals

To be absolutely explicit about what this system does not do and will not do:

- No kanban boards.
- No scrum, sprints, or velocity tracking.
- No Gantt charts or timeline views.
- No custom fields or custom ticket types.
- No subtasks or task dependencies.
- No mobile or desktop native applications.
- No Git integration (no auto-closing tickets from commits).
- No time tracking (estimation is not tracking).
- No plugins or extension system.
- No multi-tenancy.

## 11. Future Considerations

These are explicitly deferred. They are not planned, but the architecture should not preclude them:

- Inbound email (reply to comment via email).
- Saved searches / personal filters.
- Bulk ticket operations.
- Webhook integrations for external automation.
- Audit log.
- Dark mode.
