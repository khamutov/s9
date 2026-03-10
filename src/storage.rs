#![allow(dead_code)]

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::fs;

// ── Error type ─────────────────────────────────────────────────

/// Errors returned by storage operations.
#[derive(Debug)]
pub enum StorageError {
    /// Filesystem I/O error.
    Io(std::io::Error),
    /// The detected MIME type or file extension is not on the allowlist.
    MimeBlocked(String),
    /// The file exceeds the configured size limit.
    TooLarge { limit: u64, actual: u64 },
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "storage I/O error: {e}"),
            Self::MimeBlocked(m) => write!(f, "MIME type blocked: {m}"),
            Self::TooLarge { limit, actual } => {
                write!(f, "file too large: {actual} bytes (limit {limit})")
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

// ── Constants ──────────────────────────────────────────────────

/// Default maximum upload size: 20 MiB.
pub const DEFAULT_MAX_FILE_SIZE: u64 = 20 * 1024 * 1024;

/// MIME types allowed for upload.
const ALLOWED_MIMES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
    "application/pdf",
    "text/plain",
    "text/csv",
    "text/markdown",
    "application/zip",
    "application/gzip",
];

/// File extensions that are always blocked, regardless of detected MIME.
const BLOCKED_EXTENSIONS: &[&str] = &["exe", "sh", "bat", "cmd", "msi", "dll", "so", "dylib"];

// ── StoreResult ────────────────────────────────────────────────

/// Result of a successful file store operation.
#[derive(Debug, Clone)]
pub struct StoreResult {
    /// Hex-encoded SHA-256 digest.
    pub sha256: String,
    /// Detected MIME type.
    pub mime_type: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

// ── Pure functions ─────────────────────────────────────────────

/// Returns the content-addressed path for a given SHA-256 digest.
///
/// Layout: `{data_dir}/attachments/{sha[0..2]}/{sha[2..4]}/{sha}`
pub fn attachment_path(data_dir: &Path, sha256: &str) -> PathBuf {
    data_dir
        .join("attachments")
        .join(&sha256[..2])
        .join(&sha256[2..4])
        .join(sha256)
}

/// Returns the temp directory for in-flight uploads.
pub fn tmp_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("attachments").join("tmp")
}

/// Returns a unique temp file path using a UUID v4 filename.
pub fn tmp_file_path(data_dir: &Path) -> PathBuf {
    tmp_dir(data_dir).join(uuid::Uuid::new_v4().to_string())
}

/// Sanitizes a user-supplied filename: strips path separators, null/control
/// chars, truncates to 255 bytes, falls back to `"unnamed"`.
pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| *c != '/' && *c != '\\' && !c.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return "unnamed".to_string();
    }
    if trimmed.len() > 255 {
        trimmed[..255].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Detects the MIME type for `data`, falling back through magic bytes →
/// extension guess → `application/octet-stream`.
pub fn detect_mime(data: &[u8], original_name: &str) -> String {
    // Try magic-byte detection first.
    if let Some(kind) = infer::get(data) {
        return kind.mime_type().to_string();
    }

    // Fall back to extension-based guess.
    let guess = mime_guess::from_path(original_name).first_raw();
    if let Some(mime) = guess {
        return mime.to_string();
    }

    "application/octet-stream".to_string()
}

/// Validates that the MIME type is on the allowlist and the original filename
/// does not have a blocked extension.
pub fn check_allowed(mime_type: &str, original_name: &str) -> Result<(), StorageError> {
    // Check blocked extensions.
    if let Some(ext) = Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
    {
        let ext_lower = ext.to_lowercase();
        if BLOCKED_EXTENSIONS.contains(&ext_lower.as_str()) {
            return Err(StorageError::MimeBlocked(format!(
                "blocked extension: .{ext_lower}"
            )));
        }
    }

    // Check MIME allowlist.
    if !ALLOWED_MIMES.contains(&mime_type) {
        return Err(StorageError::MimeBlocked(mime_type.to_string()));
    }

    Ok(())
}

// ── Async functions ────────────────────────────────────────────

/// Creates the temp directory and removes any leftover temp files.
pub async fn init_dirs(data_dir: &Path) -> Result<(), StorageError> {
    let tmp = tmp_dir(data_dir);
    fs::create_dir_all(&tmp).await?;

    // Clean up stale temp files.
    let mut entries = fs::read_dir(&tmp).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_file() {
            let _ = fs::remove_file(entry.path()).await;
        }
    }

    Ok(())
}

