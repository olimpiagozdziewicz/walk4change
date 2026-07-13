use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{Profile, UserSearchItem},
};

/// Minimal user row used during authentication.
#[derive(sqlx::FromRow)]
pub struct UserAuthRow {
    pub id: Uuid,
    pub password_hash: String,
}

/// Version of the terms/privacy documents the user accepts at sign-up.
/// Bump when `regulamin.html` / `privacy.html` change materially.
pub const TERMS_VERSION: &str = "2026-07-13";

/// Insert a new user and their totals row in a single transaction.
///
/// `accepted_terms` records the sign-up consent (RODO, spec 2026-07-13);
/// callers validate that it is true BEFORE creating the account.
/// Maps a unique-violation on the email column to [`AppError::Conflict`].
pub async fn create(
    pool: &PgPool,
    id: Uuid,
    email: &str,
    password_hash: &str,
    display_name: &str,
    accepted_terms: bool,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name, \
                            accepted_terms_at, terms_version) \
         VALUES ($1, $2, $3, $4, \
                 CASE WHEN $5 THEN now() END, \
                 CASE WHEN $5 THEN $6 END)",
    )
    .bind(id)
    .bind(email)
    .bind(password_hash)
    .bind(display_name)
    .bind(accepted_terms)
    .bind(TERMS_VERSION)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db) = e {
            if db.is_unique_violation() {
                return AppError::Conflict("email_taken".into());
            }
        }
        AppError::internal(e)
    })?;

    sqlx::query("INSERT INTO user_totals (user_id) VALUES ($1)")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::internal)?;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(())
}

/// Look up a user by email for authentication purposes.
///
/// Returns `None` when no matching user is found.
pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<UserAuthRow>, AppError> {
    sqlx::query_as::<_, UserAuthRow>(
        "SELECT id, password_hash FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)
}

/// Fetch the full public profile for a user.
///
/// Returns [`AppError::NotFound`] when the id does not exist.
pub async fn get_profile(pool: &PgPool, id: Uuid) -> Result<Profile, AppError> {
    sqlx::query_as::<_, Profile>(
        "SELECT id, email::text AS email, display_name, avatar_url, bio, \
         interests, created_at, \
         (email_verified_at IS NOT NULL) AS email_verified \
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?
    .ok_or(AppError::NotFound)
}

/// Mark the user's e-mail as verified (idempotent — keeps the first timestamp).
pub async fn set_email_verified(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE users SET email_verified_at = now() \
         WHERE id = $1 AND email_verified_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(AppError::internal)?;
    Ok(())
}

/// Whether the user's e-mail is verified (open-walks gate, spec 2026-07-13).
pub async fn is_email_verified(pool: &PgPool, id: Uuid) -> Result<bool, AppError> {
    let verified: Option<bool> = sqlx::query_scalar(
        "SELECT email_verified_at IS NOT NULL FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;
    Ok(verified.unwrap_or(false))
}

/// Whether the account is soft-deleted (tombstone). Missing row counts as
/// deleted — a JWT for a nonexistent user must not authenticate.
pub async fn is_deleted(pool: &PgPool, id: Uuid) -> Result<bool, AppError> {
    let alive: Option<bool> = sqlx::query_scalar(
        "SELECT deleted_at IS NULL FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;
    Ok(!alive.unwrap_or(false))
}

/// Search users by display name (case-insensitive substring), excluding the caller.
///
/// Returns at most 10 minimal public rows (id + display name + avatar; never e-mail).
/// LIKE wildcards in the query are escaped so they match literally.
pub async fn search(pool: &PgPool, q: &str, exclude: Uuid) -> Result<Vec<UserSearchItem>, AppError> {
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let pattern = format!("%{escaped}%");

    sqlx::query_as::<_, UserSearchItem>(
        "SELECT id, display_name, avatar_url \
         FROM users \
         WHERE display_name ILIKE $1 AND id <> $2 \
         ORDER BY display_name \
         LIMIT 10",
    )
    .bind(pattern)
    .bind(exclude)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)
}

/// Fields that can be updated via `PATCH /api/v1/me`.
/// All fields are optional; `None` means "leave unchanged".
pub struct ProfilePatch {
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub interests: Option<Vec<String>>,
}

/// Update the mutable profile fields for a user.
///
/// Uses `COALESCE` so that `None` values leave the existing column intact.
/// Returns the updated [`Profile`], or [`AppError::NotFound`] when the id does not exist.
pub async fn update_profile(
    pool: &PgPool,
    id: Uuid,
    patch: ProfilePatch,
) -> Result<Profile, AppError> {
    sqlx::query_as::<_, Profile>(
        "UPDATE users \
         SET display_name = COALESCE($1, display_name), \
             avatar_url   = COALESCE($2, avatar_url), \
             bio          = COALESCE($3, bio), \
             interests    = COALESCE($4::text[], interests) \
         WHERE id = $5 \
         RETURNING id, email::text AS email, display_name, avatar_url, bio, \
                   interests, created_at, \
                   (email_verified_at IS NOT NULL) AS email_verified",
    )
    .bind(patch.display_name)
    .bind(patch.avatar_url)
    .bind(patch.bio)
    .bind(patch.interests)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?
    .ok_or(AppError::NotFound)
}
