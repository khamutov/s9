//! Attachment API endpoints: upload and download.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_SECURITY_POLICY, CONTENT_TYPE,
};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tokio_util::io::ReaderStream;

use crate::auth::middleware::AuthUser;
use crate::models::AttachmentResponse;
use crate::repos;
use crate::storage;

use super::AppState;
use super::error;

/// Query parameters for `GET /api/attachments/:id/:filename`.
#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub download: Option<String>,
}

/// `POST /api/attachments` — upload a file attachment.
///
/// Accepts `multipart/form-data` with a single `file` field. The file is
/// stored content-addressed by SHA-256 and a metadata row is created.
#[utoipa::path(
    post, path = "/api/attachments", tag = "Attachments",
    request_body(content_type = "multipart/form-data", content = String, description = "File upload"),
    responses(
        (status = 201, description = "Attachment uploaded", body = AttachmentResponse),
        (status = 413, description = "File too large"),
        (status = 422, description = "No file or blocked type"),
    ),
    security(("session_cookie" = []))
)]
pub async fn upload_attachment(
    State(state): State<AppState>,
    user: AuthUser,
    mut multipart: axum::extract::Multipart,
) -> Response {
    // Extract the `file` field from the multipart request.
    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return error::validation_error_msg("No file in request.");
        }
        Err(_) => {
            return error::bad_request("Invalid multipart request.");
        }
    };

    let original_name = field
        .file_name()
        .map(storage::sanitize_filename)
        .unwrap_or_else(|| "unnamed".to_string());

    // Read the full file data.
    let data = match field.bytes().await {
        Ok(b) => b,
        Err(_) => {
            return error::bad_request("Failed to read file data.");
        }
    };

    // Store on filesystem (validates MIME, checks size, computes SHA-256).
    let store_result = match storage::store_file(
        &state.data_dir,
        &data,
        &original_name,
        storage::DEFAULT_MAX_FILE_SIZE,
    )
    .await
    {
        Ok(r) => r,
        Err(storage::StorageError::TooLarge { limit, .. }) => {
            let limit_mb = limit / (1024 * 1024);
            return error::payload_too_large(&format!(
                "File exceeds the {limit_mb} MB size limit."
            ));
        }
        Err(storage::StorageError::MimeBlocked(mime)) => {
            return error::validation_error_msg(&format!("File type not allowed: {mime}"));
        }
        Err(storage::StorageError::Io(_)) => return error::internal_error(),
    };

    // Insert DB row.
    let row = match repos::attachment::create(
        &state.pool,
        &store_result.sha256,
        &original_name,
        &store_result.mime_type,
        store_result.size_bytes as i64,
        user.id,
    )
    .await
    {
        Ok(r) => r,
        Err(_) => return error::internal_error(),
    };

    let response = AttachmentResponse::from(&row);
    (StatusCode::CREATED, axum::Json(response)).into_response()
}

