use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::{HeaderValue, SET_COOKIE};
use axum::response::{IntoResponse, Response};
use openidconnect::core::{
    CoreIdTokenClaims, CoreProviderMetadata, CoreResponseType, CoreTokenResponse,
};
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::api::AppState;
use crate::config::OidcConfig;
use crate::models::{CreateUserRequest, Role};
use crate::repos;

/// Max-Age for the session cookie (30 days).
const COOKIE_MAX_AGE: i64 = 30 * 24 * 60 * 60;

/// Max-Age for the OIDC state cookie (10 minutes).
const OIDC_STATE_COOKIE_MAX_AGE: i64 = 10 * 60;

/// Pre-initialized OIDC provider metadata and credentials.
///
/// Constructed once at startup via discovery. Per-request, a fresh client is
/// built from the stored metadata with the appropriate redirect URI.
pub struct OidcProvider {
    metadata: CoreProviderMetadata,
    client_id: ClientId,
    client_secret: ClientSecret,
    pub display_name: String,
}

impl OidcProvider {
    /// Discover OIDC provider metadata and build a reusable provider.
    pub async fn discover(config: &OidcConfig) -> anyhow::Result<Self> {
        let issuer_url = IssuerUrl::new(config.issuer_url.clone())?;
        let http_client = reqwest::Client::new();
        let metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client)
            .await
            .map_err(|e| anyhow::anyhow!("OIDC discovery failed: {e}"))?;

        Ok(Self {
            metadata,
            client_id: ClientId::new(config.client_id.clone()),
            client_secret: ClientSecret::new(config.client_secret.clone()),
            display_name: config.display_name.clone(),
        })
    }
}

/// Build a `Set-Cookie` header value for the session token.
fn session_cookie(token: &str, max_age: i64) -> HeaderValue {
    let value =
        format!("s9_session={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age}");
    HeaderValue::from_str(&value).expect("cookie value is valid ASCII")
}

/// Build a `Set-Cookie` header for the OIDC state+nonce parameter.
fn oidc_state_cookie(state: &str, nonce: &str, max_age: i64) -> HeaderValue {
    let value = format!(
        "s9_oidc_state={state}:{nonce}; HttpOnly; Secure; SameSite=Lax; Path=/api/auth/oidc; Max-Age={max_age}"
    );
    HeaderValue::from_str(&value).expect("cookie value is valid ASCII")
}

/// Parse the `s9_oidc_state` cookie from the request headers, returning `(state, nonce)`.
fn extract_oidc_state_cookie(headers: &axum::http::HeaderMap) -> Option<(String, String)> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .flat_map(|v| v.to_str().ok())
        .flat_map(|s| s.split("; "))
        .find_map(|pair| pair.strip_prefix("s9_oidc_state="))
        .and_then(|val| {
            let (state, nonce) = val.split_once(':')?;
            Some((state.to_string(), nonce.to_string()))
        })
}

/// Derive the callback redirect URI from the request Host header.
fn callback_url(headers: &axum::http::HeaderMap) -> String {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8080");

    // Use plain http for localhost, https otherwise.
    let scheme = if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    format!("{scheme}://{host}/api/auth/oidc/callback")
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: String,
}

/// `GET /api/auth/oidc/authorize` — initiate the OIDC authorization code flow.
///
/// Generates state and nonce parameters, stores them in a short-lived cookie,
/// and redirects the user to the IdP's authorization endpoint.
pub async fn authorize(State(state): State<AppState>, headers: axum::http::HeaderMap) -> Response {
    let provider = match &state.oidc {
        Some(p) => p,
        None => return oidc_not_configured(),
    };

    let redirect_url = RedirectUrl::new(callback_url(&headers)).expect("callback URL is valid");

    let client = openidconnect::core::CoreClient::from_provider_metadata(
        provider.metadata.clone(),
        provider.client_id.clone(),
        Some(provider.client_secret.clone()),
    )
    .set_redirect_uri(redirect_url);

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .url();

    let state_cookie = oidc_state_cookie(
        csrf_token.secret(),
        nonce.secret(),
        OIDC_STATE_COOKIE_MAX_AGE,
    );

    (
        StatusCode::FOUND,
        [
            (SET_COOKIE, state_cookie),
            (
                axum::http::header::LOCATION,
                HeaderValue::from_str(auth_url.as_str()).expect("auth URL is valid header"),
            ),
        ],
    )
        .into_response()
}

