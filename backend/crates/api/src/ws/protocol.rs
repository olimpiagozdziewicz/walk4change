use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Frames sent from the client to the server.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientFrame {
    Auth {
        token: String,
    },
    Ping {
        session_id: Uuid,
        seq: i32,
        lat: f64,
        lng: f64,
        recorded_at: DateTime<Utc>,
        /// GPS accuracy radius in meters (lower is better). Optional for
        /// backward compatibility; when present, poor-accuracy pings are dropped.
        #[serde(default)]
        accuracy: Option<f64>,
    },
    Subscribe {
        session_id: Uuid,
    },
    SubscribeLeaderboard,
}

/// Frames sent from the server to connected clients.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFrame {
    PingScored { data: Value },
    LeaderboardUpdate { data: Value },
    SessionEvent { data: Value },
    Error { error: Value },
}

impl ServerFrame {
    /// Build an error frame with a human-readable message.
    pub fn error(msg: impl Into<String>) -> Self {
        ServerFrame::Error {
            error: serde_json::json!({ "message": msg.into() }),
        }
    }

    /// Serialize the frame to a JSON string (panics only if serialisation is
    /// fundamentally broken — all fields are `Value`, so this should never
    /// fail in practice).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ServerFrame serialization failed")
    }
}
