//! RODO: account deletion (anonimizacja + twarde usunięcie danych wrażliwych)
//! i eksport danych (art. 20) — spec 2026-07-13.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Delete the user's account.
///
/// Hard-deletes personal data (GPS pings, messages, tokens, blocks,
/// friendships, ratings, eco reports + their likes/comments) and anonymises
/// the `users` row into a tombstone. The row itself must survive: no FK to
/// `users` has `ON DELETE`, and shared walk sessions belong to the other
/// participants too. After anonymisation the remaining aggregates
/// (`user_totals`, `walk_participants`, `reward_redemptions`) no longer
/// reference an identifiable person.
///
/// The freed e-mail can immediately register a fresh account.
pub async fn delete_account(pool: &PgPool, user: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    // Likes/comments hang off eco_reports without ON DELETE — clear the
    // user's own activity AND everything attached to the user's reports.
    for sql in [
        "DELETE FROM eco_likes WHERE user_id = $1 \
           OR report_id IN (SELECT id FROM eco_reports WHERE user_id = $1)",
        "DELETE FROM eco_comments WHERE user_id = $1 \
           OR report_id IN (SELECT id FROM eco_reports WHERE user_id = $1)",
        "DELETE FROM eco_reports WHERE user_id = $1",
        "DELETE FROM location_pings WHERE user_id = $1",
        "DELETE FROM messages WHERE sender_id = $1 OR recipient_id = $1",
        "DELETE FROM magic_links WHERE user_id = $1",
        "DELETE FROM email_verification_tokens WHERE user_id = $1",
        "DELETE FROM user_blocks WHERE blocker_id = $1 OR blocked_id = $1",
        "DELETE FROM friendships WHERE requester_id = $1 OR addressee_id = $1",
        "DELETE FROM walk_ratings WHERE rater_id = $1 OR rated_id = $1",
    ] {
        sqlx::query(sql)
            .bind(user)
            .execute(&mut *tx)
            .await
            .map_err(AppError::internal)?;
    }

    // Tombstone: unique non-personal e-mail (frees the original address),
    // no PII, unusable password hash, verification & consent reset.
    let rows = sqlx::query(
        "UPDATE users SET \
            email = 'deleted-' || left(id::text, 8) || '@anon.seasteps.pl', \
            display_name = 'Konto usunięte', \
            avatar_url = NULL, \
            bio = NULL, \
            interests = '{}', \
            password_hash = 'DELETED', \
            email_verified_at = NULL, \
            accepted_terms_at = NULL, \
            terms_version = NULL, \
            deleted_at = now() \
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(user)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound);
    }

    tx.commit().await.map_err(AppError::internal)?;
    Ok(())
}

/// One export section: `SELECT json_agg(...) ... ::text` parsed into a Value.
async fn section(pool: &PgPool, sql: &str, user: Uuid) -> Result<serde_json::Value, AppError> {
    let text: String = sqlx::query_scalar(sql)
        .bind(user)
        .fetch_one(pool)
        .await
        .map_err(AppError::internal)?;
    serde_json::from_str(&text).map_err(AppError::internal)
}

/// Full personal-data export (art. 20 RODO).
///
/// Received ratings are stripped to `recommend` + timestamp: the author,
/// flag and comment are the OTHER person's data and at small scale would
/// identify the rater (same reasoning as the ≥3-ratings visibility gate).
pub async fn export(pool: &PgPool, user: Uuid) -> Result<serde_json::Value, AppError> {
    let profile = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT id, email::text AS email, display_name, avatar_url, bio, \
                   interests, created_at, email_verified_at, \
                   accepted_terms_at, terms_version \
            FROM users WHERE id = $1) t",
        user,
    )
    .await?;

    let totals = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT total_points, spent_points, total_meters, total_walks, \
                   updated_at \
            FROM user_totals WHERE user_id = $1) t",
        user,
    )
    .await?;

    let walks = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT s.id AS session_id, s.host_id, s.status, s.is_open, \
                   s.started_at, s.ended_at, p.joined_at, p.left_at, \
                   p.total_meters, p.total_points \
            FROM walk_participants p \
            JOIN walk_sessions s ON s.id = p.session_id \
            WHERE p.user_id = $1 \
            ORDER BY s.started_at) t",
        user,
    )
    .await?;

    let location_pings = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT session_id, recorded_at, \
                   ST_Y(geom::geometry) AS lat, ST_X(geom::geometry) AS lng, \
                   segment_meters, points \
            FROM location_pings WHERE user_id = $1 \
            ORDER BY recorded_at) t",
        user,
    )
    .await?;

    let messages = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT id, sender_id, recipient_id, body, created_at, read_at \
            FROM messages \
            WHERE sender_id = $1 OR recipient_id = $1 \
            ORDER BY created_at) t",
        user,
    )
    .await?;

    let friendships = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT requester_id, addressee_id, status, created_at \
            FROM friendships \
            WHERE requester_id = $1 OR addressee_id = $1) t",
        user,
    )
    .await?;

    let ratings_given = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT session_id, rated_id, recommend, flag, comment, created_at \
            FROM walk_ratings WHERE rater_id = $1) t",
        user,
    )
    .await?;

    let ratings_received = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT recommend, created_at \
            FROM walk_ratings WHERE rated_id = $1) t",
        user,
    )
    .await?;

    let redemptions = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT reward_id, points_spent, code, status, created_at, \
                   redeemed_at \
            FROM reward_redemptions WHERE user_id = $1) t",
        user,
    )
    .await?;

    let eco_reports = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT id, kind, category, description, location, status, \
                   photo_url, photo_before_url, photo_after_url, created_at \
            FROM eco_reports WHERE user_id = $1) t",
        user,
    )
    .await?;

    let blocked_users = section(
        pool,
        "SELECT COALESCE(json_agg(t), '[]')::text FROM ( \
            SELECT blocked_id, created_at \
            FROM user_blocks WHERE blocker_id = $1) t",
        user,
    )
    .await?;

    Ok(serde_json::json!({
        "format": "seasteps-export",
        "version": 1,
        "profile": profile,
        "totals": totals,
        "walks": walks,
        "location_pings": location_pings,
        "messages": messages,
        "friendships": friendships,
        "ratings_given": ratings_given,
        "ratings_received": ratings_received,
        "reward_redemptions": redemptions,
        "eco_reports": eco_reports,
        "blocked_users": blocked_users,
    }))
}
