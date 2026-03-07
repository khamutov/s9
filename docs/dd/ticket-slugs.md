# Design Document: Ticket Slugs

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-07                   |
| PRD ref      | 2. Ticket Slugs              |
| Depends on   | DD 0.1 (Database), DD 0.4 (Endpoint Schema) |

---

## 1. Context and Scope

Tickets are identified by auto-increment integers (`#23`). This is unambiguous but carries no information about which part of the system the ticket belongs to. Engineers discussing tickets in Slack, commit messages, or code reviews have no immediate context from the ID alone.

Ticket slugs compose a component-derived prefix with the ticket ID (e.g. `MAP-23`) to produce human-readable, contextual identifiers. This document covers the schema change, slug resolution algorithm, API surface changes, and micro-syntax parser updates required to support slugs.

It unblocks:
- **4.6** Micro-syntax reference parsing (slug-prefixed `#MAP-23` pattern)
- **3.6** Ticket API (slug field in responses)
- **3.8** Component API (slug validation on create/update)

## 2. Problem Statement

- Where is the slug stored and how is uniqueness enforced?
- How does a ticket resolve its effective slug without storing it?
- What changes to the component and ticket APIs are needed?
- How does the micro-syntax parser handle the new `#PREFIX-ID` pattern alongside `#ID`?
- What happens to existing data (components without slugs)?

## 3. Goals

- Add an optional `slug` column to the `components` table with a unique partial index.
- Define the inheritance resolution algorithm (walk up `parent_id`).
- Extend the component API to validate slug format and uniqueness on create/update.
- Extend ticket API responses with a computed `slug` field.
- Update micro-syntax parsing rules for `#PREFIX-ID` references.
- Define a migration path for existing deployments.

## 4. Non-goals

