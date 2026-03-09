mod attachment;
mod auth;
mod comment;
mod component;
pub mod error;
mod events;
mod milestone;
pub mod oidc;
mod ticket;
mod user;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::middleware;
use axum::routing::{get, patch, post};
use sqlx::SqlitePool;
use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};

use crate::auth::middleware::{require_admin, require_auth};
use crate::config::OidcConfig;
use crate::events::EventBus;
use crate::notifications::NotificationProducer;
use crate::slug::SlugCache;

/// Shared application state threaded into all handlers via Axum's state system.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub oidc: Option<Arc<oidc::OidcProvider>>,
    pub slug_cache: Option<SlugCache>,
    pub data_dir: PathBuf,
    pub event_bus: EventBus,
    pub notif_producer: NotificationProducer,
}

/// OpenAPI specification for the S9 Bug Tracker API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "S9 Bug Tracker",
        version = "0.1.0",
        description = "Bug tracker REST API with SSE real-time events."
    ),
    paths(
        // Auth
        auth::login,
        auth::logout,
        auth::me,
        // Tickets
        ticket::list_tickets,
        ticket::get_ticket,
        ticket::create_ticket,
        ticket::update_ticket,
        // Comments
        comment::list_comments,
        comment::create_comment,
        comment::edit_comment,
        comment::delete_comment,
        // Components
        component::list_components,
        component::create_component,
        component::update_component,
        component::delete_component,
        // Milestones
        milestone::list_milestones,
        milestone::create_milestone,
        milestone::update_milestone,
        milestone::delete_milestone,
        // Attachments
        attachment::upload_attachment,
        attachment::download_attachment,
        // Users
        user::list_users,
        user::create_user,
        user::update_user,
        user::set_password,
        // Events
        events::event_stream,
    ),
    components(schemas(
        // Auth
        auth::LoginRequest,
        auth::AuthResponse,
        // Enums
        crate::models::TicketType,
        crate::models::TicketStatus,
        crate::models::Priority,
        crate::models::Role,
        crate::models::MilestoneStatus,
        // Users
        crate::models::CompactUser,
        crate::models::FullUser,
        crate::models::CreateUserRequest,
        crate::models::UpdateUserRequest,
        crate::models::SetPasswordRequest,
        // Components
        crate::models::CompactComponent,
        crate::models::ComponentResponse,
        crate::models::CreateComponentRequest,
        crate::models::UpdateComponentRequest,
        // Tickets
        crate::models::TicketResponse,
        crate::models::CreateTicketRequest,
        crate::models::UpdateTicketRequest,
        crate::models::CursorPage<crate::models::TicketResponse>,
        crate::models::OffsetPage<crate::models::TicketResponse>,
        // Comments
        crate::models::CommentResponse,
        crate::models::CommentEditResponse,
        crate::models::CreateCommentRequest,
        crate::models::EditCommentRequest,
        // Milestones
        crate::models::CompactMilestone,
        crate::models::MilestoneResponse,
        crate::models::MilestoneStats,
        crate::models::CreateMilestoneRequest,
        crate::models::UpdateMilestoneRequest,
        // Attachments
        crate::models::AttachmentResponse,
    )),
    tags(
        (name = "Auth", description = "Authentication and session management"),
        (name = "Tickets", description = "Ticket CRUD and search"),
        (name = "Comments", description = "Comment operations on tickets"),
        (name = "Components", description = "Component tree management"),
        (name = "Milestones", description = "Milestone management"),
        (name = "Attachments", description = "File upload and download"),
        (name = "Users", description = "User management (admin)"),
        (name = "Events", description = "Real-time event stream (SSE)"),
    )
)]
struct ApiDoc;

impl ApiDoc {
    /// Returns the OpenAPI spec with the session cookie security scheme injected.
    fn spec() -> utoipa::openapi::OpenApi {
        let mut doc = Self::openapi();
        let components = doc.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session_cookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("s9_session"))),
        );
        doc
    }
}

/// `GET /api/openapi.json` — serves the generated OpenAPI specification.
async fn openapi_spec() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::spec())
}

