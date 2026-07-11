use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::AppError,
    repo::message as message_repo,
    response,
    state::AppState,
};

/// Query parameters for `GET /api/v1/messages/:user_id`.
#[derive(Deserialize)]
pub struct ConversationQuery {
    /// Only messages strictly newer than this timestamp (polling cursor).
    pub after: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

/// Body for `POST /api/v1/messages/:user_id`.
#[derive(Deserialize)]
pub struct SendMessageBody {
    pub body: String,
}

/// `GET /api/v1/conversations`
///
/// List the caller's conversations: one row per partner with the latest
/// message and unread counter, newest first.
pub async fn list_conversations(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let rows = message_repo::conversations(&state.pool, auth.id).await?;
    Ok(response::data(rows))
}

/// `GET /api/v1/messages/:user_id`
///
/// Fetch the conversation with `user_id`, oldest first. Supports `?after=<ts>`
/// for cheap polling. Side effect: marks incoming messages as read.
pub async fn get_conversation(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ConversationQuery>,
) -> Result<Json<Value>, AppError> {
    let messages =
        message_repo::conversation(&state.pool, auth.id, user_id, query.after, query.limit)
            .await?;
    Ok(response::data(messages))
}

/// `POST /api/v1/messages/:user_id`
///
/// Send a direct message to `user_id`. Friends-only (403 otherwise);
/// body must be 1..=2000 characters (422 otherwise). Returns 201 with the row.
pub async fn send_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(body): Json<SendMessageBody>,
) -> Result<Response, AppError> {
    // Per-ACCOUNT quota (audit N3/B1.6): 120/min per-IP still allowed one
    // "friend" to firehose the victim; 30/min per account caps it sanely.
    crate::util::ratelimit::check_message_quota(auth.id)
        .map_err(|_| AppError::RateLimited)?;

    let id = Uuid::new_v4();
    let message = message_repo::send(&state.pool, id, auth.id, user_id, &body.body).await?;
    Ok((StatusCode::CREATED, response::data(message)).into_response())
}
