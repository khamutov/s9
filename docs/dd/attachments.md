# Design Document: Attachment Storage

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD §5            |
| Depends on   | DD 0.1 (Database), DD 0.4 (Endpoint Schema) |

---

## 1. Context and Scope

DD 0.1 defined the `attachments` and `comment_attachments` tables (§7.10–7.11) but deferred the filesystem layout to this document. DD 0.4 defined upload/download endpoints (§11) but deferred orphan cleanup to this document. PRD §5 requires content-addressable SHA-256 storage, a configurable 20 MB max file size, and inline preview for images.

This DD decides: filesystem layout, deduplication strategy, orphan cleanup, upload processing flow, MIME type handling, security constraints, storage configuration, backup/restore story, and concurrent upload handling.

It unblocks:

- **2.12** Attachment storage implementation (SHA-256 content-addressed FS)
- **3.10** Attachment upload/download endpoints

## 2. Problem Statement

Two open questions remain from prior DDs:

- **DD 0.1 open question #3:** Should `attachments.sha256` have a UNIQUE constraint to deduplicate identical files?
- **DD 0.4 open question #4:** How are orphan attachments (uploaded but never linked to a comment) cleaned up?

Beyond resolving those, implementors need precise specs for: filesystem layout, upload processing steps, MIME detection, download headers, security hardening, and configuration.

## 3. Goals

- Efficient filesystem layout that scales to tens of thousands of files.
- Disk-level deduplication without wasting storage on duplicate screenshots.
- Automatic cleanup of orphan attachments (uploaded but never linked).
- Path traversal prevention and safe download headers.
- Clean backup/restore story compatible with SQLite `.backup`.

## 4. Non-goals

- S3 or cloud object storage (single-binary deployment only).
- Image thumbnail generation.
- Virus/malware scanning.
- CDN or edge caching.
- Encryption at rest.

## 5. Filesystem Layout

### Option A: Flat directory `[rejected]`

Store all files directly in `{data_dir}/attachments/{sha256}`. Simple, but filesystem performance degrades with thousands of entries in a single directory.

### Option B: Two-level shard `[selected]`

```
{data_dir}/attachments/{sha256[0..2]}/{sha256[2..4]}/{sha256}
```

Example: SHA-256 `abcdef0123456789…` is stored at `attachments/ab/cd/abcdef0123456789…`.

Files are stored without extension; the MIME type and original filename come from the DB. This avoids ambiguity when the same content is uploaded under different names.

Rust helper:

```rust
/// Constructs the filesystem path for a content-addressed attachment.
fn attachment_path(data_dir: &Path, sha256: &str) -> PathBuf {
    data_dir
        .join("attachments")
        .join(&sha256[0..2])
        .join(&sha256[2..4])
        .join(sha256)
}
```

## 6. SHA-256 Deduplication

*Resolves DD 0.1 open question #3.*

### Option A: UNIQUE constraint on sha256 `[rejected]`

A UNIQUE constraint would require merging metadata (original_name, uploader_id) across uploads of the same content. This complicates the data model and loses per-upload provenance.

### Option B: Disk-level dedup, no UNIQUE on sha256 `[selected]`

- Each upload creates its own DB row in `attachments`, preserving the per-upload `original_name`, `uploader_id`, and `created_at`.
- On disk, only one copy exists per unique SHA-256 hash.
- On upload: if the file already exists at the content-addressed path, skip the write; insert the DB row regardless.
- On cleanup: delete a file from disk only when zero DB rows reference that SHA-256.

**Decision: No UNIQUE constraint.** The existing index (`idx_attachments_sha256`) is sufficient for dedup lookups and cleanup queries.

## 7. Orphan Cleanup

*Resolves DD 0.4 open question #4.*

### Option A: Eager cleanup on comment delete `[rejected]`

Deleting attachment files immediately when a comment is deleted does not handle attachments that were uploaded but never linked to any comment (e.g. user uploads then navigates away).

### Option B: Background job with TTL `[selected]`

**Orphan definition:** An attachment row with no corresponding `comment_attachments` entry AND `created_at` older than a configurable TTL (default 24 hours).

**Background task:** A tokio task runs on a configurable interval (default 1 hour). Each run:

1. Find orphan rows:

```sql
SELECT a.id, a.sha256
FROM attachments a
LEFT JOIN comment_attachments ca ON ca.attachment_id = a.id
WHERE ca.comment_id IS NULL
  AND a.created_at < datetime('now', '-24 hours');
```

2. Delete the orphan DB rows.
3. For each distinct SHA-256 from step 1, check if any remaining `attachments` rows reference it. If zero remain, delete the file from disk.

**Comment deletion trigger:** When a comment is deleted (CASCADE removes `comment_attachments` rows), immediately check that comment's attachments and clean up any that are now orphaned past the TTL. This provides faster reclamation for explicitly deleted content.

**Configuration:**

| Key                | Default | Description                              |
|--------------------|---------|------------------------------------------|
| `orphan_ttl`       | `24h`   | Min age before an unlinked attachment is eligible for cleanup |
| `cleanup_interval` | `1h`    | How often the background job runs        |

## 8. Upload Processing Flow

Implements DD 0.4 §11.1 (`POST /api/attachments`).

1. **Authenticate** — verify session cookie (DD 0.3).
2. **Extract multipart** — read `file` field. Return 422 if missing.
3. **Stream to temp file** — write to `{data_dir}/attachments/tmp/{uuid}`. While streaming:
   - Compute SHA-256 incrementally (single pass).
   - Track byte count; abort with 413 if `size_bytes` exceeds the configured limit.
   - Optionally pre-check `Content-Length` header as a fast reject before streaming begins.