/// Build the application router with all API routes and static file fallback.
pub fn build_router(
    pool: SqlitePool,
    oidc: Option<Arc<oidc::OidcProvider>>,
    slug_cache: Option<SlugCache>,
    data_dir: PathBuf,
    event_bus: EventBus,
    notif_producer: NotificationProducer,
) -> Router {
    let state = AppState {
        pool,
        oidc,
        slug_cache,
        data_dir,
        event_bus,
        notif_producer,
    };
    build_router_with_state(state)
}

/// Build the router from a pre-constructed [`AppState`] (used by tests).
pub fn build_router_with_state(state: AppState) -> Router {
    // Public auth endpoints — no session required.
    let public = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/oidc/authorize", get(oidc::authorize))
        .route("/auth/oidc/callback", get(oidc::callback))
        .route("/openapi.json", get(openapi_spec));

    // Admin-only route group — require_admin layer on top of require_auth.
    let admin_only = Router::new()
        .route("/components", post(component::create_component))
        .route(
            "/components/{id}",
            patch(component::update_component).delete(component::delete_component),
        )
        .route("/milestones", post(milestone::create_milestone))
        .route(
            "/milestones/{id}",
            patch(milestone::update_milestone).delete(milestone::delete_milestone),
        )
        .route("/users", get(user::list_users).post(user::create_user))
        .route_layer(middleware::from_fn(require_admin));

    // Authenticated routes — session required but any role.
    let authenticated = Router::new()
        .route("/auth/me", get(auth::me))
        .route(
            "/tickets",
            get(ticket::list_tickets).post(ticket::create_ticket),
        )
        .route(
            "/tickets/{id}",
            get(ticket::get_ticket).patch(ticket::update_ticket),
        )
        .route(
            "/tickets/{id}/comments",
            get(comment::list_comments).post(comment::create_comment),
        )
        .route(
            "/tickets/{id}/comments/{num}",
            patch(comment::edit_comment).delete(comment::delete_comment),
        )
        .route("/components", get(component::list_components))
        .route("/milestones", get(milestone::list_milestones))
        .route("/attachments", post(attachment::upload_attachment))
        .route(
            "/attachments/{id}/{filename}",
            get(attachment::download_attachment),
        )
        .route("/users/{id}", patch(user::update_user))
        .route("/users/{id}/password", post(user::set_password))
        .route("/events", get(events::event_stream))
        .merge(admin_only)
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let api = public.merge(authenticated);

    Router::new()
        .nest("/api", api)
        .fallback(crate::embed::static_handler)
        .with_state(state)
}

/// Initialize the OIDC provider from configuration (performs async discovery).
pub async fn init_oidc(config: &OidcConfig) -> anyhow::Result<oidc::OidcProvider> {
    oidc::OidcProvider::discover(config).await
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::db;
    use crate::events::EventBus;

    use super::*;

    #[tokio::test]
    async fn openapi_spec_is_accessible_without_auth() {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();

        let state = AppState {
            notif_producer: crate::notifications::NotificationProducer::new(
                pool.clone(),
                120,
                false,
            ),
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
            event_bus: EventBus::new(),
        };

        let app = build_router_with_state(state);

        let resp = app
            .oneshot(
                Request::get("/api/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        // Verify basic OpenAPI structure.
        assert_eq!(json["openapi"], "3.1.0");
        assert_eq!(json["info"]["title"], "S9 Bug Tracker");

        // Verify key paths exist.
        assert!(json["paths"]["/api/tickets"].is_object());
        assert!(json["paths"]["/api/auth/login"].is_object());
        assert!(json["paths"]["/api/components"].is_object());
        assert!(json["paths"]["/api/milestones"].is_object());
        assert!(json["paths"]["/api/users"].is_object());
        assert!(json["paths"]["/api/events"].is_object());
        assert!(json["paths"]["/api/attachments"].is_object());

        // Verify security scheme is defined.
        assert!(json["components"]["securitySchemes"]["session_cookie"].is_object());
    }

    #[tokio::test]
    async fn openapi_spec_contains_all_schemas() {
        let spec = ApiDoc::spec();
        let schemas = &spec.components.unwrap().schemas;

        // Spot-check key schemas are present.
        assert!(schemas.contains_key("TicketResponse"));
        assert!(schemas.contains_key("CompactUser"));
        assert!(schemas.contains_key("CommentResponse"));
        assert!(schemas.contains_key("MilestoneResponse"));
        assert!(schemas.contains_key("AttachmentResponse"));
        assert!(schemas.contains_key("FullUser"));
    }
}
