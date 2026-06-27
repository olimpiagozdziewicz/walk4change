use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppError, models::Profile};

/// Minimal user row used during authentication.
#[derive(sqlx::FromRow)]
pub struct UserAuthRow {
    pub id: Uuid,
    pub password_hash: String,
}

/// Insert a new user and their totals row in a single transaction.
///
/// Maps a unique-violation on the email column to [`AppError::Conflict`].
pub async fn create(
    pool: &PgPool,
    id: Uuid,
    email: &str,
    password_hash: &str,
    display_name: &str,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(email)
    .bind(password_hash)
    .bind(display_name)
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
         interests, created_at \
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?
    .ok_or(AppError::NotFound)
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
                   interests, created_at",
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
