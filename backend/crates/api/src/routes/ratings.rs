//! Post-walk rating endpoints (spec 2026-07-13).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::AppError,
    repo::rating as rating_repo,
    response,
    state::AppState,
};

/// Body for `POST /api/v1/walks/:id/rate`.
#[derive(Deserialize)]
pub struct RateBody {
    pub user_id: Uuid,
    pub recommend: bool,
    #[serde(default)]
    pub flag: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
}

/// `POST /api/v1/walks/:id/rate`
///
/// Rate a co-participant of a finished walk (recommend / don't recommend,
/// optional non-public moderation flag). Upserts within the 48 h window.
/// 201 on success; 403 not a participant / kicked / blocked; 404 session not
/// finished or target not a participant; 409 window closed; 422 validation.
pub async fn rate_participant(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<RateBody>,
) -> Result<StatusCode, AppError> {
    rating_repo::rate(
        &state.pool,
        session_id,
        auth.id,
        body.user_id,
        body.recommend,
        body.flag.as_deref(),
        body.comment.as_deref(),
    )
    .await?;
    Ok(StatusCode::CREATED)
}

/// `GET /api/v1/walks/:id/ratings/mine`
///
/// The caller's own ratings for this session (UI: mark who is already rated).
pub async fn my_ratings(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let ratings = rating_repo::mine(&state.pool, session_id, auth.id).await?;
    Ok(response::data(ratings))
}

/// `GET /api/v1/users/:id/rating`
///
/// Reputation aggregate: `{ total, recommend_count, visible }` — clients show
/// the counts only when `visible` (≥3 ratings; below that a rating would
/// identify its author).
pub async fn user_rating(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let agg = rating_repo::aggregate(&state.pool, user_id).await?;
    Ok(response::data(agg))
}