/// Stores file data to the content-addressed filesystem.
///
/// 1. Writes `data` to a temp file.
/// 2. Computes SHA-256 digest.
/// 3. Detects and validates MIME type.
/// 4. Atomically renames into the shard path (skips if file already exists for dedup).
pub async fn store_file(
    data_dir: &Path,
    data: &[u8],
    original_name: &str,
    max_size: u64,
) -> Result<StoreResult, StorageError> {
    let actual = data.len() as u64;
    if actual > max_size {
        return Err(StorageError::TooLarge {
            limit: max_size,
            actual,
        });
    }

    let mime_type = detect_mime(data, original_name);
    check_allowed(&mime_type, original_name)?;

    // Write to temp file.
    let tmp_path = tmp_file_path(data_dir);
    fs::create_dir_all(tmp_path.parent().unwrap()).await?;
    fs::write(&tmp_path, data).await?;

    // Compute SHA-256.
    let mut hasher = Sha256::new();
    hasher.update(data);
    let sha256 = hex::encode(hasher.finalize());

    // Move to content-addressed location.
    let dest = attachment_path(data_dir, &sha256);
    if !dest.exists() {
        fs::create_dir_all(dest.parent().unwrap()).await?;
        fs::rename(&tmp_path, &dest).await?;
    } else {
        // Dedup: content already on disk, remove temp file.
        let _ = fs::remove_file(&tmp_path).await;
    }

    Ok(StoreResult {
        sha256,
        mime_type,
        size_bytes: actual,
    })
}

/// Deletes a content-addressed file. Idempotent: returns `Ok` if already gone.
pub async fn delete_file(data_dir: &Path, sha256: &str) -> Result<(), StorageError> {
    let path = attachment_path(data_dir, sha256);
    match fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(StorageError::Io(e)),
    }
}

/// Returns `true` if the content-addressed file exists on disk.
pub async fn file_exists(data_dir: &Path, sha256: &str) -> bool {
    attachment_path(data_dir, sha256).exists()
}

// ── Orphan cleanup ─────────────────────────────────────────────

