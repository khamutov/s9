//! Unified API error handling (DD 0.4 §5.3).
//!
//! All error responses share a consistent JSON envelope:
//! ```json
//! { "error": "<code>", "message": "<human-readable>", "details": {} }
//! ```
//! The `details` field is only present for validation errors (422).

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Unified API error type implementing `IntoResponse`.
///
/// Handlers return `AppError::into_response()` for all failure paths.
/// Variants map directly to HTTP status codes and the standard JSON envelope.
#[derive(Debug)]
pub enum AppError {
    /// 404 — resource not found.
    NotFound(String),
    /// 403 — authenticated but insufficient permissions.
    Forbidden(String),
    /// 409 — conflict (duplicate, integrity constraint).
    Conflict(String),
    /// 422 — validation error with per-field details.
    Validation { field: String, message: String },
    /// 422 — validation error without field details.
    ValidationMsg(String),
    /// 400 — malformed request.
    BadRequest(String),
    /// 413 — payload too large.
    PayloadTooLarge(String),
    /// 401 — authentication required or invalid credentials.
    Unauthorized(String),
    /// 500 — unrecoverable internal error.
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "not_found", "message": msg })),
            ),
            Self::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "forbidden", "message": msg })),
            ),
            Self::Conflict(msg) => (
                StatusCode::CONFLICT,
                Json(json!({ "error": "conflict", "message": msg })),
            ),
            Self::Validation { field, message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": "validation_error",
                    "message": "Request validation failed.",
                    "details": { field: message },
                })),
            ),
            Self::ValidationMsg(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": "validation_error", "message": msg })),
            ),
            Self::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "bad_request", "message": msg })),
            ),
            Self::PayloadTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({ "error": "payload_too_large", "message": msg })),
            ),
            Self::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "unauthorized", "message": msg })),
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "internal_error",
                    "message": "An internal error occurred.",
                })),
            ),
        }
        .into_response()
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors — keep handler call sites concise.
// ---------------------------------------------------------------------------

/// 404 Not Found.
pub fn not_found(message: &str) -> Response {
    AppError::NotFound(message.to_string()).into_response()
}

/// 403 Forbidden.
pub fn forbidden(message: &str) -> Response {
    AppError::Forbidden(message.to_string()).into_response()
}

/// 409 Conflict.
pub fn conflict(message: &str) -> Response {
    AppError::Conflict(message.to_string()).into_response()
}

/// 422 Validation error with a single field detail.
pub fn validation_error(field: &str, message: &str) -> Response {
    AppError::Validation {
        field: field.to_string(),
        message: message.to_string(),
    }
    .into_response()
}

/// 422 Validation error without field details.
pub fn validation_error_msg(message: &str) -> Response {
    AppError::ValidationMsg(message.to_string()).into_response()
}

/// 400 Bad Request.
pub fn bad_request(message: &str) -> Response {
    AppError::BadRequest(message.to_string()).into_response()
}

/// 413 Payload Too Large.
pub fn payload_too_large(message: &str) -> Response {
    AppError::PayloadTooLarge(message.to_string()).into_response()
}

/// 401 Unauthorized.
pub fn unauthorized(message: &str) -> Response {
    AppError::Unauthorized(message.to_string()).into_response()
}

/// 500 Internal Server Error.
pub fn internal_error() -> Response {
    AppError::Internal.into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::StatusCode;

    use super::*;

    async fn response_json(resp: Response) -> (StatusCode, serde_json::Value) {
        let status = resp.status();
        let bytes = axum::body::to_bytes(Body::new(resp.into_body()), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn not_found_response() {
        let (status, json) = response_json(not_found("Ticket not found")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "not_found");
        assert_eq!(json["message"], "Ticket not found");
    }

    #[tokio::test]
    async fn forbidden_response() {
        let (status, json) = response_json(forbidden("Admin only")).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["error"], "forbidden");
    }

    #[tokio::test]
    async fn conflict_response() {
        let (status, json) = response_json(conflict("Duplicate name")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"], "conflict");
    }

    #[tokio::test]
    async fn validation_error_with_field() {
        let (status, json) = response_json(validation_error("title", "Required")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["error"], "validation_error");
        assert_eq!(json["details"]["title"], "Required");
    }

    #[tokio::test]
    async fn validation_msg_response() {
        let (status, json) = response_json(validation_error_msg("Bad input")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["error"], "validation_error");
        assert!(json.get("details").is_none());
    }

    #[tokio::test]
    async fn internal_error_response() {
        let (status, json) = response_json(internal_error()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["error"], "internal_error");
    }
}