/// `GET /api/auth/oidc/callback?code=...&state=...` — handle the IdP callback.
///
/// Validates the state parameter, exchanges the authorization code for tokens,
/// verifies the ID token, provisions or updates the local user, creates a session,
/// and redirects to the application root.
pub async fn callback(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let provider = match &state.oidc {
        Some(p) => p,
        None => return oidc_not_configured(),
    };

    // Verify state parameter matches cookie.
    let (stored_state, stored_nonce) = match extract_oidc_state_cookie(&headers) {
        Some(pair) => pair,
        None => return oidc_error("Missing OIDC state cookie."),
    };

    if query.state != stored_state {
        return oidc_error("OIDC state mismatch.");
    }

    let redirect_url = RedirectUrl::new(callback_url(&headers)).expect("callback URL is valid");

    let client = openidconnect::core::CoreClient::from_provider_metadata(
        provider.metadata.clone(),
        provider.client_id.clone(),
        Some(provider.client_secret.clone()),
    )
    .set_redirect_uri(redirect_url);

    let http_client = reqwest::Client::new();

    // Exchange authorization code for tokens.
    let code_request = match client.exchange_code(AuthorizationCode::new(query.code)) {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("OIDC code exchange setup failed: {e}");
            return oidc_error("Token exchange failed.");
        }
    };

    let token_response: CoreTokenResponse = match code_request.request_async(&http_client).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("OIDC token exchange failed: {e}");
            return oidc_error("Token exchange failed.");
        }
    };

    // Verify the ID token (signature, issuer, audience, expiry, nonce).
    let id_token = match token_response.id_token() {
        Some(t) => t,
        None => return oidc_error("No ID token in response."),
    };

    let nonce = Nonce::new(stored_nonce);
    let verifier = client.id_token_verifier();
    let claims: &CoreIdTokenClaims = match id_token.claims(&verifier, &nonce) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("OIDC ID token verification failed: {e}");
            return oidc_error("ID token verification failed.");
        }
    };

    // Extract claims.
    let sub = claims.subject().as_str();
    let login = claims
        .preferred_username()
        .map(|u| u.as_str())
        .unwrap_or(sub);
    let email = claims.email().map(|e| e.as_str()).unwrap_or("");
    let display_name = claims
        .name()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str())
        .unwrap_or(login);

    // Find or create local user by oidc_sub.
    let user = match provision_user(&state.pool, sub, login, email, display_name).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("OIDC user provisioning failed: {e}");
            return oidc_error("User provisioning failed.");
        }
    };

    if user.is_active == 0 {
        return oidc_error("Account is deactivated.");
    }

    // Create session (same as password login).
    let session = match repos::session::create(&state.pool, user.id).await {
        Ok(s) => s,
        Err(_) => return oidc_error("Session creation failed."),
    };

    // Clear the OIDC state cookie and set the session cookie.
    let clear_state = oidc_state_cookie("", "", 0);
    let set_session = session_cookie(&session.id, COOKIE_MAX_AGE);

    (
        StatusCode::FOUND,
        [
            (SET_COOKIE, clear_state),
            (SET_COOKIE, set_session),
            (axum::http::header::LOCATION, HeaderValue::from_static("/")),
        ],
    )
        .into_response()
}

/// Find a user by OIDC subject, or create a new one. Updates profile fields on re-login.
async fn provision_user(
    pool: &sqlx::SqlitePool,
    sub: &str,
    login: &str,
    email: &str,
    display_name: &str,
) -> Result<crate::models::UserRow, anyhow::Error> {
    if let Some(existing) = repos::user::get_by_oidc_sub(pool, sub).await? {
        // Update profile fields from latest IdP claims.
        let req = crate::models::UpdateUserRequest {
            display_name: Some(display_name.to_string()),
            email: Some(email.to_string()),
            role: None,
            is_active: None,
        };
        let updated = repos::user::update(pool, existing.id, &req).await?;
        return Ok(updated);
    }

    // Create a new OIDC-provisioned user (no password, role=user).
    let req = CreateUserRequest {
        login: login.to_string(),
        display_name: display_name.to_string(),
        email: email.to_string(),
        password: None,
        role: Some(Role::User),
    };
    let user = repos::user::create(pool, &req, None).await?;
    repos::user::set_oidc_sub(pool, user.id, sub).await?;
    // Re-fetch to include oidc_sub.
    Ok(repos::user::get_by_id(pool, user.id).await?.unwrap())
}

