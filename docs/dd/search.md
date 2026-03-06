# Design Document: Full-Text Search

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD, §4.2         |
| Depends on   | DD 0.1 (Database Schema)     |

---

## 1. Context and Scope

The database DD (0.1) chose SQLite with a placeholder FTS5 virtual table. This document specifies the full-text search design: the FTS5 table structure, tokenizer choice, how data is indexed, how the PRD's filter micro-syntax maps to SQL queries, and how results are ranked.

The PRD (§4.2) defines a filter micro-syntax where structured filters (`owner:alex status:new`) and free-text terms (`crash on startup`) can be combined in a single query. Structured filters map to WHERE clauses on the `tickets` table (covered by existing indexes from DD 0.1). This document focuses on the free-text portion and how it integrates with structured filters.

## 2. Problem Statement

- How should free-text search be indexed? Per-ticket or per-comment granularity?
- Which tokenizer to use and whether stemming is needed?
- How does the PRD filter micro-syntax translate to SQL?
- How should free-text results be ranked alongside structured filter results?
- How is the FTS index kept in sync with source data?

## 3. Goals

- Free-text search across ticket titles and all comment bodies, returning tickets (not individual comments).
- Sub-100ms search latency at 10k tickets / 100k comments.
- Support for prefix matching (e.g. `cras` matches "crash"), phrase queries (`"stack overflow"`), and English stemming.
- Integration with structured filters so `owner:alex crash on startup` produces a single efficient query.

## 4. Non-goals

- Fuzzy/typo-tolerant matching (FTS5 does not support edit-distance search; acceptable for v1).
- Non-English stemming or language detection.
- Search over attachment contents (only metadata: original filename).
- Per-comment search results with highlighted snippets (v1 returns tickets, not comments).

## 5. Options Considered

### Option A: Per-comment FTS table `[rejected]`

One FTS5 row per comment. `rowid` = `comment.id`.

**Pros:**
- Incremental: adding a comment inserts one FTS row.
- Can use external content table (`content='comments'`) with triggers for automatic sync.

**Cons:**
- Search returns comment IDs, not ticket IDs. Requires a JOIN + DISTINCT to get tickets, making ranking awkward — BM25 scores are per-comment, but we need per-ticket relevance.
- Title lives in the `tickets` table, not `comments`. Searching titles requires either a second FTS table (complicating ranking) or duplicating the title as a synthetic comment (fragile).

### Option B: Per-ticket FTS table with aggregated body `[selected]`

One FTS5 row per ticket. `rowid` = `ticket.id`. Two columns: `title` (ticket title) and `body` (all comment bodies concatenated, separated by newlines).

**Pros:**
- Search results are already ticket-scoped — no JOIN/DISTINCT needed.
- BM25 ranking with per-column weights is straightforward: `bm25(tickets_fts, 10.0, 1.0)` gives title matches 10x the weight.
- Single FTS table, single query, simple code.

**Cons:**
- Adding or editing a comment requires rebuilding the ticket's FTS entry (delete old, insert new with updated body). This means re-reading all comments for that ticket.
- At typical bug-tracker scale (5–20 comments per ticket), this rebuild is a sub-millisecond operation. Acceptable.

## 6. Decision

**Option B — Per-ticket FTS table with aggregated body.**

The simplicity of ticket-scoped results and straightforward BM25 ranking outweighs the cost of rebuilding on comment changes. At bug-tracker scale, the rebuild cost is negligible.

## 7. FTS5 Table Definition

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

This replaces the placeholder from DD 0.1.

### Configuration rationale

| Setting | Value | Why |
|---------|-------|-----|
| `content=''` | Contentless | Body is aggregated from comments — no single source table to reference. |
| `contentless_delete=1` | Yes | Allows standard DELETE and INSERT OR REPLACE. Requires SQLite ≥ 3.43.0 (2023-08-24). |
| `tokenize` | `porter unicode61 remove_diacritics 2` | Porter stemming wraps unicode61. `remove_diacritics 2` removes diacritics only when the base character is ASCII (e.g. é→e but preserves non-Latin scripts). Stemming means "crashing" matches "crash". |
| `prefix='2,3'` | 2 and 3-char prefix indexes | Speeds up prefix queries (`cras*`). 2-char and 3-char are the most common prefix lengths a user would type before results appear. |

