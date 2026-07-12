//! Walk ratings: post-walk mutual "recommend / don't recommend" between
//! participants of the same finished session (spec 2026-07-13).
//!
//! Design guards (chat decision 2026-07-11):
//! - binary verdict, not stars (star ratings degenerate to 5.0);
//! - optional NON-public `flag` = moderation signal, doubles as a report;
//! - kicked participants cannot rate (revenge-rating), but CAN be rated;
//! - blocked pairs cannot rate each other;
//! - aggregates go public only from [`MIN_VISIBLE_RATINGS`] — below that a
//!   rating would identify its author at SeaSteps' current scale.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, FieldError},
    models::{MyRating, RatingAggregate},
    repo::block,
};

/// How long after `ended_at` a session stays rateable.
const RATING_WINDOW_HOURS: i64 = 48;

/// Aggregates are hidden below this many ratings (author de-anonymisation).
pub const MIN_VISIBLE_RATINGS: i64 = 3;

/// Accepted moderation flags (matches the CHECK in migration 0010).
const ALLOWED_FLAGS: [&str; 4] = ["no_show", "unsafe", "spam", "other"];

/// Rate `rated` for the finished session `session_id` as `rater` (upsert).
///
/// Errors: 422 self-rating / bad flag / long comment; 404 unknown or
/// non-finished session, or `rated` never a participant; 403 rater not a
/// participant, rater was kicked, or a block exists between the pair;
/// 409 `rating_window_closed` past the 48 h window.
pub async fn rate(
    pool: &PgPool,
    session_id: Uuid,
    rater: Uuid,
    rated: Uuid,
    recommend: bool,
    flag: Option<&str>,
    comment: Option<&str>,
) -> Result<(), AppError> {
    if rater == rated {
        return Err(AppError::Validation(vec![FieldError {
            field: "user_id".into(),
            message: "cannot rate yourself".into(),
            code: "SELF_RATING".into(),
        }]));
    }
    let flag = flag.map(str::trim).filter(|f| !f.is_empty());
    if let Some(f) = flag {
        if !ALLOWED_FLAGS.contains(&f) {
            return Err(AppError::Validation(vec![FieldError {
                field: "flag".into(),
                message: "flag must be one of: no_show, unsafe, spam, other".into(),
                code: "INVALID".into(),
            }]));
        }
    }
    let comment = comment.map(str::trim).filter(|c| !c.is_empty());
    if let Some(c) = comment {
        if c.chars().count() > 280 {
            return Err(AppError::Validation(vec![FieldError {
                field: "comment".into(),
                message: "comment must be at most 280 characters".into(),
                code: "INVALID_LENGTH".into(),
            }]));
        }
    }

    // Session must exist, be finished, and be within the rating window.
    let session: Option<(String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT status, ended_at FROM walk_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    let (status, ended_at) = session.ok_or(AppError::NotFound)?;
    if status != "finished" {
        return Err(AppError::NotFound);
    }
    let ended_at = ended_at.ok_or(AppError::NotFound)?;
    if Utc::now().signed_duration_since(ended_at) > Duration::hours(RATING_WINDOW_HOURS) {
        return Err(AppError::Conflict("rating_window_closed".into()));
    }

    // Rater: participant AND not kicked (kick bars rating — revenge guard).
    let rater_row: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
        "SELECT kicked_at FROM walk_participants WHERE session_id = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(rater)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    match rater_row {
        None => return Err(AppError::Forbidden),
        Some((Some(_),)) => return Err(AppError::Forbidden), // kicked
        Some((None,)) => {}
    }

    // Rated: must have been a participant (kicked people CAN be rated/flagged).
    let rated_is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 FROM walk_participants WHERE session_id = $1 AND user_id = $2 \
        )",
    )
    .bind(session_id)
    .bind(rated)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    if !rated_is_member {
        return Err(AppError::NotFound);
    }

    if block::is_blocked_either(pool, rater, rated).await? {
        return Err(AppError::Forbidden);
    }

    // Upsert: re-rating within the window overwrites (change of mind).
    sqlx::query(
        "INSERT INTO walk_ratings (id, session_id, rater_id, rated_id, recommend, flag, comment) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (session_id, rater_id, rated_id) \
         DO UPDATE SET recommend = EXCLUDED.recommend, flag = EXCLUDED.flag, \
                       comment = EXCLUDED.comment, created_at = now()",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(rater)
    .bind(rated)
    .bind(recommend)
    .bind(flag)
    .bind(comment)
    .execute(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(())
}

/// The caller's own ratings for a session (UI state: "already rated").
pub async fn mine(
    pool: &PgPool,
    session_id: Uuid,
    rater: Uuid,
) -> Result<Vec<MyRating>, AppError> {
    let rows: Vec<MyRating> = sqlx::query_as(
        "SELECT rated_id AS user_id, recommend, flag \
         FROM walk_ratings WHERE session_id = $1 AND rater_id = $2",
    )
    .bind(session_id)
    .bind(rater)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(rows)
}

/// Public (in-app) reputation aggregate for `user_id`.
///
/// `visible = total >= MIN_VISIBLE_RATINGS`; the raw counts are still returned
/// (they are aggregate-only), the client hides them below the threshold.
pub async fn aggregate(pool: &PgPool, user_id: Uuid) -> Result<RatingAggregate, AppError> {
    let (total, recommend_count): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE recommend) \
         FROM walk_ratings WHERE rated_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(RatingAggregate {
        total,
        recommend_count,
        visible: total >= MIN_VISIBLE_RATINGS,
    })
}
