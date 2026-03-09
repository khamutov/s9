mod auth;
pub mod oidc;
mod ticket;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use sqlx::SqlitePool;

use crate::config::OidcConfig;
use crate::slug::SlugCache;

/// Shared application state threaded into all handlers via Axum's state system.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub oidc: Option<Arc<oidc::OidcProvider>>,
    pub slug_cache: Option<SlugCache>,
}

/// Build the application router with all API routes and static file fallback.
pub fn build_router(
    pool: SqlitePool,
    oidc: Option<Arc<oidc::OidcProvider>>,
    slug_cache: Option<SlugCache>,
) -> Router {
    let state = AppState {
        pool,
        oidc,
        slug_cache,
    };
    build_router_with_state(state)
}

/// Build the router from a pre-constructed [`AppState`] (used by tests).
pub fn build_router_with_state(state: AppState) -> Router {
    let api = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/oidc/authorize", get(oidc::authorize))
        .route("/auth/oidc/callback", get(oidc::callback))
        .route(
            "/tickets",
            get(ticket::list_tickets).post(ticket::create_ticket),
        )
        .route(
            "/tickets/{id}",
            get(ticket::get_ticket).patch(ticket::update_ticket),
        );

    Router::new()
        .nest("/api", api)
        .fallback(crate::embed::static_handler)
        .with_state(state)
}

/// Initialize the OIDC provider from configuration (performs async discovery).
pub async fn init_oidc(config: &OidcConfig) -> anyhow::Result<oidc::OidcProvider> {
    oidc::OidcProvider::discover(config).await
}
