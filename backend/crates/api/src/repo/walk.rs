use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{ParticipantInfo, PingPoint, WalkDetail, WalkSession},
    repo::friend,
};

/// Base-32 alphabet (RFC 4648).
const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Maximum rows returned by [`track`], regardless of the client-supplied
/// `limit`. The default request (no `limit` param) asks for 1000, so the cap
/// is kept well above that to avoid changing normal behaviour — it only
/// stops a negative/huge `limit` from reaching the SQL `LIMIT` clause
/// unbounded (security hardening 2026-07-09).
const MAX_TRACK_LIMIT: i64 = 2000;

/// Maximum number of concurrently active (non-left) participants allowed in
/// a single walk session when joining via [`join_by_code`] (no friendship
/// gate). Generous on purpose — the real two-phone/group demo is far below
/// this — but stops one actor from stuffing dozens of sockpuppet accounts
/// into a session via a leaked/guessed join code (security hardening
/// 2026-07-09).
const MAX_ACTIVE_PARTICIPANTS_PER_SESSION: i64 = 10;

/// Generate a random 8-character base-32 join code.
fn random_join_code() -> String {
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..BASE32_ALPHABET.len());
            BASE32_ALPHABET[idx] as char
        })
        .collect()
}

/// Thin struct for fetching host_id + status in a single query.
#[derive(sqlx::FromRow)]
struct SessionRow {
    host_id: Uuid,
    status: String,
}

/// Insert a new `active` walk session with a random `join_code`, and add the host as the
/// first participant — all in a single transaction.
pub async fn start(
    pool: &PgPool,
    session_id: Uuid,
    host_id: Uuid,
) -> Result<WalkSession, AppError> {
    let join_code = random_join_code();
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    let session: WalkSession = sqlx::query_as(
        "INSERT INTO walk_sessions (id, host_id, status, join_code) \
         VALUES ($1, $2, 'active', $3) \
         RETURNING id, host_id, status, join_code, started_at, ended_at",
    )
    .bind(session_id)
    .bind(host_id)
    .bind(&join_code)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    let participant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO walk_participants (id, session_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(participant_id)
    .bind(session_id)
    .bind(host_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(session)
}

/// Join an active walk session as `actor`.
///
/// Errors:
/// - Session not found or not `active` → 404.
/// - `actor` is not friends with the host → 403.
/// - `actor` already joined (UNIQUE violation) → 409.
pub async fn join(pool: &PgPool, session_id: Uuid, actor: Uuid) -> Result<(), AppError> {
    let row: Option<SessionRow> = sqlx::query_as(
        "SELECT host_id, status FROM walk_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    let SessionRow { host_id, status } = row.ok_or(AppError::NotFound)?;
    if status != "active" {
        return Err(AppError::NotFound);
    }

    if !friend::are_friends(pool, host_id, actor).await? {
        return Err(AppError::Forbidden);
    }

    let participant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO walk_participants (id, session_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(participant_id)
    .bind(session_id)
    .bind(actor)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db) = e {
            if db.is_unique_violation() {
                return AppError::Conflict("already_joined".into());
            }
        }
        AppError::internal(e)
    })?;

    Ok(())
}

/// Join an active walk session by its `join_code`, WITHOUT requiring friendship.
///
/// This powers the "join by code" demo flow where two strangers pair via a
/// short code instead of a pre-existing friendship. Returns the session id so
/// the caller can subscribe to its live feed.
///
/// Errors:
/// - No active session with that code → 404.
/// - `actor` already joined (UNIQUE violation) → idempotent success (returns id).
pub async fn join_by_code(pool: &PgPool, code: &str, actor: Uuid) -> Result<Uuid, AppError> {
    // A join code is only valid while the session is active AND started within
    // the last 24h. This bounds the window in which a leaked code grants access
    // to the session's live feed and GPS track (security audit 2026-07-08).
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM walk_sessions \
         WHERE join_code = $1 AND status = 'active' \
           AND started_at > now() - interval '24 hours'",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    let (session_id,) = row.ok_or(AppError::NotFound)?;

    // Idempotent fast path: an actor who is already an active participant may
    // "rejoin" via the same code without being counted against the cap below
    // (the INSERT further down will just hit the UNIQUE violation and return
    // the session id as before).
    let already_active: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 FROM walk_participants \
            WHERE session_id = $1 AND user_id = $2 AND left_at IS NULL \
        )",
    )
    .bind(session_id)
    .bind(actor)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    if !already_active {
        // Cap active participants per session (security audit 2026-07-09):
        // prevents one actor from farming a leaked join code with dozens of
        // sockpuppet accounts. Generous limit — real demo groups are tiny.
        let active_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM walk_participants \
             WHERE session_id = $1 AND left_at IS NULL",
        )
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(AppError::internal)?;

        if active_count >= MAX_ACTIVE_PARTICIPANTS_PER_SESSION {
            return Err(AppError::Conflict("session_full".into()));
        }
    }

    let participant_id = Uuid::new_v4();
    let res = sqlx::query(
        "INSERT INTO walk_participants (id, session_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(participant_id)
    .bind(session_id)
    .bind(actor)
    .execute(pool)
    .await;

    if let Err(e) = res {
        // Already a participant → idempotent: still return the session id.
        if let sqlx::Error::Database(ref db) = e {
            if db.is_unique_violation() {
                return Ok(session_id);
            }
        }
        return Err(AppError::internal(e));
    }

    Ok(session_id)
}

