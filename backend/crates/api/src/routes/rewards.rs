use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::AppError,
    repo::reward as reward_repo,
    response,
    state::AppState,
};

/// `GET /api/v1/rewards`
///
/// List all active rewards in the catalog. Authenticated.
///
/// Returns 200 with `{ data: [Reward] }`.
pub async fn list_rewards(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let rewards = reward_repo::list(&state.pool).await?;
    Ok(response::data(rewards))
}

/// `POST /api/v1/rewards/:id/redeem`
///
/// Atomically redeem a reward for the authenticated user. Spends points and
/// reserves a unique redemption code with no oversell.
///
/// Returns 201 with a `Location` header pointing to the user's redemptions and
/// the redemption data in the body. Returns 409 if the reward is unavailable
/// (sold out / inactive) or the user has insufficient points.
pub async fn redeem_reward(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(reward_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let redemption = reward_repo::redeem(&state.pool, reward_id, auth.id).await?;
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, "/api/v1/me/redemptions")],
        response::data(redemption),
    )
        .into_response())
}

/// `GET /api/v1/me/redemptions`
///
/// List the authenticated user's redemptions, newest first.
///
/// Returns 200 with `{ data: [Redemption] }`.
pub async fn list_my_redemptions(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let redemptions = reward_repo::list_redemptions(&state.pool, auth.id).await?;
    Ok(response::data(redemptions))
}
