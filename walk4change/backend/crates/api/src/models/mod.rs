use chrono::{DateTime, Utc};
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
