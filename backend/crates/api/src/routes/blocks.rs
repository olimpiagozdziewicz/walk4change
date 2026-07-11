//! Block endpoints: a hard, one-sided cut of all social contact
//! (audit 2026-07-10 — unfriend alone did not prevent re-inviting).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::AppError,
    repo::block as block_repo,
    response,
    state::AppState,
};

/// `POST /api/v1/blocks/:user_id`
///
/// Block `user_id`: severs any friendship (cuts the 1:1 chat both ways) and
/// forbids friend requests, direct messages, eco likes/comments on the
/// blocker's posts, and joining the blocker's walks — in EITHER direction.
/// Idempotent. 201 on success; 422 self-block; 404 unknown user.
pub async fn block_user(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    block_repo::block(&state.pool, auth.id, user_id).await?;
    Ok(StatusCode::CREATED)
}

/// `DELETE /api/v1/blocks/:user_id`
///
/// Remove the caller's block on `user_id`. 204 on success; 404 when the
/// caller had not blocked this user.
pub async fn unblock_user(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    block_repo::unblock(&state.pool, auth.id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/blocks`
///
/// List the users blocked by the caller (id + display_name + avatar_url).
pub async fn list_blocks(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let blocked = block_repo::list(&state.pool, auth.id).await?;
    Ok(response::data(blocked))
}
