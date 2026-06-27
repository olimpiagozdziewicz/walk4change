use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

/// Public user profile returned from the API.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Profile {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub interests: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// A pending friendship request bundled with the other party's profile.
#[derive(Debug, Serialize)]
pub struct PendingItem {
    pub request_id: Uuid,
    pub user: Profile,
}

/// Result of `GET /api/v1/friends`.
#[derive(Debug, Serialize)]
pub struct FriendsList {
    pub accepted: Vec<Profile>,
    pub incoming_pending: Vec<PendingItem>,
    pub outgoing_pending: Vec<PendingItem>,
}

/// A walk session row returned from the API.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WalkSession {
    pub id: Uuid,
    pub host_id: Uuid,
    pub status: String,
    pub join_code: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

/// A single participant in a walk session.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ParticipantInfo {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub total_meters: Decimal,
    pub total_points: Decimal,
}

/// Full walk detail: session + participants list.
#[derive(Debug, Serialize)]
pub struct WalkDetail {
    pub session: WalkSession,
    pub participants: Vec<ParticipantInfo>,
}

/// A reward catalog entry returned from `GET /api/v1/rewards`.
///
/// `type_` maps to the SQL column `type` (a Rust keyword), so the SELECT must
/// alias it as `type AS type_`; it is serialised back to JSON as `type`.
/// `stock = None` means unlimited stock.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Reward {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub cost_points: Decimal,
    pub partner_name: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub stock: Option<i32>,
    pub image_url: Option<String>,
}

/// A reward redemption record returned from redeem / list endpoints.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Redemption {
    pub id: Uuid,
    pub reward_id: Uuid,
    pub code: String,
    pub points_spent: Decimal,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// A single location ping point returned from the track endpoint.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PingPoint {
    pub user_id: Uuid,
    pub seq: i32,
    pub lat: f64,
    pub lng: f64,
    pub points: Decimal,
    pub recorded_at: DateTime<Utc>,
}