## 8. Index Synchronization

The FTS index is managed at the application level (not triggers), because the `body` column is an aggregate of all comments for a ticket.

### Sync operations

**On ticket creation** (with comment #0):
```sql
INSERT INTO tickets_fts(rowid, title, body) VALUES (:ticket_id, :title, :comment_body);
```

**On comment insert/update/delete or ticket title update:**
```sql
-- Rebuild the FTS entry for the affected ticket
DELETE FROM tickets_fts WHERE rowid = :ticket_id;

INSERT INTO tickets_fts(rowid, title, body)
VALUES (
    :ticket_id,
    (SELECT title FROM tickets WHERE id = :ticket_id),
    (SELECT group_concat(body, X'0A') FROM comments WHERE ticket_id = :ticket_id ORDER BY number)
);
```

These operations run within the same transaction as the source data mutation. SQLite's single-writer model guarantees consistency.

**Full rebuild** (maintenance, after schema changes, or disaster recovery):
```sql
INSERT INTO tickets_fts(tickets_fts) VALUES('delete-all');

INSERT INTO tickets_fts(rowid, title, body)
SELECT
    t.id,
    t.title,
    (SELECT group_concat(c.body, X'0A') FROM comments c WHERE c.ticket_id = t.id ORDER BY c.number)
FROM tickets t;

INSERT INTO tickets_fts(tickets_fts) VALUES('optimize');
```

The final `optimize` merges all internal b-trees into one for maximum query performance after a bulk load.

## 9. Query Translation

The PRD filter micro-syntax combines structured filters and free text. The search parser (task 2.10) will parse the query string into a structured representation. This section defines how each component maps to SQL.

### 9.1 Structured filters → WHERE clauses

These map directly to indexed columns (per DD 0.1 indexing strategy):

| Filter | SQL |
|--------|-----|
| `owner:alex` | `JOIN users u ON t.owner_id = u.id WHERE u.login = 'alex'` |
| `cc:maria` | `EXISTS (SELECT 1 FROM ticket_cc tc JOIN users u ON tc.user_id = u.id WHERE tc.ticket_id = t.id AND u.login = 'maria')` |
| `status:new` | `WHERE t.status = 'new'` |
| `priority:P0` | `WHERE t.priority = 'P0'` |
| `type:bug` | `WHERE t.type = 'bug'` |
| `component:Platform/DNS` | `JOIN components comp ON t.component_id = comp.id WHERE comp.path LIKE '/Platform/DNS/%'` |
| `milestone:v2.4` | `EXISTS (SELECT 1 FROM ticket_milestones tm JOIN milestones m ON tm.milestone_id = m.id WHERE tm.ticket_id = t.id AND m.name = 'v2.4')` |
| `is:open` | `WHERE t.status != 'done'` |
| `is:closed` | `WHERE t.status = 'done'` |
| `created:>2026-01-01` | `WHERE t.created_at > '2026-01-01T00:00:00Z'` |
| `updated:<2026-03-01` | `WHERE t.updated_at < '2026-03-01T00:00:00Z'` |
| `estimation:>2d` | `WHERE t.estimation_hours > 16.0` |
| `has:estimation` | `WHERE t.estimation_hours IS NOT NULL` |
| `has:milestone` | `EXISTS (SELECT 1 FROM ticket_milestones tm WHERE tm.ticket_id = t.id)` |
| `-status:done` | `WHERE t.status != 'done'` (negation prefix) |

### 9.2 Free-text terms → FTS5 MATCH

Unrecognized tokens (not matching `key:value` pattern) are free-text search terms.

| User input | FTS5 MATCH expression |
|------------|----------------------|
| `crash on startup` | `crash on startup` (implicit AND in FTS5) |
| `"stack overflow"` | `"stack overflow"` (phrase query, passed through) |
| `cras` | `cras*` (short terms get prefix wildcard appended) |

The search parser collects all free-text tokens, constructs the FTS5 MATCH expression, and adds it as a filter:

```sql
WHERE t.id IN (
    SELECT rowid FROM tickets_fts WHERE tickets_fts MATCH :fts_query
)
```

### 9.3 Combined query

A query like `owner:alex status:new crash on startup` becomes:

```sql
SELECT t.*, bm25(fts.rank) AS relevance
FROM tickets t
JOIN users u ON t.owner_id = u.id
JOIN tickets_fts fts ON fts.rowid = t.id
WHERE u.login = 'alex'
  AND t.status = 'new'
  AND tickets_fts MATCH 'crash on startup'
ORDER BY bm25(tickets_fts, 10.0, 1.0)
LIMIT :page_size + 1;
```

When no free-text terms are present, the FTS JOIN is omitted and ordering falls back to cursor-based pagination (DD 0.1, §9).

### 9.4 OR and negation

The PRD specifies `OR` for disjunction and `-` prefix for negation:

- `status:new OR status:in_progress` → `WHERE t.status IN ('new', 'in_progress')`
- `-status:done` → `WHERE t.status != 'done'`
- Free-text negation: `-crash` → FTS5 `NOT crash`
- `type:bug OR type:feature` → `WHERE t.type IN ('bug', 'feature')`

The search parser groups OR-connected filters on the same key into `IN (...)` clauses. OR between different keys (e.g. `owner:alex OR status:new`) is parsed as a disjunction of the full sub-expressions.

## 10. Ranking

When free-text search is active, results are ranked by BM25 relevance instead of cursor-based pagination.

```sql
ORDER BY bm25(tickets_fts, 10.0, 1.0)
```

- First argument: the FTS table.
- `10.0`: weight for the `title` column (first column).
- `1.0`: weight for the `body` column (second column).

Title matches are 10x more relevant than body matches. This ensures a ticket titled "crash handler" ranks above a ticket where "crash" appears only in comment #7.

**FTS5 BM25 scores are negative** (more negative = better match). The default ascending ORDER BY puts the best matches first.

When free-text is active, cursor-based pagination is replaced by offset-based pagination for simplicity — relevance-ranked results don't have a stable cursor. This is acceptable because search results are transient and rarely paginated deeply.

## 11. Performance Considerations

### Expected scale

| Metric | Estimate |
|--------|----------|
| Tickets | 10k–100k |
| Comments | 50k–500k |
| Avg comment length | ~200 words |
| FTS index size | ~50–200 MB |

### Benchmarks to validate (task 6.8)

- FTS MATCH query on 10k tickets: target < 20ms.
- FTS rebuild for a single ticket (20 comments): target < 5ms.
- Full index rebuild (10k tickets): target < 30s.
- Combined structured + FTS query: target < 50ms.

### Index maintenance

- The `automerge` default (4) is sufficient for normal write patterns.
- A periodic `INSERT INTO tickets_fts(tickets_fts) VALUES('optimize')` can be run during low-traffic periods if query performance degrades. This is not expected to be necessary at bug-tracker scale.

## 12. Open Questions

1. **Snippet generation.** Should the API return highlighted snippets showing where the match occurred? This requires `detail=full` (the default) and the `snippet()` auxiliary function. Useful for UI but adds query complexity. Recommendation: defer to the endpoint schema DD (0.4) to decide whether the response includes snippets.
2. **Trigram fallback.** If users frequently search for substrings that don't align with word boundaries (e.g. searching for "alloc" to match "deallocator"), a trigram tokenizer would help. This can be added as a secondary FTS table later without changing the primary design. Recommendation: defer, revisit if user feedback indicates a need.
