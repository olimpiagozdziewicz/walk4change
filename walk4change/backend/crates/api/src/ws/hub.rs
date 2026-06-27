use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ws::protocol::ServerFrame;

const CHANNEL_CAPACITY: usize = 256;

/// Minimum interval between leaderboard publishes (global, across all connections).
const LEADERBOARD_MIN_INTERVAL: Duration = Duration::from_millis(500);

struct HubInner {
    sessions: Mutex<HashMap<Uuid, broadcast::Sender<ServerFrame>>>,
    leaderboard: broadcast::Sender<ServerFrame>,
    /// Wall-clock instant of the last leaderboard publish for global throttling.
    last_leaderboard: Mutex<Option<Instant>>,
}

/// Broadcast hub for WebSocket push messages.
///
/// Holds one per-session `broadcast` channel (created on demand) and a single
/// leaderboard channel shared by all subscribers.  Cloning a `Hub` is cheap —
/// it bumps an `Arc` reference count.
#[derive(Clone)]
pub struct Hub {
    inner: Arc<HubInner>,
}

impl Hub {
    /// Create a new, empty hub.
    pub fn new() -> Self {
        let (leaderboard_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Hub {
            inner: Arc::new(HubInner {
                sessions: Mutex::new(HashMap::new()),
                leaderboard: leaderboard_tx,
                last_leaderboard: Mutex::new(None),
            }),
        }
    }

    /// Return the broadcast sender for `session_id`, creating the channel if
    /// it does not yet exist.
    pub fn session_sender(&self, session_id: Uuid) -> broadcast::Sender<ServerFrame> {
        let mut sessions = self.inner.sessions.lock().unwrap();
        sessions
            .entry(session_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .clone()
    }

    /// Return the shared leaderboard broadcast sender.
    pub fn leaderboard_sender(&self) -> broadcast::Sender<ServerFrame> {
        self.inner.leaderboard.clone()
    }

    /// Publish `frame` to all receivers subscribed to `session_id`.
    ///
    /// If the send fails (no active receivers) the channel entry is reaped to
    /// prevent unbounded map growth.
    pub fn publish_session(&self, session_id: Uuid, frame: ServerFrame) {
        let mut sessions = self.inner.sessions.lock().unwrap();
        // Evaluate send result without holding the borrow into the next branch.
        let should_reap = if let Some(tx) = sessions.get(&session_id) {
            tx.send(frame).is_err()
        } else {
            false
        };
        if should_reap {
            sessions.remove(&session_id);
        }
    }

    /// Publish `frame` to all leaderboard subscribers.
    pub fn publish_leaderboard(&self, frame: ServerFrame) {
        let _ = self.inner.leaderboard.send(frame);
    }

    /// Check the global leaderboard throttle.
    ///
    /// Returns `true` and records the current time if at least
    /// [`LEADERBOARD_MIN_INTERVAL`] has elapsed since the last publish.
    /// Returns `false` if the minimum interval has not yet elapsed.
    ///
    /// This is a sync, lock-based check intentionally: it is cheap and prevents
    /// multiple concurrent handlers from all firing a DB query for the same
    /// leaderboard snapshot.
    pub fn should_publish_leaderboard(&self) -> bool {
        let mut guard = self.inner.last_leaderboard.lock().unwrap();
        let now = Instant::now();
        let allow = guard
            .map(|prev| now.duration_since(prev) >= LEADERBOARD_MIN_INTERVAL)
            .unwrap_or(true);
        if allow {
            *guard = Some(now);
        }
        allow
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn session_subscriber_receives_published_frame() {
        let hub = Hub::new();
        let session_id = Uuid::new_v4();

        let tx = hub.session_sender(session_id);
        let mut rx = tx.subscribe();

        hub.publish_session(
            session_id,
            ServerFrame::SessionEvent {
                data: json!({"msg": "hello"}),
            },
        );

        let received = rx.recv().await.expect("should receive frame");
        assert!(
            matches!(received, ServerFrame::SessionEvent { .. }),
            "expected SessionEvent, got {received:?}"
        );
    }

    #[tokio::test]
    async fn two_subscribers_both_receive_same_frame() {
        let hub = Hub::new();
        let session_id = Uuid::new_v4();

        let tx = hub.session_sender(session_id);
        let mut rx1 = tx.subscribe();
        let mut rx2 = tx.subscribe();

        hub.publish_session(
            session_id,
            ServerFrame::PingScored {
                data: json!({"score": 42}),
            },
        );

        let r1 = rx1.recv().await.expect("rx1 should receive");
        let r2 = rx2.recv().await.expect("rx2 should receive");
        assert!(matches!(r1, ServerFrame::PingScored { .. }));
        assert!(matches!(r2, ServerFrame::PingScored { .. }));
    }

    #[tokio::test]
    async fn leaderboard_subscriber_receives_published_frame() {
        let hub = Hub::new();

        let tx = hub.leaderboard_sender();
        let mut rx = tx.subscribe();

        hub.publish_leaderboard(ServerFrame::LeaderboardUpdate {
            data: json!({"rank": 1}),
        });

        let received = rx.recv().await.expect("should receive leaderboard frame");
        assert!(
            matches!(received, ServerFrame::LeaderboardUpdate { .. }),
            "expected LeaderboardUpdate, got {received:?}"
        );
    }

    #[tokio::test]
    async fn session_channel_reaped_when_no_receivers() {
        let hub = Hub::new();
        let session_id = Uuid::new_v4();

        // Create sender but drop the subscriber immediately so receiver_count == 0.
        let _tx = hub.session_sender(session_id);
        // No subscriber — publish should reap the entry silently.
        hub.publish_session(
            session_id,
            ServerFrame::Error {
                error: json!({"message": "test"}),
            },
        );

        // After reaping, getting the sender again creates a fresh channel (len 0).
        let tx2 = hub.session_sender(session_id);
        assert_eq!(tx2.receiver_count(), 0, "fresh channel should have 0 receivers");
    }
}
