# Design Document: Endpoint Schema

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD               |
| Depends on   | DD 0.1 (Database), DD 0.3 (Auth), API Contract DD |

---

## 1. Context and Scope

The API contract DD chose JSON REST + SSE. DD 0.1 defined the database schema, DD 0.2 the search strategy, and DD 0.3 the full authentication design. This document specifies the exact URL, method, request body, response body, and authorization rules for every endpoint in the system.

This DD unblocks:

- **3.4–3.12** All API handler implementations
- **3.14** OpenAPI spec generation
- **5.4** Frontend API client layer and TypeScript types
- Indirectly, all frontend pages (5.6–5.17)

## 2. Problem Statement

Implementation tasks need precise, copy-paste-friendly endpoint definitions. Without this DD, each implementor would make ad-hoc decisions about URL shapes, field names, error formats, pagination conventions, and response expansion — resulting in an inconsistent API.

Remaining open questions from prior DDs:

- API Contract DD #1: OpenAPI generation strategy.
- API Contract DD #2: SSE event granularity (per-resource vs global stream).
- API Contract DD #3: Pagination style (resolved in DD 0.1 as cursor-based, but free-text search needs offset-based).

## 3. Goals

- Define every endpoint precisely enough for implementation without ambiguity.
- Standardize cross-cutting concerns: errors, pagination, timestamps, user expansion, estimation I/O.
- Resolve all open questions from the API contract DD.

## 4. Non-goals

