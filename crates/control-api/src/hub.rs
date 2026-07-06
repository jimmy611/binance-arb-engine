//! The streaming hub: a fan-out of engine events to WebSocket subscribers.
//!
//! The engine publishes typed [`StreamEvent`]s; each connected client holds a
//! broadcast receiver and forwards them as JSON. A single multiplexed topic
//! stream keeps the browser to one socket. The envelope is tagged so the client
//! can route by topic. Because messages are serialized from the same generated
//! protobuf types the gRPC services use, the streaming and RPC contracts cannot
//! drift.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::pb;

/// A tagged streaming event. `topic` selects the payload variant.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "topic", content = "data", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Top-of-book update for a symbol.
    Ticker(pb::BookTop),
    /// A detected opportunity.
    Opportunity(pb::Opportunity),
    /// An executed trade.
    Trade(pb::Trade),
    /// A system metrics snapshot.
    Metrics(pb::SystemMetrics),
    /// A risk or error alert.
    Alert(pb::Alert),
}

/// Fan-out channel from the engine to WebSocket clients.
pub struct Hub {
    tx: broadcast::Sender<StreamEvent>,
}

impl Hub {
    /// Creates a hub with the given per-subscriber buffer capacity. A slow
    /// client that lags beyond the buffer drops intermediate messages (reported
    /// as a lag), which is the right behavior for a live dashboard.
    #[must_use]
    pub fn new(capacity: usize) -> Hub {
        let (tx, _rx) = broadcast::channel(capacity);
        Hub { tx }
    }

    /// Publishes an event to all subscribers. Silently succeeds when there are
    /// no subscribers.
    pub fn publish(&self, event: StreamEvent) {
        let _ = self.tx.send(event);
    }

    /// Subscribes a new client.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.tx.subscribe()
    }

    /// Current number of subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for Hub {
    fn default() -> Hub {
        Hub::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_reaches_subscribers() {
        let hub = Hub::new(16);
        let mut rx = hub.subscribe();
        assert_eq!(hub.subscriber_count(), 1);

        let alert = pb::Alert {
            id: "a1".into(),
            level: "warn".into(),
            source: "risk".into(),
            message: "elevated ghost rate".into(),
            ts: None,
        };
        hub.publish(StreamEvent::Alert(alert.clone()));

        match rx.recv().await.unwrap() {
            StreamEvent::Alert(a) => assert_eq!(a.id, "a1"),
            other => panic!("expected alert, got {other:?}"),
        }
    }

    #[test]
    fn stream_event_serializes_tagged() {
        let event = StreamEvent::Alert(pb::Alert {
            id: "x".into(),
            level: "info".into(),
            source: "sys".into(),
            message: "hi".into(),
            ts: None,
        });
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"topic\":\"alert\""));
        assert!(json.contains("\"data\""));
    }
}
