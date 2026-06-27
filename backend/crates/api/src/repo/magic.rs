//! Magic-link token storage: create one-time tokens, consume them atomically.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Token lifetime.
const TTL_MINUTES: i64 = 15;

/// Create a one-time magic token for `user_id`, valid for [`TTL_MINUTES`].
pub async fn create_token(pool: &PgPool, token: &str, user_id: Uuid) -> Result<(), AppError> {
    let expires_at = Utc::now() + Duration::minutes(TTL_MINUTES);
    sqlx::query("INSERT INTO magic_links (token, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(token)
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await
        .map_err(AppError::internal)?;
    Ok(())
}

/// Atomically consume a valid (unused, unexpired) token, returning its `user_id`.
/// Marks the token used so it cannot be replayed. Invalid/expired → Unauthorized.
pub async fn consume_token(pool: &PgPool, token: &str) -> Result<Uuid, AppError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "UPDATE magic_links SET used = true \
         WHERE token = $1 AND used = false AND expires_at > now() \
         RETURNING user_id",
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    row.map(|(id,)| id).ok_or(AppError::Unauthorized)
}