4. **Detect MIME type** — see §9.
5. **Place file** — compute final path via `attachment_path()`. If a file already exists at that path (dedup hit), delete the temp file. Otherwise, `rename(2)` temp → final path (atomic, since both are on the same filesystem). Create shard directories as needed.
6. **Insert DB row** — `INSERT INTO attachments (sha256, original_name, mime_type, size_bytes, uploader_id)`.
7. **Return 201** — response body per DD 0.4 §11.1.

Key properties:
- Temp file lives on the same filesystem as the final location, enabling atomic `rename(2)`.
- SHA-256 is computed in a single streaming pass — no re-read of the file.
- The file is fully written and hashed before the DB row is created.

## 9. MIME Type Handling

### Option A: Trust client Content-Type `[rejected]`

The client-provided MIME type is trivially spoofed and unreliable across browsers.

### Option B: Server-side detection `[selected]`

Detection chain (first match wins):

1. **`infer` crate** — detects type from magic bytes (file header). Reliable for images, PDFs, archives.
2. **`mime_guess` crate** — falls back to extension-based detection using the `original_name`.
3. **`application/octet-stream`** — ultimate fallback if neither method matches.

The detected MIME type is stored in the DB and used for `Content-Type` on download.

## 10. Security

### 10.1 Path traversal prevention

The only value used to construct filesystem paths is the SHA-256 hex digest, which is guaranteed to match `[0-9a-f]{64}`. The `original_name` is never used in any filesystem operation.

`original_name` sanitization before DB storage:
- Strip path separator characters (`/`, `\`).
- Strip null bytes and control characters.
- Truncate to 255 characters.

### 10.2 MIME allowlist

Allowed types:

| Category   | Types                                                  |
|------------|--------------------------------------------------------|
| Images     | `image/png`, `image/jpeg`, `image/gif`, `image/webp`, `image/svg+xml` |
| Documents  | `application/pdf`, `text/plain`, `text/csv`, `text/markdown` |
| Archives   | `application/zip`, `application/gzip`                  |

Executable files (`.exe`, `.sh`, `.bat`, `.cmd`, `.msi`, `.dll`, `.so`, `.dylib`) are always blocked regardless of detected MIME type. Reject with 422 and a descriptive error message.

SVG files are served with `Content-Security-Policy: sandbox` to prevent embedded script execution.

### 10.3 Download headers

All download responses include:

| Header                     | Value                                              |
|----------------------------|----------------------------------------------------|
| `Content-Type`             | Detected MIME type from DB                         |
| `Content-Length`           | `size_bytes` from DB                               |
| `Content-Security-Policy`  | `sandbox`                                          |
| `X-Content-Type-Options`   | `nosniff`                                          |
| `Cache-Control`            | `private, immutable, max-age=31536000`             |

Content-addressed files are immutable, so aggressive caching is safe. `private` prevents shared caches from storing authenticated content.

### 10.4 Size enforcement

- **Fast reject:** Check `Content-Length` header before reading the body. If it exceeds the limit, return 413 immediately.
- **Streaming check:** Track bytes while writing to the temp file. Abort and delete the temp file as soon as the limit is exceeded.

## 11. Storage Path Configuration

```
{data_dir}/
  s9.db
  attachments/
    tmp/           # in-progress uploads
    ab/cd/abcdef…  # content-addressed files (two-level shard)
```

Config precedence (highest first):

1. CLI flag: `--data-dir /path`
2. Environment variable: `S9_DATA_DIR`
3. Default: `./data`

**Startup behavior:** Create `{data_dir}/attachments/tmp/` if it does not exist. Delete any leftover files in `tmp/` (interrupted uploads from a previous run).

## 12. Backup and Restore

Recommended backup order:

1. Copy the `attachments/` directory (e.g. `rsync` or `tar`).
2. Back up SQLite via `.backup` command or `VACUUM INTO 'backup.db'`.

This order ensures every SHA-256 referenced in the DB backup exists in the filesystem backup. Orphan files (present on disk but unreferenced in DB) are harmless — the cleanup job will remove them after restore.

Document this procedure in task 6.6 (Deployment documentation).

## 13. Concurrent Upload Handling

**Same-content race condition:** Two users upload the same file simultaneously. Both write separate temp files. Both attempt `rename(2)` to the same content-addressed path. Since `rename(2)` is atomic, the second rename overwrites the first with identical content — this is harmless.

Both uploads insert their own DB rows (no UNIQUE constraint on `sha256`). SQLite's write-ahead log serializes writes automatically.

**Different-content uploads:** Fully independent paths — no contention.

## 14. Download Flow

Implements DD 0.4 §11.2 (`GET /api/attachments/:id/:filename`).

1. Look up the `attachments` row by `id`.
2. Verify `:filename` matches `original_name`. Return 404 if not (prevents URL guessing with wrong filenames).
3. Construct the filesystem path from `sha256` via `attachment_path()`. Open the file. If the file is missing, return 404 and log a warning (indicates data inconsistency).
4. Set response headers per §10.3. Content-Disposition:
   - `image/*` types: `inline` (browser displays the image).
   - All other types: `attachment; filename="original_name"` (browser downloads).
   - `?download=1` query parameter forces `attachment` disposition for all types.
5. Stream the file body.

## 15. Open Questions

1. **Image thumbnails.** Generating resized previews for the ticket list. Deferred — not in v1.
2. **Antivirus scanning.** Server-side malware detection on upload. Not in v1.
3. **Storage quotas.** Per-user or global disk usage limits. Not in v1; the 20 MB per-file limit is sufficient for initial deployment.