- Handler implementation details (error handling middleware internals, SQL queries).
- Webhook payloads (PRD §11: deferred).
- Search result snippets (Search DD open question #1: deferred).
- Email notification payloads (deferred to DD 0.6).

## 5. Conventions

All endpoints share these cross-cutting conventions.

### 5.1 Base URL

All endpoints are prefixed with `/api/`. The frontend is served from `/` on the same origin.

### 5.2 Content types

| Context | Content-Type |
|---------|-------------|
| Request/response bodies | `application/json` |
| Attachment upload | `multipart/form-data` |
| Event stream | `text/event-stream` |

### 5.3 Error format

All error responses use a consistent JSON envelope:

```json
{
  "error": "error_code",
  "message": "Human-readable description.",
  "details": {}
}
```

The `details` field is optional and only present for validation errors (422).

Standard HTTP status codes:

| Code | Usage |
|------|-------|
| 400 | Malformed request (bad JSON, invalid query param) |
| 401 | Authentication required or session invalid |
| 403 | Authenticated but insufficient permissions |
| 404 | Resource not found |
| 409 | Conflict (duplicate login, component has children, etc.) |
| 422 | Validation error (missing required field, value out of range) |

Validation error example (422):

```json
{
  "error": "validation_error",
  "message": "Request validation failed.",
  "details": {
    "title": "Title is required.",
    "priority": "Invalid priority. Must be one of: P0, P1, P2, P3, P4, P5."
  }
}
```

### 5.4 Pagination

Two pagination modes, auto-selected based on query content.

**Cursor pagination** — used when no free-text search is present (structured filters only). Stable under concurrent updates.

```json
{
  "items": [...],
  "next_cursor": "MjAyNi0wMy0wNlQxNDozMDowMC4wMDBaLDQy",
  "has_more": true
}
```

- Cursor encodes `(updated_at, id)` as base64url (per DD 0.1 §9).
- Default page size: 50. Maximum: 200.
- Parameters: `?cursor=<token>&page_size=50`

**Offset pagination** — used when free-text search is present (BM25 ranked). No stable cursor for relevance-ordered results.

```json
{
  "items": [...],
  "total": 142,
  "page": 1,
  "page_size": 50
}
```

- Parameters: `?page=1&page_size=50`
- Default page: 1. Default page size: 50. Maximum page size: 200.

### 5.5 Timestamps

RFC 3339 with millisecond precision, UTC. Example: `"2026-03-06T14:30:00.000Z"`.

All `created_at` and `updated_at` fields use this format.

### 5.6 Compact user object

Expanded user references throughout the API use a compact form:

```json
{
  "id": 1,
  "login": "alex",
  "display_name": "Alex Kim"
}
```

This avoids exposing email/role in contexts where they are not needed.

### 5.7 Estimation I/O

**Input:** Duration string — `"4h"`, `"2d"`, `"1w"`. Conversion: `1d = 8h`, `1w = 40h`.

**Output:** Always two fields:

```json
{
  "estimation_hours": 16.0,
  "estimation_display": "2d"
}
```

When no estimation is set, both fields are `null`.

### 5.8 OpenAPI

Endpoint handlers are annotated with `utoipa` macros. The generated spec is served at:

```
GET /api/openapi.json
```

This resolves API Contract DD open question #1: use `utoipa` annotations, generate spec at runtime.

### 5.9 Sorting

The ticket list endpoint supports sorting via query parameters:

- `?sort=updated_at` (default), `created_at`, `priority`, `status`, `id`
- `?order=desc` (default), `asc`

When free-text search is active, sort defaults to `relevance` and the sort/order parameters are ignored.

## 6. Auth Endpoints (Reference)

Auth endpoints are fully specified in DD 0.3. Listed here for completeness — see the referenced section for request/response details.

| Method | Path | Description | DD 0.3 ref |
|--------|------|-------------|------------|
| POST | `/api/auth/login` | Password login, sets session cookie | §9.1 |
| POST | `/api/auth/logout` | Destroys session, clears cookie | §9.2 |
| GET | `/api/auth/me` | Returns current authenticated user | §9.3 |
| GET | `/api/auth/oidc/authorize` | Initiates OIDC authorization code flow | §10.2 |
| GET | `/api/auth/oidc/callback` | OIDC callback, creates session | §10.2 |
| POST | `/api/auth/password-reset/request` | Requests password reset email | §13.2 |
| POST | `/api/auth/password-reset/confirm` | Confirms reset with token + new password | §13.3 |

## 7. Ticket Endpoints

### 7.1 List / search tickets

```
GET /api/tickets
```

**Query parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `q` | string | Filter micro-syntax (see PRD §4.2, Search DD §9). Structured filters and free-text combined. |
| `cursor` | string | Cursor token for next page (cursor pagination mode) |
| `page` | integer | Page number (offset pagination mode, default: 1) |
| `page_size` | integer | Items per page (default: 50, max: 200) |
| `sort` | string | Sort field: `updated_at` (default), `created_at`, `priority`, `status`, `id` |
| `order` | string | Sort direction: `desc` (default), `asc` |

**Pagination mode selection:** If `q` contains free-text terms (anything not matching a `key:value` filter), offset pagination is used with BM25 ranking. Otherwise, cursor pagination is used.

**Response (200) — cursor mode:**

```json
{
  "items": [
    {
      "id": 42,
      "type": "bug",
      "title": "Crash on startup when config is missing",
      "status": "new",
      "priority": "P1",
      "owner": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
      "component": { "id": 5, "name": "DNS", "path": "/Platform/Networking/DNS/" },
      "created_by": { "id": 2, "login": "maria", "display_name": "Maria Chen" },
      "cc": [
        { "id": 3, "login": "bob", "display_name": "Bob Lee" }
      ],
      "milestones": [
        { "id": 1, "name": "v2.4" }
      ],
      "estimation_hours": 16.0,
      "estimation_display": "2d",
      "comment_count": 5,
      "created_at": "2026-03-01T10:00:00.000Z",
      "updated_at": "2026-03-06T14:30:00.000Z"
    }
  ],
  "next_cursor": "MjAyNi0wMy0wNlQxNDozMDowMC4wMDBaLDQy",
  "has_more": true
}
```

**Response (200) — offset mode** (when free-text search is active):

Same `items` shape, but paginated with offset:

```json
{
  "items": [...],
  "total": 142,
  "page": 1,
  "page_size": 50
}
```

**Design decision:** The list includes expanded relations (compact owner, component, created_by, CC list, milestone names, comment_count). This avoids N+1 requests on the frontend. The extra bytes are negligible at bug-tracker scale.

### 7.2 Create ticket

```
POST /api/tickets
```

**Request:**

```json
{
  "type": "bug",
  "title": "Crash on startup when config is missing",
  "owner_id": 1,
  "component_id": 5,
  "priority": "P1",
  "description": "Steps to reproduce:\n1. Delete config.toml\n2. Run s9\n3. Observe segfault",
  "cc": [3, 7],
  "milestones": [1],
  "estimation": "2d"
}
```

| Field | Required | Default | Notes |
|-------|----------|---------|-------|
| `type` | Yes | — | `"bug"` or `"feature"` |
| `title` | Yes | — | Max 256 characters |
| `owner_id` | Yes | — | Must reference an active user |
| `component_id` | Yes | — | Must reference an existing component |
| `priority` | No | `"P3"` | `P0`–`P5` |
| `description` | No | `""` | Becomes comment #0 body. Avoids a second round-trip. |
| `cc` | No | `[]` | Array of user IDs |
| `milestones` | No | `[]` | Array of milestone IDs |
| `estimation` | No | `null` | Duration string: `"4h"`, `"2d"`, `"1w"` |

**Response (201):**

Returns the full ticket object (same shape as list items, plus `description` as `comment_count: 1`).

```json
{
  "id": 42,
  "type": "bug",
  "title": "Crash on startup when config is missing",
  "status": "new",
  "priority": "P1",
  "owner": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
  "component": { "id": 5, "name": "DNS", "path": "/Platform/Networking/DNS/" },
  "created_by": { "id": 2, "login": "maria", "display_name": "Maria Chen" },
  "cc": [
    { "id": 3, "login": "bob", "display_name": "Bob Lee" }
  ],
  "milestones": [
    { "id": 1, "name": "v2.4" }
  ],
  "estimation_hours": 16.0,
  "estimation_display": "2d",
  "comment_count": 1,
  "created_at": "2026-03-06T15:00:00.000Z",
  "updated_at": "2026-03-06T15:00:00.000Z"
}
```

**Errors:**

- 422: Missing required fields, invalid type/priority, referenced user/component/milestone not found.

### 7.3 Get ticket

```
GET /api/tickets/:id
```

**Response (200):**

Same shape as the list item object. Does NOT include comments — those are a separate sub-resource (§8) for independent loading and refreshing.

**Errors:**

- 404: Ticket not found.

### 7.4 Update ticket

```
PATCH /api/tickets/:id
```

**Request:** Only include fields to update. Omitted fields are unchanged.

```json
{
  "title": "Updated title",
  "status": "in_progress",
  "priority": "P0",
  "owner_id": 3,
  "component_id": 7,
  "cc": [1, 5],
  "milestones": [1, 2],
  "estimation": "4h"
}
```

| Field | Notes |
|-------|-------|
| `title` | Only ticket creator or admin can change title. 403 otherwise. |
| `status` | Any authenticated user. |
| `priority` | Any authenticated user. |
| `owner_id` | Any authenticated user. Must be an active user. |
| `component_id` | Any authenticated user. Must be an existing component. |
| `cc` | **Replace-all semantics.** The provided array replaces the entire CC list. |
| `milestones` | **Replace-all semantics.** The provided array replaces all milestone assignments. |
| `estimation` | Duration string, or `null` to clear. |
| `type` | Any authenticated user. `"bug"` or `"feature"`. |

**Design decision:** CC and milestones use replace-all on PATCH. Simpler than delta add/remove operations; the frontend always has the full list available.

**Response (200):**

Returns the updated ticket object (same shape as GET).

**Errors:**

- 403: Title change by non-creator and non-admin.
- 404: Ticket not found.
- 422: Invalid field values, referenced entities not found.

## 8. Comment Endpoints

### 8.1 List comments

```
GET /api/tickets/:id/comments
```

**Query parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `include_edits` | boolean | `false` | Include edit history for each comment |

Returns all comments for the ticket. No pagination — comment counts are bounded in practice (see open question #1).

**Response (200):**

```json
{
  "items": [
    {
      "number": 0,
      "author": { "id": 2, "login": "maria", "display_name": "Maria Chen" },
      "body": "Steps to reproduce:\n1. Delete config.toml\n2. Run s9\n3. Observe segfault",
      "attachments": [
        {
          "id": 1,
          "original_name": "screenshot.png",
          "mime_type": "image/png",
          "size_bytes": 245760,
          "url": "/api/attachments/1/screenshot.png"
        }
      ],
      "edit_count": 0,
      "edits": [],
      "created_at": "2026-03-06T15:00:00.000Z",
      "updated_at": "2026-03-06T15:00:00.000Z"
    },
    {
      "number": 1,
      "author": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
      "body": "Confirmed. The config loader doesn't handle missing files.",
      "attachments": [],
      "edit_count": 1,
      "edits": [
        {
          "old_body": "Confirmed. Investigating.",
          "edited_at": "2026-03-06T16:00:00.000Z"
        }
      ],
      "created_at": "2026-03-06T15:30:00.000Z",
      "updated_at": "2026-03-06T16:00:00.000Z"
    }
  ]
}
```

- `edit_count` is always present.
- `edits` array is only populated when `include_edits=true`. Otherwise it is an empty array.

**Errors:**

- 404: Ticket not found.

### 8.2 Create comment

```
POST /api/tickets/:id/comments
```

**Request:**

```json
{
  "body": "I can reproduce this. See attached log.",
  "attachment_ids": [3, 4]
}
```

| Field | Required | Notes |
|-------|----------|-------|
| `body` | Yes | Markdown text. Must not be empty. |
| `attachment_ids` | No | Array of attachment IDs to link to this comment. Attachments must have been uploaded first (§11). |

The comment `number` is auto-assigned as `max(number) + 1` for the ticket.

Triggers FTS index rebuild for the ticket (Search DD §8).

**Response (201):**

```json
{
  "number": 2,
  "author": { "id": 3, "login": "bob", "display_name": "Bob Lee" },
  "body": "I can reproduce this. See attached log.",
  "attachments": [
    {
      "id": 3,
      "original_name": "app.log",
      "mime_type": "text/plain",
      "size_bytes": 8192,
      "url": "/api/attachments/3/app.log"
    }
  ],
  "edit_count": 0,
  "edits": [],
  "created_at": "2026-03-06T17:00:00.000Z",
  "updated_at": "2026-03-06T17:00:00.000Z"
}
```

**Errors:**

- 404: Ticket not found.
- 422: Empty body, attachment ID not found.

### 8.3 Edit comment

```
PATCH /api/tickets/:id/comments/:num
```

**Authorization:** Comment author or admin. Returns 403 otherwise.

**Request:**

```json
{
  "body": "Updated comment text with more details."
}
```

Creates a `comment_edits` entry with the previous body before updating.

Triggers FTS index rebuild for the ticket.

**Response (200):**

Returns the updated comment object (same shape as list items, without edits array).

**Errors:**

- 403: Not the comment author and not admin.
- 404: Ticket or comment not found.

### 8.4 Delete comment

```
DELETE /api/tickets/:id/comments/:num
```

**Authorization:** Admin only. Returns 403 for non-admin users.

Comment #0 (the ticket description) cannot be deleted — returns 422.

Triggers FTS index rebuild for the ticket.

**Response (204):** No content.

**Errors:**

- 403: Not admin.
- 404: Ticket or comment not found.
- 422: Attempt to delete comment #0.

## 9. Component Endpoints

### 9.1 List components

```
GET /api/components
```

Returns a flat list of all components. The frontend reconstructs the tree from `parent_id`.

**Response (200):**

```json
{
  "items": [
    {
      "id": 1,
      "name": "Platform",
      "parent_id": null,
      "path": "/Platform/",
      "owner": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
      "ticket_count": 42,
      "created_at": "2026-01-15T10:00:00.000Z",
      "updated_at": "2026-01-15T10:00:00.000Z"
    },
    {
      "id": 2,
      "name": "Networking",
      "parent_id": 1,
      "path": "/Platform/Networking/",
      "owner": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
      "ticket_count": 18,
      "created_at": "2026-01-15T10:00:00.000Z",
      "updated_at": "2026-01-15T10:00:00.000Z"
    }
  ]
}
```

### 9.2 Create component

```
POST /api/components
```

**Authorization:** Admin only.

**Request:**

```json
{
  "name": "DNS",
  "parent_id": 2,
  "owner_id": 1
}
```

| Field | Required | Notes |
|-------|----------|-------|
| `name` | Yes | Must be unique among siblings (same parent_id). |
| `parent_id` | No | `null` for root component. Must reference existing component. |
| `owner_id` | Yes | Must reference an active user. |

The `path` is computed server-side from the parent's path + name (per DD 0.1 §7.3).

**Response (201):**

```json
{
  "id": 5,
  "name": "DNS",
  "parent_id": 2,
  "path": "/Platform/Networking/DNS/",
  "owner": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
  "ticket_count": 0,
  "created_at": "2026-03-06T15:00:00.000Z",
  "updated_at": "2026-03-06T15:00:00.000Z"
}
```

**Errors:**

- 403: Not admin.
- 409: Duplicate name under same parent.
- 422: Invalid parent_id or owner_id.

### 9.3 Update component

```
PATCH /api/components/:id
```

**Authorization:** Admin only.

**Request:**

```json
{
  "name": "DNS Resolution",
  "parent_id": 3,
  "owner_id": 2
}
```

Name or parent change triggers path recalculation for this component and all descendants (per DD 0.1 §7.3).

**Response (200):**

Returns the updated component object.

**Errors:**

- 403: Not admin.
- 404: Component not found.
- 409: Duplicate name under new parent. Circular parent reference.
- 422: Invalid parent_id or owner_id.

### 9.4 Delete component

```
DELETE /api/components/:id
```

**Authorization:** Admin only.

Cannot delete a component that has tickets assigned or has child components.

**Response (204):** No content.

**Errors:**

- 403: Not admin.
- 404: Component not found.
- 409: Component has assigned tickets or child components.

## 10. Milestone Endpoints

### 10.1 List milestones

```
GET /api/milestones
```

**Query parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `status` | string | — | Filter by status: `open`, `closed`. Omit to return all. |

**Response (200):**

```json
{
  "items": [
    {
      "id": 1,
      "name": "v2.4",
      "description": "Q1 release",
      "due_date": "2026-04-01",
      "status": "open",
      "stats": {
        "total": 24,
        "new": 8,
        "in_progress": 10,
        "verify": 4,
        "done": 2,
        "estimated_hours": 120.0,
        "remaining_hours": 96.0
      },
      "created_at": "2026-01-01T10:00:00.000Z",
      "updated_at": "2026-03-01T10:00:00.000Z"
    }
  ]
}
```

The `stats` object is computed server-side:

- `total`: count of all assigned tickets.
- `new`, `in_progress`, `verify`, `done`: count per status.
- `estimated_hours`: sum of `estimation_hours` for all assigned tickets (excluding nulls).
- `remaining_hours`: sum of `estimation_hours` for tickets not in `done` status.

### 10.2 Create milestone

```
POST /api/milestones
```

**Authorization:** Admin only.

**Request:**

```json
{
  "name": "v2.4",
  "description": "Q1 release",
  "due_date": "2026-04-01"
}
```

| Field | Required | Default | Notes |
|-------|----------|---------|-------|
| `name` | Yes | — | Unique. |
| `description` | No | `null` | Markdown. |
| `due_date` | No | `null` | ISO 8601 date (`YYYY-MM-DD`). |
| `status` | No | `"open"` | `"open"` or `"closed"`. |

**Response (201):**

Returns the milestone object with `stats` (all zeros for a new milestone).

**Errors:**

- 403: Not admin.
- 409: Duplicate name.
- 422: Invalid date format.

### 10.3 Update milestone

```
PATCH /api/milestones/:id
```

**Authorization:** Admin only.

**Request:** Only include fields to update.

```json
{
  "status": "closed"
}
```

**Response (200):**

Returns the updated milestone object with `stats`.

**Errors:**

- 403: Not admin.
- 404: Milestone not found.
- 409: Duplicate name.

### 10.4 Delete milestone

```
DELETE /api/milestones/:id
```

**Authorization:** Admin only.

Cannot delete a milestone that has tickets assigned.

**Response (204):** No content.

**Errors:**

- 403: Not admin.
- 404: Milestone not found.
- 409: Milestone has assigned tickets.

## 11. Attachment Endpoints

### 11.1 Upload attachment

```
POST /api/attachments
```

**Content-Type:** `multipart/form-data`

**Form fields:**

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `file` | file | Yes | Max 20 MB (configurable). |

The attachment is stored on the filesystem (content-addressed by SHA-256, per DD 0.1 §7.10) and a metadata row is created. The attachment is **not linked to any comment** until referenced via `attachment_ids` in a comment create/edit request.

**Response (201):**

```json
{
  "id": 3,
  "original_name": "screenshot.png",
  "mime_type": "image/png",
  "size_bytes": 245760,
  "url": "/api/attachments/3/screenshot.png"
}
```

**Errors:**

- 413: File exceeds size limit.
- 422: No file in request.

### 11.2 Download attachment

```
GET /api/attachments/:id/:filename
```

The `:filename` segment is the `original_name` — included in the URL for human-readable download links.

**Response headers:**

- Images (`image/*`): `Content-Disposition: inline` — displayed in browser.
- Other types: `Content-Disposition: attachment; filename="original_name"` — triggers download.
- `?download=1` query parameter forces `Content-Disposition: attachment` for all types.

**Response:** Raw file bytes with appropriate `Content-Type`.

**Errors:**

- 404: Attachment not found, or filename doesn't match.

## 12. User Management Endpoints

### 12.1 List users

```
GET /api/users
```

**Authorization:** Admin only.

**Query parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `include_inactive` | boolean | `false` | Include deactivated users |

No pagination — user counts are small for a bug tracker.

**Response (200):**

```json
{
  "items": [
    {
      "id": 1,
      "login": "alex",
      "display_name": "Alex Kim",
      "email": "alex@example.com",
      "role": "admin",
      "is_active": true,
      "has_password": true,
      "has_oidc": false,
      "created_at": "2026-01-01T10:00:00.000Z",
      "updated_at": "2026-03-01T10:00:00.000Z"
    }
  ]
}
```

- `has_password`: whether `password_hash` is set (boolean, never exposes the hash).
- `has_oidc`: whether `oidc_sub` is set.

### 12.2 Create user

```
POST /api/users
```

**Authorization:** Admin only.

**Request:**

```json
{
  "login": "newuser",
  "display_name": "New User",
  "email": "new@example.com",
  "password": "securepassword",
  "role": "user"
}
```

| Field | Required | Default | Notes |
|-------|----------|---------|-------|
| `login` | Yes | — | Unique. |
| `display_name` | Yes | — | |
| `email` | Yes | — | |
| `password` | No | — | Minimum 8 chars. Omit for OIDC-only user. |
| `role` | No | `"user"` | `"admin"` or `"user"`. |

**Response (201):**

Returns the user object (same shape as list items).

**Errors:**

- 403: Not admin.
- 409: Duplicate login.
- 422: Invalid fields, password too short.

### 12.3 Update user

```
PATCH /api/users/:id
```

**Authorization:**

- Admin can change: `role`, `is_active`, `display_name`, `email`.
- Self (non-admin) can change: `display_name`, `email`.

**Request:** Only include fields to update.

```json
{
  "display_name": "Alex K.",
  "email": "alex.k@example.com",
  "role": "admin",
  "is_active": false
}
```

When `is_active` is set to `false`, all sessions for the user are deleted immediately (per DD 0.3 §7.5).

**Response (200):**

Returns the updated user object.

**Errors:**

- 403: Non-admin trying to change role/is_active, or editing another user's profile.
- 404: User not found.
- 422: Invalid fields.

### 12.4 Set password

```
POST /api/users/:id/password
```

**Authorization:**

- Self: requires `current_password` in the request.
- Admin: can set without `current_password`.

**Request (self):**

```json
{
  "current_password": "oldpassword",
  "new_password": "newsecurepassword"
}
```

**Request (admin):**

```json
{
  "new_password": "newsecurepassword"
}
```

Deletes all other sessions for the user (force re-login on other devices).

**Response (204):** No content.

**Errors:**

- 403: Non-admin setting another user's password.
- 401: Incorrect `current_password`.
- 422: New password too short (minimum 8 characters).

## 13. Notification Mute Endpoints

### 13.1 Mute ticket

```
POST /api/tickets/:id/mute
```

Mutes notifications for the current user on this ticket. Idempotent — calling when already muted is a no-op.

**Response (204):** No content.

**Errors:**

- 404: Ticket not found.

### 13.2 Unmute ticket

```
DELETE /api/tickets/:id/mute
```

Unmutes notifications for the current user on this ticket. Idempotent — calling when not muted is a no-op.

**Response (204):** No content.

**Errors:**

- 404: Ticket not found.

## 14. SSE Event Stream

```
GET /api/events
```

Opens a persistent Server-Sent Events connection. Authenticated via session cookie (per DD 0.3 §11.4).

This resolves API Contract DD open question #2: **global stream, no server-side filtering.** The frontend filters events client-side. This keeps the server implementation simple and avoids subscription management.

### 14.1 Connection behavior

- Content-Type: `text/event-stream`
- 30-second keepalive ping (`event: ping`, `data: {}`)
- Automatic reconnection via `EventSource` API (browser-native)
- `Last-Event-ID` header is not supported in v1 — clients may miss events during reconnection

### 14.2 Event types

Each event has `event:` (type) and `data:` (JSON payload).

**`ticket_created`**

```
event: ticket_created
data: {"ticket": {"id": 42, "type": "bug", "title": "Crash on startup", "status": "new", "priority": "P1", "owner": {"id": 1, "login": "alex", "display_name": "Alex Kim"}, "component": {"id": 5, "name": "DNS", "path": "/Platform/Networking/DNS/"}, "created_by": {"id": 2, "login": "maria", "display_name": "Maria Chen"}}}
```

**`ticket_updated`**

Includes only the changed fields plus the ticket ID:

```
event: ticket_updated
data: {"ticket_id": 42, "changed_fields": {"status": "in_progress", "owner": {"id": 3, "login": "bob", "display_name": "Bob Lee"}}, "updated_by": {"id": 1, "login": "alex", "display_name": "Alex Kim"}}
```

**`comment_created`**

```
event: comment_created
data: {"ticket_id": 42, "comment": {"number": 3, "author": {"id": 1, "login": "alex", "display_name": "Alex Kim"}, "body": "Fixed in commit abc123.", "created_at": "2026-03-06T18:00:00.000Z"}}
```

**`comment_updated`**

```
event: comment_updated
data: {"ticket_id": 42, "comment_number": 1, "body": "Updated comment text.", "updated_by": {"id": 1, "login": "alex", "display_name": "Alex Kim"}}
```

**`comment_deleted`**

```
event: comment_deleted
data: {"ticket_id": 42, "comment_number": 5, "deleted_by": {"id": 1, "login": "alex", "display_name": "Alex Kim"}}
```

## 15. Endpoint Summary

| # | Method | Path | Auth | Admin | Description |
|---|--------|------|------|-------|-------------|
| 1 | POST | `/api/auth/login` | No | No | Password login |
| 2 | POST | `/api/auth/logout` | No | No | Destroy session |
| 3 | GET | `/api/auth/me` | Yes | No | Current user info |
| 4 | GET | `/api/auth/oidc/authorize` | No | No | Start OIDC flow |
| 5 | GET | `/api/auth/oidc/callback` | No | No | OIDC callback |
| 6 | POST | `/api/auth/password-reset/request` | No | No | Request password reset |
| 7 | POST | `/api/auth/password-reset/confirm` | No | No | Confirm password reset |
| 8 | GET | `/api/tickets` | Yes | No | List/search tickets |
| 9 | POST | `/api/tickets` | Yes | No | Create ticket |
| 10 | GET | `/api/tickets/:id` | Yes | No | Get ticket |
| 11 | PATCH | `/api/tickets/:id` | Yes | No | Update ticket |
| 12 | GET | `/api/tickets/:id/comments` | Yes | No | List comments |
| 13 | POST | `/api/tickets/:id/comments` | Yes | No | Create comment |
| 14 | PATCH | `/api/tickets/:id/comments/:num` | Yes | No | Edit comment |
| 15 | DELETE | `/api/tickets/:id/comments/:num` | Yes | Yes | Delete comment |
| 16 | GET | `/api/components` | Yes | No | List components |
| 17 | POST | `/api/components` | Yes | Yes | Create component |
| 18 | PATCH | `/api/components/:id` | Yes | Yes | Update component |
| 19 | DELETE | `/api/components/:id` | Yes | Yes | Delete component |
| 20 | GET | `/api/milestones` | Yes | No | List milestones |
| 21 | POST | `/api/milestones` | Yes | Yes | Create milestone |
| 22 | PATCH | `/api/milestones/:id` | Yes | Yes | Update milestone |
| 23 | DELETE | `/api/milestones/:id` | Yes | Yes | Delete milestone |
| 24 | POST | `/api/attachments` | Yes | No | Upload attachment |
| 25 | GET | `/api/attachments/:id/:filename` | Yes | No | Download attachment |
| 26 | GET | `/api/users` | Yes | Yes | List users |
| 27 | POST | `/api/users` | Yes | Yes | Create user |
| 28 | PATCH | `/api/users/:id` | Yes | No* | Update user (self or admin) |
| 29 | POST | `/api/users/:id/password` | Yes | No* | Set password (self or admin) |
| 30 | POST | `/api/tickets/:id/mute` | Yes | No | Mute ticket notifications |
| 31 | DELETE | `/api/tickets/:id/mute` | Yes | No | Unmute ticket notifications |
| 32 | GET | `/api/events` | Yes | No | SSE event stream |
| 33 | GET | `/api/openapi.json` | No | No | OpenAPI spec |

\* Endpoints 28–29 have mixed authorization: self-edit for own profile, admin-only for other users. See §12.3 and §12.4.

## 16. Open Questions

1. **Comment pagination threshold.** Currently all comments are returned unpaginated. If tickets with 100+ comments become common, add cursor pagination. Defer until usage data is available.
2. **Bulk operations.** Bulk status change, bulk reassign, etc. Deferred per PRD §11.
3. **Saved searches.** Persisted filter queries per user. Deferred per PRD §11.
4. **Attachment orphan cleanup.** Attachments uploaded but never linked to a comment. Background job to purge after a TTL. Deferred to DD 0.5 (Attachment Storage).
5. **Search snippets.** Highlighted text showing where the FTS match occurred. Deferred per Search DD open question #1.