- Storing the resolved slug on the ticket row (it's always computed).
- Allowing tickets to override their component's slug.
- URL routing changes (ticket URLs remain `/tickets/:id`).
- Retroactive slug assignment automation (admins set slugs manually).

## 5. Options Considered

### Option A: Store slug on each ticket row `[rejected]`

Add a `slug_prefix` column to the `tickets` table, populated at creation time and updated on component slug change.

**Pros:**
- Direct lookup: `WHERE slug_prefix = 'MAP' AND id = 23`.
- No parent-walking at read time.

**Cons:**
- **Denormalized.** Renaming a component's slug requires updating every ticket under that component tree.
- Adds write amplification to component slug changes.
- Slug consistency depends on application-level cascade logic — easy to get wrong.

### Option B: Compute slug at read time via inheritance `[selected]`

Store `slug` only on the `components` table. Resolve a ticket's effective slug by walking the component tree at query time.

**Pros:**
- **Single source of truth.** A component slug change takes effect immediately for all tickets in its subtree.
- No denormalization, no cascade updates.
- Schema change is minimal (one column, one index).

**Cons:**
- Requires a tree walk per ticket to resolve the slug. Mitigated by caching (§7.2).

## 6. Decision

**Option B — Compute at read time.** The slug lives on the component, not the ticket. The tree walk is cheap (component trees are small and fully cacheable) and avoids the consistency hazards of denormalization.

## 7. Implementation Details

### 7.1 Schema Migration

```sql
ALTER TABLE components ADD COLUMN slug TEXT;

-- Partial unique index: only non-null slugs participate.
CREATE UNIQUE INDEX idx_components_slug ON components(slug) WHERE slug IS NOT NULL;
```

The `slug` column is nullable. Root components (where `parent_id IS NULL`) must have a non-null slug — this invariant is enforced at the application level, not via a CHECK constraint, because SQLite lacks partial CHECK constraints on nullable columns with conditional logic referencing other columns.

### 7.2 Slug Resolution Algorithm

Given a ticket's `component_id`, resolve the effective slug:

```
function resolve_slug(component_id):
    current = get_component(component_id)
    while current is not null:
        if current.slug is not null:
            return current.slug
        current = get_component(current.parent_id)
    // Unreachable if root components always have slugs
    error("no slug found in component ancestry")
```

**Caching strategy:** Components change infrequently. The full component table is loaded into an in-memory map at startup and refreshed on any component mutation. The tree walk then costs zero SQL queries — it's a handful of hashmap lookups.

In practice, component trees rarely exceed a few hundred nodes. The entire table fits comfortably in memory. The cache is invalidated (reloaded) whenever a component is created, updated, or deleted.

### 7.3 Component API Changes

#### Create component (`POST /api/components`)

New optional field in request body:

| Field | Required | Notes |
|-------|----------|-------|
| `slug` | Conditional | **Required** when `parent_id` is null (root component). Optional otherwise. |

Validation:
- Must match `^[A-Z][A-Z0-9]{1,9}$`.
- Must be globally unique (query `idx_components_slug` index).
- Root components (`parent_id` is null) must provide a slug. Return 422 if missing.

#### Update component (`PATCH /api/components/:id`)

New optional field:

| Field | Notes |
|-------|-------|
| `slug` | Same format and uniqueness validation. Setting to `null` is allowed only if the component is not a root. |

If a root component update attempts to set `slug` to null, return 422.

#### Response enrichment

All component responses include the new field:

```json
{
  "id": 5,
  "name": "DNS",
  "parent_id": 3,
  "path": "/Platform/Networking/DNS/",
  "slug": null,
  "effective_slug": "NET",
  "owner": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
  "created_at": "2026-03-07T10:00:00.000Z",
  "updated_at": "2026-03-07T10:00:00.000Z"
}
```

`slug` is the component's own slug (may be null). `effective_slug` is the resolved slug from inheritance (never null).

#### List components (`GET /api/components`)

Each component in the flat list includes both `slug` and `effective_slug`. The frontend can display the effective slug as a badge on each component node.

### 7.4 Ticket API Changes

#### Response enrichment

Ticket responses gain a `slug` field at the top level:

```json
{
  "id": 23,
  "slug": "MAP-23",
  "type": "bug",
  "title": "Tile rendering glitch on zoom level 18",
  "status": "new",
  "priority": "P1",
  "owner": { "id": 1, "login": "alex", "display_name": "Alex Kim" },
  "component": { "id": 5, "name": "Maps", "path": "/Maps/", "slug": "MAP", "effective_slug": "MAP" },
  ...
}
```

The `slug` field is computed server-side: `format!("{}-{}", effective_slug, ticket.id)`. It is not stored and not accepted as input on create/update.

#### Lookup by slug

No new endpoint is needed. The existing `GET /api/tickets/:id` endpoint uses the numeric ID. Slug-based lookup is handled by the frontend or micro-syntax parser:

1. Parse `MAP-23` → prefix `MAP`, id `23`.
2. Fetch ticket 23 via `GET /api/tickets/23`.
3. Verify the ticket's effective slug matches `MAP`. If not, the reference is stale or invalid.

### 7.5 Micro-syntax Parser Changes

The reference parser (task 4.6) must handle two patterns:

| Pattern | Regex | Capture groups | Resolution |
|---------|-------|----------------|------------|
| Slug reference | `#([A-Z][A-Z0-9]+)-(\d+)` | prefix, id | Validate ticket exists and effective slug matches prefix. |
| Numeric reference | `#(\d+)` | id | Look up ticket by ID directly. |

**Parse order:** The slug pattern must be attempted first (it's a longer, more specific match). If it doesn't match, fall back to the numeric pattern.

**Rendering:** Both forms render as hyperlinks to the ticket. The display text uses the slug form when the ticket's slug is available:
- `#MAP-23` → link to ticket 23, display as `MAP-23`
- `#23` → link to ticket 23, display as `MAP-23` (resolved at render time)

### 7.6 Search Filter

A new `slug:` filter is added to the search micro-syntax:

```
slug:MAP          → tickets whose effective component slug is MAP
slug:MAP,NET      → tickets in MAP or NET (comma-separated)
```

Implementation: given a slug prefix, find all component IDs whose effective slug matches, then filter tickets by `component_id IN (...)`.

## 8. Migration Path

For existing deployments upgrading to a version with slug support:

1. **Schema migration** adds the `slug` column (nullable) and the partial unique index.
2. **All existing components have `slug = NULL`.**
3. On first startup after migration, the application logs a warning if any root component lacks a slug: `"Root component '{}' (id={}) has no slug. Set one via PATCH /api/components/{}."`
4. The system functions without slugs — ticket responses show `slug: null` until components are configured. Micro-syntax `#ID` references continue to work.
5. An administrator sets slugs on root components (and optionally on subtree components) via the component API.

No automated slug generation. Slugs are chosen by humans to be meaningful.

## 9. Open Questions

1. **Should slug references validate the prefix?** If a user writes `#FOO-23` but ticket 23's effective slug is `MAP`, should it resolve (with a warning) or fail silently? Current design: resolve to ticket 23 but do not render as a link if the prefix doesn't match, treating it as plain text.
2. **Should moving a ticket to a different component warn about slug change?** The ticket's slug changes on component reassignment. The UI could show a notice. Deferred to frontend implementation (task 5.10).
