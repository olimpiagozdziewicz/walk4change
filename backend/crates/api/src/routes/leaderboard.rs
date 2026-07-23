//! `GET /api/v1/leaderboard` — paginated standings ordered by `total_points DESC`.

use axum::{
    extract::{Query, State},
    Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::AppError,
    response,
    state::AppState,
    util::pagination::{PageMeta, Pagination},
};

/// A single leaderboard row returned to clients.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LeaderboardEntry {
    pub user_id: Uuid,
    pub display_name: String,
    pub total_points: Decimal,
}

/// Query parameters for `GET /api/v1/leaderboard`.
#[derive(Deserialize)]
pub struct LeaderboardQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Return the top-`n` leaderboard entries without pagination.
///
/// Used by the WebSocket handler to build the `leaderboard_update` broadcast.
pub async fn top_n(pool: &PgPool, n: i64) -> Result<Vec<LeaderboardEntry>, AppError> {
    sqlx::query_as::<_, LeaderboardEntry>(
        "SELECT ut.user_id, u.display_name, ut.total_points \
         FROM user_totals ut \
         JOIN users u ON u.id = ut.user_id \
         WHERE u.deleted_at IS NULL \
         ORDER BY ut.total_points DESC, u.display_name ASC, u.id ASC \
         LIMIT $1",
    )
    .bind(n)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)
}

/// Total number of rows in `user_totals` (for pagination).
async fn count_leaderboard(pool: &PgPool) -> Result<i64, AppError> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_totals ut \
         JOIN users u ON u.id = ut.user_id \
         WHERE u.deleted_at IS NULL",
    )
    .fetch_one(pool)
        .await
        .map_err(AppError::internal)
}

/// Fetch one page of leaderboard entries.
async fn list_leaderboard(
    pool: &PgPool,
    pagination: &Pagination,
) -> Result<Vec<LeaderboardEntry>, AppError> {
    sqlx::query_as::<_, LeaderboardEntry>(
        "SELECT ut.user_id, u.display_name, ut.total_points \
         FROM user_totals ut \
         JOIN users u ON u.id = ut.user_id \
         WHERE u.deleted_at IS NULL \
         ORDER BY ut.total_points DESC, u.display_name ASC, u.id ASC \
         LIMIT $1 OFFSET $2",
    )
    .bind(pagination.per_page)
    .bind(pagination.offset())
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)
}

/// `GET /api/v1/leaderboard?page=&per_page=`
///
/// Returns the global leaderboard ordered by `total_points DESC`.
///
/// - Requires `Authorization: Bearer <jwt>` (or `wc_session` cookie) → 401 otherwise.
/// - `per_page` defaults to 20, maximum 100 → 422 if exceeded.
/// - `page` defaults to 1, must be ≥ 1 → 422 otherwise.
///
/// Response envelope:
/// ```json
/// { "data": [...], "meta": { "total", "page", "per_page", "total_pages" } }
/// ```
pub async fn get_leaderboard(
    _auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Json<Value>, AppError> {
    let pagination = Pagination::from_query(query.page, query.per_page)?;

    let total = count_leaderboard(&state.pool).await?;
    let entries = list_leaderboard(&state.pool, &pagination).await?;
    let meta = PageMeta::new(total, &pagination);

    Ok(response::data_paginated(
        entries,
        serde_json::to_value(&meta).expect("PageMeta serialization is infallible"),
    ))
}