/// `GET /api/attachments/:id/:filename` — download an attachment.
///
/// Streams the file with appropriate headers for inline display (images)
/// or download (other types). The `?download=1` query param forces download.
#[utoipa::path(
    get, path = "/api/attachments/{id}/{filename}", tag = "Attachments",
    params(
        ("id" = i64, Path, description = "Attachment ID"),
        ("filename" = String, Path, description = "Original filename"),
        ("download" = Option<String>, Query, description = "Set to 1 to force download"),
    ),
    responses(
        (status = 200, description = "File contents", content_type = "application/octet-stream"),
        (status = 404, description = "Attachment not found"),
    ),
    security(("session_cookie" = []))
)]
pub async fn download_attachment(
    State(state): State<AppState>,
    _user: AuthUser,
    Path((id, filename)): Path<(i64, String)>,
    Query(query): Query<DownloadQuery>,
) -> Response {
    // Look up attachment row.
    let row = match repos::attachment::get_by_id(&state.pool, id).await {
        Ok(Some(r)) => r,
        Ok(None) => return error::not_found("Attachment not found."),
        Err(_) => return error::internal_error(),
    };

    // Verify filename matches original_name (prevents URL guessing).
    if row.original_name != filename {
        return error::not_found("Attachment not found.");
    }

    // Open the file from content-addressed storage.
    let file_path = storage::attachment_path(&state.data_dir, &row.sha256);
    let file = match tokio::fs::File::open(&file_path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                attachment_id = id,
                sha256 = %row.sha256,
                "attachment file missing from disk"
            );
            return error::not_found("Attachment not found.");
        }
        Err(_) => return error::internal_error(),
    };

    // Determine Content-Disposition.
    let force_download = query.download.as_deref() == Some("1");
    let is_image = row.mime_type.starts_with("image/");
    let disposition = if is_image && !force_download {
        "inline".to_string()
    } else {
        format!("attachment; filename=\"{}\"", row.original_name)
    };

    // Build response with streaming body and security headers.
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, &row.mime_type)
        .header(CONTENT_LENGTH, row.size_bytes.to_string())
        .header(CONTENT_DISPOSITION, disposition)
        .header(CONTENT_SECURITY_POLICY, "sandbox")
        .header("X-Content-Type-Options", "nosniff")
        .header(CACHE_CONTROL, "private, immutable, max-age=31536000")
        .body(body)
        .unwrap()
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::api::build_router_with_state;
    use crate::db;
    use crate::models::CreateUserRequest;
    use crate::repos::{session, user};
    use crate::slug::SlugCache;

    async fn setup() -> (AppState, String, tempfile::TempDir) {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();

        let admin_req = CreateUserRequest {
            login: "admin".to_string(),
            display_name: "Admin User".to_string(),
            email: "admin@example.com".to_string(),
            password: None,
            role: Some(crate::models::Role::Admin),
        };
        let admin = user::create(&pool, &admin_req, None).await.unwrap();
        let sess = session::create(&pool, admin.id).await.unwrap();

        let slug_cache = SlugCache::new(&pool).await.unwrap();

        let tmp_dir = tempfile::tempdir().unwrap();
        storage::init_dirs(tmp_dir.path()).await.unwrap();

        let state = AppState {
            notif_producer: crate::notifications::NotificationProducer::new(
                pool.clone(),
                120,
                false,
            ),
            pool,
            oidc: None,
            slug_cache: Some(slug_cache),
            data_dir: tmp_dir.path().to_path_buf(),
            event_bus: crate::events::EventBus::new(),
        };

        (state, sess.id, tmp_dir)
    }

    fn app(state: AppState) -> axum::Router {
        build_router_with_state(state)
    }

    // ── Upload tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn upload_returns_201() {
        let (state, token, _dir) = setup().await;
        let app = app(state);

        let boundary = "----testboundary";
        let body = format!(
            "------testboundary\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             Hello, world!\r\n\
             ------testboundary--\r\n"
        );

        let req = Request::post("/api/attachments")
            .header("Cookie", format!("s9_session={token}"))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["original_name"], "hello.txt");
        assert_eq!(json["mime_type"], "text/plain");
        assert!(json["url"].as_str().unwrap().contains("/api/attachments/"));
        assert!(json["id"].as_i64().is_some());
        assert!(json["size_bytes"].as_i64().unwrap() > 0);
    }

    #[tokio::test]
    async fn upload_no_file_returns_422() {
        let (state, token, _dir) = setup().await;
        let app = app(state);

        let boundary = "----testboundary";
        let body = format!(
            "------testboundary\r\n\
             Content-Disposition: form-data; name=\"other\"\r\n\r\n\
             no file here\r\n\
             ------testboundary--\r\n"
        );

        let req = Request::post("/api/attachments")
            .header("Cookie", format!("s9_session={token}"))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn upload_blocked_extension_returns_422() {
        let (state, token, _dir) = setup().await;
        let app = app(state);

        let boundary = "----testboundary";
        let body = format!(
            "------testboundary\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"malware.exe\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n\
             MZ fake exe\r\n\
             ------testboundary--\r\n"
        );

        let req = Request::post("/api/attachments")
            .header("Cookie", format!("s9_session={token}"))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn upload_unauthenticated_returns_401() {
        let (state, _, _dir) = setup().await;
        let app = app(state);

        let boundary = "----testboundary";
        let body = format!(
            "------testboundary\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             Hello\r\n\
             ------testboundary--\r\n"
        );

        let req = Request::post("/api/attachments")
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ── Download tests ────────────────────────────────────────────

    #[tokio::test]
    async fn download_returns_file() {
        let (state, token, _dir) = setup().await;

        // Store a file and create a DB row.
        let data = b"file content for download";
        let store_result = storage::store_file(
            &state.data_dir,
            data,
            "download.txt",
            storage::DEFAULT_MAX_FILE_SIZE,
        )
        .await
        .unwrap();

        let row = repos::attachment::create(
            &state.pool,
            &store_result.sha256,
            "download.txt",
            &store_result.mime_type,
            store_result.size_bytes as i64,
            1,
        )
        .await
        .unwrap();

        let app = app(state);

        let req = Request::get(format!("/api/attachments/{}/download.txt", row.id))
            .header("Cookie", format!("s9_session={token}"))
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Check security headers.
        assert_eq!(
            resp.headers().get("Content-Security-Policy").unwrap(),
            "sandbox"
        );
        assert_eq!(
            resp.headers().get("X-Content-Type-Options").unwrap(),
            "nosniff"
        );
        assert_eq!(
            resp.headers().get("Cache-Control").unwrap(),
            "private, immutable, max-age=31536000"
        );
        assert_eq!(
            resp.headers().get("Content-Disposition").unwrap(),
            "attachment; filename=\"download.txt\""
        );

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], data);
    }

    #[tokio::test]
    async fn download_image_inline() {
        let (state, token, _dir) = setup().await;

        // Minimal PNG magic bytes.
        let png_data: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];

        let store_result = storage::store_file(
            &state.data_dir,
            png_data,
            "image.png",
            storage::DEFAULT_MAX_FILE_SIZE,
        )
        .await
        .unwrap();

        let row = repos::attachment::create(
            &state.pool,
            &store_result.sha256,
            "image.png",
            &store_result.mime_type,
            store_result.size_bytes as i64,
            1,
        )
        .await
        .unwrap();

        let app = app(state);

        let req = Request::get(format!("/api/attachments/{}/image.png", row.id))
            .header("Cookie", format!("s9_session={token}"))
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get("Content-Disposition").unwrap(), "inline");
    }

    #[tokio::test]
    async fn download_image_force_download() {
        let (state, token, _dir) = setup().await;

        let png_data: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];

        let store_result = storage::store_file(
            &state.data_dir,
            png_data,
            "image2.png",
            storage::DEFAULT_MAX_FILE_SIZE,
        )
        .await
        .unwrap();

        let row = repos::attachment::create(
            &state.pool,
            &store_result.sha256,
            "image2.png",
            &store_result.mime_type,
            store_result.size_bytes as i64,
            1,
        )
        .await
        .unwrap();

        let app = app(state);

        let req = Request::get(format!("/api/attachments/{}/image2.png?download=1", row.id))
            .header("Cookie", format!("s9_session={token}"))
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("Content-Disposition").unwrap(),
            "attachment; filename=\"image2.png\""
        );
    }

    #[tokio::test]
    async fn download_wrong_filename_returns_404() {
        let (state, token, _dir) = setup().await;

        let data = b"some data";
        let store_result = storage::store_file(
            &state.data_dir,
            data,
            "real.txt",
            storage::DEFAULT_MAX_FILE_SIZE,
        )
        .await
        .unwrap();

        let row = repos::attachment::create(
            &state.pool,
            &store_result.sha256,
            "real.txt",
            &store_result.mime_type,
            store_result.size_bytes as i64,
            1,
        )
        .await
        .unwrap();

        let app = app(state);

        let req = Request::get(format!("/api/attachments/{}/wrong.txt", row.id))
            .header("Cookie", format!("s9_session={token}"))
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_nonexistent_id_returns_404() {
        let (state, token, _dir) = setup().await;
        let app = app(state);

        let req = Request::get("/api/attachments/9999/file.txt")
            .header("Cookie", format!("s9_session={token}"))
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_unauthenticated_returns_401() {
        let (state, _, _dir) = setup().await;
        let app = app(state);

        let req = Request::get("/api/attachments/1/file.txt")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
