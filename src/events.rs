use serde::Serialize;
use tokio::sync::broadcast;

/// Maximum number of buffered events before slow receivers start dropping.
const CHANNEL_CAPACITY: usize = 256;

/// Server-sent event types per DD 0.4 §14.2.
///
/// Variants are constructed by event producers (ticket/comment handlers)
/// which will be wired in Phase 4.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
#[allow(dead_code)]
pub enum Event {
    /// A new ticket was created.
    #[serde(rename = "ticket_created")]
    TicketCreated(serde_json::Value),

    /// One or more ticket fields were updated.
    #[serde(rename = "ticket_updated")]
    TicketUpdated(serde_json::Value),

    /// A new comment was posted on a ticket.
    #[serde(rename = "comment_created")]
    CommentCreated(serde_json::Value),

    /// A comment was edited.
    #[serde(rename = "comment_updated")]
    CommentUpdated(serde_json::Value),

    /// A comment was deleted.
    #[serde(rename = "comment_deleted")]
    CommentDeleted(serde_json::Value),
}

impl Event {
    /// Returns the SSE event type name.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TicketCreated(_) => "ticket_created",
            Self::TicketUpdated(_) => "ticket_updated",
            Self::CommentCreated(_) => "comment_created",
            Self::CommentUpdated(_) => "comment_updated",
            Self::CommentDeleted(_) => "comment_deleted",
        }
    }

    /// Returns a reference to the JSON payload.
    pub fn data(&self) -> &serde_json::Value {
        match self {
            Self::TicketCreated(v)
            | Self::TicketUpdated(v)
            | Self::CommentCreated(v)
            | Self::CommentUpdated(v)
            | Self::CommentDeleted(v) => v,
        }
    }
}

/// Shared event bus backed by a tokio broadcast channel.
///
/// Cloning is cheap (Arc internally). Call [`EventBus::send`] to publish
/// events and [`EventBus::subscribe`] to receive them.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    /// Creates a new event bus with the default channel capacity.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    /// Publishes an event to all connected subscribers.
    ///
    /// Returns the number of receivers that got the message.
    /// A return of 0 means no SSE clients are currently connected.
    #[allow(dead_code)]
    pub fn send(&self, event: Event) -> usize {
        // `send` returns Err only when there are no receivers, which is fine.
        self.tx.send(event).unwrap_or(0)
    }

    /// Returns a new receiver that will get all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_type_names() {
        let cases = vec![
            (Event::TicketCreated(json!({})), "ticket_created"),
            (Event::TicketUpdated(json!({})), "ticket_updated"),
            (Event::CommentCreated(json!({})), "comment_created"),
            (Event::CommentUpdated(json!({})), "comment_updated"),
            (Event::CommentDeleted(json!({})), "comment_deleted"),
        ];
        for (event, expected) in cases {
            assert_eq!(event.event_type(), expected);
        }
    }

    #[test]
    fn event_data_accessor() {
        let payload = json!({"ticket_id": 42});
        let event = Event::TicketCreated(payload.clone());
        assert_eq!(event.data(), &payload);
    }

    #[tokio::test]
    async fn bus_send_receive() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        let payload = json!({"ticket": {"id": 1, "title": "Test"}});
        bus.send(Event::TicketCreated(payload.clone()));

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type(), "ticket_created");
        assert_eq!(received.data(), &payload);
    }

    #[tokio::test]
    async fn bus_no_receivers_ok() {
        let bus = EventBus::new();
        let count = bus.send(Event::TicketUpdated(json!({})));
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn bus_multiple_receivers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let count = bus.send(Event::CommentCreated(json!({"n": 1})));
        assert_eq!(count, 2);

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert_eq!(e1.event_type(), "comment_created");
        assert_eq!(e2.event_type(), "comment_created");
    }
}