fn oidc_not_configured() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "OIDC is not configured."
        })),
    )
        .into_response()
}

fn oidc_error(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "oidc_error",
            "message": message
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use tower::ServiceExt;

    #[test]
    fn parse_oidc_state_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "s9_oidc_state=abc123:nonce456".parse().unwrap(),
        );

        let (state, nonce) = extract_oidc_state_cookie(&headers).unwrap();
        assert_eq!(state, "abc123");
        assert_eq!(nonce, "nonce456");
    }

    #[test]
    fn parse_oidc_state_cookie_missing() {
        let headers = HeaderMap::new();
        assert!(extract_oidc_state_cookie(&headers).is_none());
    }

    #[test]
    fn parse_oidc_state_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "s9_session=deadbeef; s9_oidc_state=state1:nonce1; other=val"
                .parse()
                .unwrap(),
        );

        let (state, nonce) = extract_oidc_state_cookie(&headers).unwrap();
        assert_eq!(state, "state1");
        assert_eq!(nonce, "nonce1");
    }

    #[test]
    fn oidc_state_cookie_roundtrip() {
        let cookie = oidc_state_cookie("mystate", "mynonce", 600);
        let value = cookie.to_str().unwrap();
        assert!(value.starts_with("s9_oidc_state=mystate:mynonce;"));
        assert!(value.contains("HttpOnly"));
        assert!(value.contains("Max-Age=600"));
    }

    #[test]
    fn callback_url_localhost() {
        let mut headers = HeaderMap::new();
        headers.insert(axum::http::header::HOST, "localhost:8080".parse().unwrap());
        assert_eq!(
            callback_url(&headers),
            "http://localhost:8080/api/auth/oidc/callback"
        );
    }

    #[test]
    fn callback_url_production() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::HOST,
            "bugs.example.com".parse().unwrap(),
        );
        assert_eq!(
            callback_url(&headers),
            "https://bugs.example.com/api/auth/oidc/callback"
        );
    }

    #[tokio::test]
    async fn provision_creates_new_user() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        let user = provision_user(&pool, "oidc-sub-1", "jdoe", "j@example.com", "Jane Doe")
            .await
            .unwrap();

        assert_eq!(user.login, "jdoe");
        assert_eq!(user.email, "j@example.com");
        assert_eq!(user.display_name, "Jane Doe");
        assert_eq!(user.oidc_sub.as_deref(), Some("oidc-sub-1"));
        assert!(user.password_hash.is_none());
        assert_eq!(user.role, Role::User);
        assert_eq!(user.is_active, 1);
    }

    #[tokio::test]
    async fn provision_updates_existing_user() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        // First login — creates user.
        let user1 = provision_user(&pool, "oidc-sub-2", "alice", "alice@old.com", "Alice Old")
            .await
            .unwrap();

        // Second login — updates display_name and email from IdP claims.
        let user2 = provision_user(&pool, "oidc-sub-2", "alice", "alice@new.com", "Alice New")
            .await
            .unwrap();

        assert_eq!(user2.id, user1.id);
        assert_eq!(user2.display_name, "Alice New");
        assert_eq!(user2.email, "alice@new.com");
    }

    #[tokio::test]
    async fn authorize_returns_404_when_oidc_disabled() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = crate::api::build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/api/auth/oidc/authorize")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn callback_returns_404_when_oidc_disabled() {
        let pool = crate::db::init_memory_pool().await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        let notif = crate::notifications::NotificationProducer::new(pool.clone(), 120, false);
        let app = crate::api::build_router(
            pool,
            None,
            None,
            std::path::PathBuf::from("/tmp/test"),
            crate::events::EventBus::new(),
            notif,
        );

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/api/auth/oidc/callback?code=abc&state=xyz")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
