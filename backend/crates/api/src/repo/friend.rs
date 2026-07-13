use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, FieldError},
    models::{FriendsList, PendingItem, Profile},
};

/// Flat row returned from pending-request joins.
#[derive(sqlx::FromRow)]
struct PendingRow {
    pub request_id: Uuid,
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub interests: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub email_verified: bool,
}

impl From<PendingRow> for PendingItem {
    fn from(r: PendingRow) -> Self {
        PendingItem {
            request_id: r.request_id,
            user: Profile {
                id: r.id,
                email: r.email,
                display_name: r.display_name,
                avatar_url: r.avatar_url,
                bio: r.bio,
                interests: r.interests,
                created_at: r.created_at,
                email_verified: r.email_verified,
            },
        }
    }
}

/// Send a friend request from `requester` to `addressee`.
///
/// Errors:
/// - `requester == addressee` → 422 Validation
/// - any row already exists in EITHER direction → 409 Conflict("friendship_exists")
/// - DB unique violation (race) → 409 Conflict("friendship_exists")
pub async fn send_request(
    pool: &PgPool,
    id: Uuid,
    requester: Uuid,
    addressee: Uuid,
) -> Result<(), AppError> {
    if requester == addressee {
        return Err(AppError::Validation(vec![FieldError {
            field: "addressee_id".into(),
            message: "cannot send a friend request to yourself".into(),
            code: "SELF_REQUEST".into(),
        }]));
    }

    // Block gate (audit 2026-07-10): a block in either direction forbids
    // re-inviting — without this, unfriend+block could be undone by a fresh
    // request the victim accepts by mistake.
    if crate::repo::block::is_blocked_either(pool, requester, addressee).await? {
        return Err(AppError::Forbidden);
    }

    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    // Check for any existing row in EITHER direction.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
            SELECT 1 FROM friendships \
            WHERE (requester_id = $1 AND addressee_id = $2) \
               OR (requester_id = $2 AND addressee_id = $1) \
        )",
    )
    .bind(requester)
    .bind(addressee)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    if exists {
        return Err(AppError::Conflict("friendship_exists".into()));
    }

    sqlx::query(
        "INSERT INTO friendships (id, requester_id, addressee_id, status) \
         VALUES ($1, $2, $3, 'pending')",
    )
    .bind(id)
    .bind(requester)
    .bind(addressee)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db) = e {
            if db.is_unique_violation() {
                return AppError::Conflict("friendship_exists".into());
            }
        }
        AppError::internal(e)
    })?;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(())
}

/// Accept or decline a pending friendship request.
///
/// `actor` must be the `addressee_id` of the request and its status must be `'pending'`.
/// - If `accept` is true: update status to `'accepted'`.
/// - If `accept` is false: delete the row (decline — schema only allows 'pending'|'accepted').
///
/// 0 rows affected → `AppError::Forbidden` (actor is not the addressee or not pending).
pub async fn respond(
    pool: &PgPool,
    request_id: Uuid,
    actor: Uuid,
    accept: bool,
) -> Result<(), AppError> {
    let rows_affected = if accept {
        sqlx::query(
            "UPDATE friendships SET status = 'accepted' \
             WHERE id = $1 AND addressee_id = $2 AND status = 'pending'",
        )
        .bind(request_id)
        .bind(actor)
        .execute(pool)
        .await
        .map_err(AppError::internal)?
        .rows_affected()
    } else {
        sqlx::query(
            "DELETE FROM friendships \
             WHERE id = $1 AND addressee_id = $2 AND status = 'pending'",
        )
        .bind(request_id)
        .bind(actor)
        .execute(pool)
        .await
        .map_err(AppError::internal)?
        .rows_affected()
    };

    if rows_affected == 0 {
        return Err(AppError::Forbidden);
    }

    Ok(())
}

/// Remove ANY friendship row between `actor` and `other` (either direction,
/// any status): unfriend an accepted friend, cancel an outgoing request or
/// drop an incoming one. Severs the 1:1 chat channel (friends-only).
///
/// Returns 404 when no row exists.
pub async fn remove(pool: &PgPool, actor: Uuid, other: Uuid) -> Result<(), AppError> {
    let rows = sqlx::query(
        "DELETE FROM friendships \
         WHERE (requester_id = $1 AND addressee_id = $2) \
            OR (requester_id = $2 AND addressee_id = $1)",
    )
    .bind(actor)
    .bind(other)
    .execute(pool)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Return `true` if users `a` and `b` have an `accepted` friendship in either direction.
pub async fn are_friends(pool: &PgPool, a: Uuid, b: Uuid) -> Result<bool, AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
            SELECT 1 FROM friendships \
            WHERE status = 'accepted' \
              AND ( (requester_id = $1 AND addressee_id = $2) \
                 OR (requester_id = $2 AND addressee_id = $1) ) \
        )",
    )
    .bind(a)
    .bind(b)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(exists)
}

/// Return all friendships (accepted + pending) for `actor`.
pub async fn list(pool: &PgPool, actor: Uuid) -> Result<FriendsList, AppError> {
    // Accepted: the other party's profile, direction-agnostic.
    let accepted: Vec<Profile> = sqlx::query_as(
        "SELECT u.id, u.email::text AS email, u.display_name, u.avatar_url, u.bio, \
                u.interests, u.created_at, \
                (u.email_verified_at IS NOT NULL) AS email_verified \
         FROM friendships f \
         JOIN users u ON u.id = CASE \
                 WHEN f.requester_id = $1 THEN f.addressee_id \
                 ELSE f.requester_id \
             END \
         WHERE f.status = 'accepted' \
           AND (f.requester_id = $1 OR f.addressee_id = $1)",
    )
    .bind(actor)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    // Incoming pending: actor is the addressee.
    let incoming_rows: Vec<PendingRow> = sqlx::query_as(
        "SELECT f.id AS request_id, \
                u.id, u.email::text AS email, u.display_name, u.avatar_url, u.bio, \
                u.interests, u.created_at, \
                (u.email_verified_at IS NOT NULL) AS email_verified \
         FROM friendships f \
         JOIN users u ON u.id = f.requester_id \
         WHERE f.addressee_id = $1 AND f.status = 'pending'",
    )
    .bind(actor)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    // Outgoing pending: actor is the requester.
    let outgoing_rows: Vec<PendingRow> = sqlx::query_as(
        "SELECT f.id AS request_id, \
                u.id, u.email::text AS email, u.display_name, u.avatar_url, u.bio, \
                u.interests, u.created_at, \
                (u.email_verified_at IS NOT NULL) AS email_verified \
         FROM friendships f \
         JOIN users u ON u.id = f.addressee_id \
         WHERE f.requester_id = $1 AND f.status = 'pending'",
    )
    .bind(actor)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(FriendsList {
        accepted,
        incoming_pending: incoming_rows.into_iter().map(PendingItem::from).collect(),
        outgoing_pending: outgoing_rows.into_iter().map(PendingItem::from).collect(),
    })
}
