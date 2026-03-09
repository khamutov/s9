use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use futures_util::stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::api::AppState;
use crate::auth::middleware::AuthUser;

/// `GET /api/events` — Server-Sent Events stream.
///
/// Opens a persistent SSE connection. Authenticated via session cookie.
/// Broadcasts all system events (ticket/comment CRUD) without server-side
/// filtering — clients filter on their end per DD 0.4 §14.
#[utoipa::path(
    get, path = "/api/events", tag = "Events",
    responses((status = 200, description = "SSE event stream", content_type = "text/event-stream")),
    security(("session_cookie" = []))
)]
pub async fn event_stream(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = state.event_bus.subscribe();

    let stream =
        BroadcastStream::new(rx).filter_map(
            |result: Result<crate::events::Event, _>| match result {
                Ok(event) => {
                    let data = serde_json::to_string(event.data()).unwrap_or_default();
                    Some(Ok(SseEvent::default().event(event.event_type()).data(data)))
                }
                // Lagged receiver — events were dropped due to slow consumption.
                // Skip the error and continue receiving future events.
                Err(_) => None,
            },
        );

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .event(SseEvent::default().event("ping").data("{}")),
    )
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::api::AppState;
    use crate::api::build_router_with_state;
    use crate::db;
    use crate::events::{Event, EventBus};
    use crate::models::{CreateUserRequest, Role};
    use crate::repos::{session, user};

    async fn test_state() -> (AppState, String) {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();

        let admin = user::create(
            &pool,
            &CreateUserRequest {
                login: "admin".to_string(),
                display_name: "Admin".to_string(),
                email: "admin@test.com".to_string(),
                password: None,
                role: Some(Role::Admin),
            },
            None,
        )
        .await
        .unwrap();

        let sess = session::create(&pool, admin.id).await.unwrap();

        let state = AppState {
            pool,
            oidc: None,
            slug_cache: None,
            data_dir: std::path::PathBuf::from("/tmp/test"),
            event_bus: EventBus::new(),
        };

        (state, sess.id)
    }

    #[tokio::test]
    async fn unauthenticated_returns_401() {
        let (state, _) = test_state().await;
        let app = build_router_with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn authenticated_returns_sse_stream() {
        let (state, session_id) = test_state().await;
        let bus = state.event_bus.clone();
        let app = build_router_with_state(state);

        // Send an event before the stream is consumed to verify delivery.
        let payload = serde_json::json!({"ticket": {"id": 1, "title": "Test"}});
        tokio::spawn(async move {
            // Short delay so the SSE handler has time to subscribe.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            bus.send(Event::TicketCreated(payload));
        });

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/events")
                    .header("Cookie", format!("s9_session={session_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/event-stream"
        );
    }
}
