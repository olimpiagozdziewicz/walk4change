use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::AppError,
    repo::friend as friend_repo,
    response,
    state::AppState,
};

/// Request body for `POST /api/v1/friends/request`.
#[derive(Deserialize)]
pub struct SendRequestBody {
    pub addressee_id: Uuid,
}

/// Request body for `POST /api/v1/friends/respond`.
#[derive(Deserialize)]
pub struct RespondBody {
    pub request_id: Uuid,
    pub accept: bool,
}

/// `POST /api/v1/friends/request`
///
/// Sends a friend request from the authenticated user to `addressee_id`.
/// - 201 on success.
/// - 422 if `addressee_id == requester` (self-request).
/// - 409 if any friendship row already exists in either direction.
pub async fn send_request(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<SendRequestBody>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::new_v4();
    friend_repo::send_request(&state.pool, id, auth.id, body.addressee_id).await?;
    Ok(StatusCode::CREATED)
}

/// `POST /api/v1/friends/respond`
///
/// Accept or decline a pending friend request.
/// - 200 on success.
/// - 403 if the authenticated user is not the addressee or the request is not pending.
pub async fn respond(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<RespondBody>,
) -> Result<Json<Value>, AppError> {
    friend_repo::respond(&state.pool, body.request_id, auth.id, body.accept).await?;
    Ok(response::data(serde_json::json!({})))
}

/// `GET /api/v1/friends`
///
/// Returns the authenticated user's full friendship list:
/// - `accepted`: profiles of accepted friends (either direction).
/// - `incoming_pending`: requests addressed to the caller, not yet accepted.
/// - `outgoing_pending`: requests sent by the caller, not yet accepted.
pub async fn list(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let friends = friend_repo::list(&state.pool, auth.id).await?;
    Ok(response::data(friends))
}
