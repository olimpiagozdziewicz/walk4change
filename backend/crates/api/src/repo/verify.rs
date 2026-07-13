//! E-mail verification tokens (spec 2026-07-13): one-time, 24 h TTL.
//!
//! Mirrors `repo::magic`, but consuming a token only proves mailbox ownership
//! (sets `users.email_verified_at`) — it never mints a session.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Token lifetime — longer than magic-link: the mail is not a login credential.
const TTL_HOURS: i64 = 24;

/// Create a one-time verification token for `user_id`, valid for [`TTL_HOURS`].
pub async fn create_token(pool: &PgPool, token: &str, user_id: Uuid) -> Result<(), AppError> {
    let expires_at = Utc::now() + Duration::hours(TTL_HOURS);
    sqlx::query(
        "INSERT INTO email_verification_tokens (token, user_id, expires_at) \
         VALUES ($1, $2, $3)",
    )
    .bind(token)
    .bind(user_id)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(AppError::internal)?;
    Ok(())
}

/// Atomically consume a valid (unused, unexpired) token, returning its `user_id`.
/// Invalid/expired → Unauthorized.
pub async fn consume_token(pool: &PgPool, token: &str) -> Result<Uuid, AppError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "UPDATE email_verification_tokens SET used = true \
         WHERE token = $1 AND used = false AND expires_at > now() \
         RETURNING user_id",
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    row.map(|(id,)| id).ok_or(AppError::Unauthorized)
}