/// Deletes orphaned attachment rows and their unreferenced files.
///
/// 1. Finds attachment rows with no `comment_attachments` link older than `ttl`.
/// 2. Deletes those rows, collecting distinct SHA-256 digests.
/// 3. For each digest, checks if any remaining row references it.
/// 4. If unreferenced, deletes the file from disk.
///
/// Returns the number of orphan rows removed.
pub async fn cleanup_orphans(
    pool: &SqlitePool,
    data_dir: &Path,
    ttl: chrono::Duration,
) -> Result<usize, anyhow::Error> {
    use crate::repos::attachment as att_repo;

    let orphans = att_repo::find_orphans(pool, ttl).await?;
    if orphans.is_empty() {
        return Ok(0);
    }

    let ids: Vec<i64> = orphans.iter().map(|o| o.id).collect();
    let count = ids.len();
    let sha256s = att_repo::delete_orphan_rows(pool, &ids).await?;

    for sha in &sha256s {
        let still_referenced = att_repo::has_references(pool, sha).await?;
        if !still_referenced {
            delete_file(data_dir, sha).await?;
        }
    }

    Ok(count)
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // PNG magic bytes (minimal 1x1 transparent PNG).
    const PNG_MAGIC: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
        0x49, 0x48, 0x44, 0x52, // IHDR
    ];

    // ── attachment_path ────────────────────────────────────────

    #[test]
    fn attachment_path_layout() {
        let dir = Path::new("/data");
        let sha = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let p = attachment_path(dir, sha);
        assert_eq!(
            p,
            PathBuf::from(
                "/data/attachments/ab/cd/abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
            )
        );
    }

    // ── sanitize_filename ──────────────────────────────────────

    #[test]
    fn sanitize_strips_slashes() {
        assert_eq!(sanitize_filename("path/to\\file.txt"), "pathtofile.txt");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize_filename("file\0name\x01.txt"), "filename.txt");
    }

    #[test]
    fn sanitize_truncates_to_255() {
        let long_name = "a".repeat(300);
        let result = sanitize_filename(&long_name);
        assert_eq!(result.len(), 255);
    }

    #[test]
    fn sanitize_empty_returns_unnamed() {
        assert_eq!(sanitize_filename(""), "unnamed");
        assert_eq!(sanitize_filename("/\\"), "unnamed");
        assert_eq!(sanitize_filename("\0\x01"), "unnamed");
    }

    // ── detect_mime ────────────────────────────────────────────

    #[test]
    fn detect_mime_png() {
        let mime = detect_mime(PNG_MAGIC, "image.png");
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn detect_mime_fallback_extension() {
        let mime = detect_mime(b"just,some,csv,data", "data.csv");
        assert_eq!(mime, "text/csv");
    }

    #[test]
    fn detect_mime_fallback_octet_stream() {
        let mime = detect_mime(&[0x00, 0x01, 0x02], "mystery");
        assert_eq!(mime, "application/octet-stream");
    }

    // ── check_allowed ──────────────────────────────────────────

    #[test]
    fn check_allowed_accepts_image() {
        assert!(check_allowed("image/png", "photo.png").is_ok());
    }

    #[test]
    fn check_allowed_rejects_exe() {
        let err = check_allowed("application/octet-stream", "malware.exe").unwrap_err();
        assert!(matches!(err, StorageError::MimeBlocked(_)));
    }

    #[test]
    fn check_allowed_rejects_unknown_mime() {
        let err = check_allowed("application/x-shockwave-flash", "anim.swf").unwrap_err();
        assert!(matches!(err, StorageError::MimeBlocked(_)));
    }

    // ── store_file ─────────────────────────────────────────────

    #[tokio::test]
    async fn store_and_retrieve() {
        let dir = tempdir().unwrap();
        init_dirs(dir.path()).await.unwrap();

        let data = b"hello, world!";
        let result = store_file(dir.path(), data, "hello.txt", DEFAULT_MAX_FILE_SIZE)
            .await
            .unwrap();

        assert_eq!(result.mime_type, "text/plain");
        assert_eq!(result.size_bytes, 13);
        assert!(!result.sha256.is_empty());

        // Verify file on disk.
        let path = attachment_path(dir.path(), &result.sha256);
        assert!(path.exists());
        let on_disk = std::fs::read(&path).unwrap();
        assert_eq!(on_disk, data);
    }

    #[tokio::test]
    async fn store_dedup() {
        let dir = tempdir().unwrap();
        init_dirs(dir.path()).await.unwrap();

        let data = b"duplicate content";
        let r1 = store_file(dir.path(), data, "file1.txt", DEFAULT_MAX_FILE_SIZE)
            .await
            .unwrap();
        let r2 = store_file(dir.path(), data, "file2.txt", DEFAULT_MAX_FILE_SIZE)
            .await
            .unwrap();

        assert_eq!(r1.sha256, r2.sha256);
        // Only one file on disk.
        let path = attachment_path(dir.path(), &r1.sha256);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn store_rejects_too_large() {
        let dir = tempdir().unwrap();
        init_dirs(dir.path()).await.unwrap();

        let data = vec![0u8; 100];
        let err = store_file(dir.path(), &data, "big.txt", 50)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            StorageError::TooLarge {
                limit: 50,
                actual: 100
            }
        ));
    }

    #[tokio::test]
    async fn store_rejects_blocked_mime() {
        let dir = tempdir().unwrap();
        init_dirs(dir.path()).await.unwrap();

        let err = store_file(
            dir.path(),
            b"#!/bin/bash",
            "script.sh",
            DEFAULT_MAX_FILE_SIZE,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, StorageError::MimeBlocked(_)));
    }

    // ── delete_file ────────────────────────────────────────────

    #[tokio::test]
    async fn delete_file_removes() {
        let dir = tempdir().unwrap();
        init_dirs(dir.path()).await.unwrap();

        let result = store_file(dir.path(), b"delete me", "del.txt", DEFAULT_MAX_FILE_SIZE)
            .await
            .unwrap();
        assert!(file_exists(dir.path(), &result.sha256).await);

        delete_file(dir.path(), &result.sha256).await.unwrap();
        assert!(!file_exists(dir.path(), &result.sha256).await);
    }

    #[tokio::test]
    async fn delete_file_idempotent() {
        let dir = tempdir().unwrap();
        delete_file(dir.path(), "nonexistent_hash").await.unwrap();
    }

    // ── init_dirs ──────────────────────────────────────────────

    #[tokio::test]
    async fn init_dirs_creates_and_cleans() {
        let dir = tempdir().unwrap();
        let tmp = tmp_dir(dir.path());

        // First init creates the directory.
        init_dirs(dir.path()).await.unwrap();
        assert!(tmp.exists());

        // Drop a stale file in tmp.
        let stale = tmp.join("stale-upload");
        std::fs::write(&stale, b"leftover").unwrap();
        assert!(stale.exists());

        // Re-init cleans it up.
        init_dirs(dir.path()).await.unwrap();
        assert!(!stale.exists());
    }
}
