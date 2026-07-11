//! User blocks: a hard, one-sided cut of all social contact (audit 2026-07-10
//! B1.3 follow-up — unfriend severed the chat but did not prevent re-inviting).
//!
//! Blocking also deletes any friendship row (either direction, any status), so
//! the chat channel closes immediately via the existing friends-only gates.
//! Enforcement points live at: friend requests, direct messages, eco
//! likes/comments on the blocker's reports, and joining the blocker's walks.

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, FieldError},
    models::UserSearchItem,
};

/// Block `blocked` on behalf of `blocker`. Idempotent (already blocked → OK).
///
/// In one transaction: insert the block row and delete any friendship between
/// the pair so the 1:1 chat cuts instantly (friends-only on read and write).
pub async fn block(pool: &PgPool, blocker: Uuid, blocked: Uuid) -> Result<(), AppError> {
    if blocker == blocked {
        return Err(AppError::Validation(vec![FieldError {
            field: "user_id".into(),
            message: "cannot block yourself".into(),
            code: "SELF_BLOCK".into(),
        }]));
    }

    // FK on blocked_id catches unknown users; map to 404 instead of 500.
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    sqlx::query(
        "INSERT INTO user_blocks (blocker_id, blocked_id) VALUES ($1, $2) \
         ON CONFLICT DO NOTHING",
    )
    .bind(blocker)
    .bind(blocked)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db) = e {
            if db.is_foreign_key_violation() {
                return AppError::NotFound;
            }
        }
        AppError::internal(e)
    })?;

    sqlx::query(
        "DELETE FROM friendships \
         WHERE (requester_id = $1 AND addressee_id = $2) \
            OR (requester_id = $2 AND addressee_id = $1)",
    )
    .bind(blocker)
    .bind(blocked)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    tx.commit().await.map_err(AppError::internal)?;
    Ok(())
}

/// Remove the block `blocker → blocked`. 404 when no such block exists.
pub async fn unblock(pool: &PgPool, blocker: Uuid, blocked: Uuid) -> Result<(), AppError> {
    let rows = sqlx::query(
        "DELETE FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2",
    )
    .bind(blocker)
    .bind(blocked)
    .execute(pool)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// List the users blocked by `blocker` (same minimal shape as user search).
pub async fn list(pool: &PgPool, blocker: Uuid) -> Result<Vec<UserSearchItem>, AppError> {
    let rows: Vec<UserSearchItem> = sqlx::query_as(
        "SELECT u.id, u.display_name, u.avatar_url \
         FROM user_blocks b \
         JOIN users u ON u.id = b.blocked_id \
         WHERE b.blocker_id = $1 \
         ORDER BY b.created_at DESC",
    )
    .bind(blocker)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(rows)
}

/// `true` when a block exists between `a` and `b` in EITHER direction.
///
/// All enforcement uses the symmetric check: the blocked party must not reach
/// the blocker, and the blocker also stops interacting with the blocked one
/// (standard social-app semantics; avoids "block then keep poking" asymmetry).
pub async fn is_blocked_either(pool: &PgPool, a: Uuid, b: Uuid) -> Result<bool, AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
            SELECT 1 FROM user_blocks \
            WHERE (blocker_id = $1 AND blocked_id = $2) \
               OR (blocker_id = $2 AND blocked_id = $1) \
        )",
    )
    .bind(a)
    .bind(b)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(exists)
}
