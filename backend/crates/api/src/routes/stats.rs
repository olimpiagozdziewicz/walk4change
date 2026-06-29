use axum::{extract::State, Json};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::{auth::extractor::AuthUser, error::AppError, response, state::AppState};

/// `GET /api/v1/me/stats`
///
/// Returns today's walk stats + lifetime totals for the authenticated user.
/// All values default to 0 when the user has no walk history.
pub async fn get_me_stats(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    // Today's accumulated points and meters from walk sessions started today.
    let (today_points, today_meters): (Decimal, Decimal) = sqlx::query_as(
        "SELECT \
            COALESCE(SUM(wp.total_points), 0), \
            COALESCE(SUM(wp.total_meters), 0) \
         FROM walk_participants wp \
         JOIN walk_sessions ws ON ws.id = wp.session_id \
         WHERE wp.user_id = $1 AND ws.started_at >= CURRENT_DATE",
    )
    .bind(auth.id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;

    // Lifetime totals from user_totals.
    let totals: Option<(Decimal, Decimal, i32)> = sqlx::query_as(
        "SELECT total_points, total_meters, total_walks \
         FROM user_totals WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let (total_points, _total_meters, total_walks) =
        totals.unwrap_or((Decimal::ZERO, Decimal::ZERO, 0));

    // Streak: consecutive calendar days ending today where the user has a walk.
    let streak: i64 = sqlx::query_scalar(
        "WITH daily AS ( \
            SELECT DISTINCT ws.started_at::date AS d \
            FROM walk_participants wp \
            JOIN walk_sessions ws ON ws.id = wp.session_id \
            WHERE wp.user_id = $1 AND ws.started_at >= CURRENT_DATE - INTERVAL '365 days' \
         ) \
         SELECT COUNT(*) FROM ( \
            SELECT d, ROW_NUMBER() OVER (ORDER BY d DESC) AS rn \
            FROM daily WHERE d <= CURRENT_DATE \
         ) sub \
         WHERE CURRENT_DATE - (rn - 1)::int = d",
    )
    .bind(auth.id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    // Estimated steps: avg stride ~0.75 m → steps = meters × 4/3.
    let today_steps = (today_meters * Decimal::new(4, 0) / Decimal::new(3, 0)).round();

    Ok(response::data(serde_json::json!({
        "today_steps":   today_steps,
        "today_points":  today_points,
        "today_meters":  today_meters,
        "total_points":  total_points,
        "total_walks":   total_walks,
        "streak_days":   streak,
    })))
}
