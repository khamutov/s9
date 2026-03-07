mod auth;

use axum::routing::{get, post};
use axum::Router;
use sqlx::SqlitePool;

/// Shared application state threaded into all handlers via Axum's state system.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}

/// Build the application router with all API routes and static file fallback.
pub fn build_router(pool: SqlitePool) -> Router {
    let state = AppState { pool };
    let api = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me));

    Router::new()
        .nest("/api", api)
        .fallback(crate::embed::static_handler)
        .with_state(state)
}
