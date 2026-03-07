use axum::Router;

/// Build the application router with all API routes and static file fallback.
pub fn build_router() -> Router {
    let api = Router::new();

    Router::new()
        .nest("/api", api)
        .fallback(crate::embed::static_handler)
}