/// Set `left_at = now()` for `actor` in `session_id`.
///
/// - 0 rows AND never a member → 404.
/// - 0 rows AND already left → idempotent success (204).
pub async fn leave(pool: &PgPool, session_id: Uuid, actor: Uuid) -> Result<(), AppError> {
    let rows = sqlx::query(
        "UPDATE walk_participants SET left_at = now() \
         WHERE session_id = $1 AND user_id = $2 AND left_at IS NULL",
    )
    .bind(session_id)
    .bind(actor)
    .execute(pool)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if rows == 0 {
        // Distinguish "never a member" (404) from "already left" (idempotent 204).
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
                SELECT 1 FROM walk_participants \
                WHERE session_id = $1 AND user_id = $2 \
            )",
        )
        .bind(session_id)
        .bind(actor)
        .fetch_one(pool)
        .await
        .map_err(AppError::internal)?;

        if !is_member {
            return Err(AppError::NotFound);
        }
        // else: already left — idempotent, return Ok
    }

    Ok(())
}

/// Stop an active walk session (host only).
///
/// In a single transaction:
/// 1. Sets `status = 'finished'` and `ended_at = now()` (host-only guard via `host_id`).
/// 2. Sets `left_at = now()` for all participants still in the session.
/// 3. Bumps `user_totals.total_walks` by 1 for every participant (including those who left).
///
/// Returns 403 if `actor` is not the host or the session is not active.
pub async fn stop(pool: &PgPool, session_id: Uuid, actor: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    let rows = sqlx::query(
        "UPDATE walk_sessions \
         SET status = 'finished', ended_at = now() \
         WHERE id = $1 AND host_id = $2 AND status = 'active'",
    )
    .bind(session_id)
    .bind(actor)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::Forbidden);
    }

    // Close any participants still in the session.
    sqlx::query(
        "UPDATE walk_participants SET left_at = now() \
         WHERE session_id = $1 AND left_at IS NULL",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    // Bump total_walks for EVERY participant of the session (left or not).
    sqlx::query(
        "INSERT INTO user_totals (user_id, total_walks) \
         SELECT user_id, 1 FROM walk_participants WHERE session_id = $1 \
         ON CONFLICT (user_id) \
         DO UPDATE SET total_walks = user_totals.total_walks + 1, updated_at = now()",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(())
}

/// Fetch full walk detail (session + participants) for a member.
///
/// Returns 403 if `actor` has never been a participant (left or not).
pub async fn get(
    pool: &PgPool,
    session_id: Uuid,
    actor: Uuid,
) -> Result<WalkDetail, AppError> {
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 FROM walk_participants \
            WHERE session_id = $1 AND user_id = $2 \
        )",
    )
    .bind(session_id)
    .bind(actor)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    if !is_member {
        return Err(AppError::Forbidden);
    }

    let session: Option<WalkSession> = sqlx::query_as(
        "SELECT id, host_id, status, join_code, started_at, ended_at \
         FROM walk_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    let session = session.ok_or(AppError::NotFound)?;

    let participants: Vec<ParticipantInfo> = sqlx::query_as(
        "SELECT id, session_id, user_id, joined_at, left_at, total_meters, total_points \
         FROM walk_participants WHERE session_id = $1 ORDER BY joined_at",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(WalkDetail { session, participants })
}

/// Fetch ordered location pings for all participants in a session.
///
/// Returns 403 if `actor` has never been a participant.
/// Results are ordered by `seq` and capped at `limit` rows.
pub async fn track(
    pool: &PgPool,
    session_id: Uuid,
    actor: Uuid,
    limit: i64,
) -> Result<Vec<PingPoint>, AppError> {
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 FROM walk_participants \
            WHERE session_id = $1 AND user_id = $2 \
        )",
    )
    .bind(session_id)
    .bind(actor)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    if !is_member {
        return Err(AppError::Forbidden);
    }

    // Clamp the client-supplied limit before it reaches SQL: a negative value
    // would error, and an unbounded value would let one request pull the
    // entire ping history for a session (security hardening 2026-07-09).
    let limit = limit.clamp(1, MAX_TRACK_LIMIT);

    let pings: Vec<PingPoint> = sqlx::query_as(
        "SELECT user_id, seq, \
                ST_Y(geom::geometry) AS lat, \
                ST_X(geom::geometry) AS lng, \
                points, recorded_at \
         FROM location_pings \
         WHERE session_id = $1 \
         ORDER BY seq \
         LIMIT $2",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(pings)
}

/// Returns `true` if `actor` is currently an active (non-left) participant of an active session.
///
/// Used by the WebSocket ping authorisation layer (Task 15).
pub async fn is_active_participant(
    pool: &PgPool,
    session_id: Uuid,
    actor: Uuid,
) -> Result<bool, AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 \
            FROM walk_sessions ws \
            JOIN walk_participants wp ON wp.session_id = ws.id \
            WHERE ws.id = $1 \
              AND ws.status = 'active' \
              AND wp.user_id = $2 \
              AND wp.left_at IS NULL \
        )",
    )
    .bind(session_id)
    .bind(actor)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(exists)
}

/// Returns `true` if `actor` has ever been a participant of `session_id` (left or not).
///
/// Used by the WebSocket subscribe authorisation layer (Task 15).
pub async fn is_member(
    pool: &PgPool,
    session_id: Uuid,
    actor: Uuid,
) -> Result<bool, AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 FROM walk_participants \
            WHERE session_id = $1 AND user_id = $2 \
        )",
    )
    .bind(session_id)
    .bind(actor)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(exists)
}
