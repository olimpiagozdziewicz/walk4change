use chrono::{DateTime, Utc};
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, FieldError},
    models::{MyWalk, OpenWalk, ParticipantInfo, PingPoint, WalkDetail, WalkSession},
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

/// Thin struct for fetching host_id + status (+ open flag) in a single query.
#[derive(sqlx::FromRow)]
struct SessionRow {
    host_id: Uuid,
    status: String,
    is_open: bool,
}

/// Insert a new `active` walk session with a random `join_code`, and add the host as the
/// first participant — all in a single transaction.
///
/// `is_open` + `open_note` power the "spaceruję — dołącz" opt-in: an open
/// session is listed publicly via [`open_walks`] and joinable without friendship.
pub async fn start(
    pool: &PgPool,
    session_id: Uuid,
    host_id: Uuid,
    is_open: bool,
    open_note: Option<&str>,
) -> Result<WalkSession, AppError> {
    let open_note = open_note.map(str::trim).filter(|s| !s.is_empty());
    if let Some(note) = open_note {
        if note.chars().count() > 200 {
            return Err(AppError::Validation(vec![FieldError {
                field: "open_note".into(),
                message: "note must be at most 200 characters".into(),
                code: "INVALID_LENGTH".into(),
            }]));
        }
    }

    let join_code = random_join_code();
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    let session: WalkSession = sqlx::query_as(
        "INSERT INTO walk_sessions (id, host_id, status, join_code, is_open, open_note) \
         VALUES ($1, $2, 'active', $3, $4, $5) \
         RETURNING id, host_id, status, join_code, started_at, ended_at, is_open, open_note",
    )
    .bind(session_id)
    .bind(host_id)
    .bind(&join_code)
    .bind(is_open)
    .bind(open_note)
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

/// Insert-or-rejoin a participant inside an open transaction, enforcing the
/// active-participant cap. Locks the session row (`FOR UPDATE`) first, so
/// parallel joins serialize and cannot overshoot the cap (audit B3.3).
///
/// Rejoin-after-leave clears `left_at` (audit N1 — previously a former
/// participant could never return: INSERT hit the UNIQUE constraint).
///
/// Returns `Conflict("already_joined")` when the actor is already active,
/// `Conflict("session_full")` at the cap, `NotFound` when the session is gone
/// or no longer active.
async fn join_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: Uuid,
    actor: Uuid,
) -> Result<(), AppError> {
    let status: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM walk_sessions WHERE id = $1 FOR UPDATE",
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::internal)?;

    match status {
        None => return Err(AppError::NotFound),
        Some((st,)) if st != "active" => return Err(AppError::NotFound),
        _ => {}
    }

    // Wyrzucony przez hosta nie wraca do TEJ sesji (kick, audyt B3.1) —
    // rejoin-po-leave (N1) zostaje możliwy tylko dla nie-wyrzuconych.
    let kicked: Option<bool> = sqlx::query_scalar(
        "SELECT kicked_at IS NOT NULL FROM walk_participants \
         WHERE session_id = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(actor)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::internal)?;

    if kicked == Some(true) {
        return Err(AppError::Forbidden);
    }

    // Cap liczony bez samego aktora (jego powrót nie zjada miejsca).
    let active_others: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM walk_participants \
         WHERE session_id = $1 AND left_at IS NULL AND user_id <> $2",
    )
    .bind(session_id)
    .bind(actor)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::internal)?;

    if active_others >= MAX_ACTIVE_PARTICIPANTS_PER_SESSION {
        return Err(AppError::Conflict("session_full".into()));
    }

    // Nowy uczestnik → INSERT; powrót po leave → left_at=NULL;
    // już aktywny → 0 wierszy (WHERE odfiltruje) → already_joined.
    let rows = sqlx::query(
        "INSERT INTO walk_participants (id, session_id, user_id) VALUES ($1, $2, $3) \
         ON CONFLICT (session_id, user_id) \
         DO UPDATE SET left_at = NULL \
         WHERE walk_participants.left_at IS NOT NULL",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(actor)
    .execute(&mut **tx)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::Conflict("already_joined".into()));
    }
    Ok(())
}

/// Join an active walk session as `actor`.
///
/// Friendship with the host is required UNLESS the session is open
/// ("spaceruję — dołącz" opt-in) or the actor is a returning participant.
/// Cap + rejoin handled transactionally in [`join_in_tx`].
///
/// Errors:
/// - Session not found or not `active` → 404.
/// - closed session, stranger and not friends with the host → 403.
/// - session at the participant cap → 409 `session_full`.
/// - `actor` already an active participant → 409 `already_joined`.
pub async fn join(pool: &PgPool, session_id: Uuid, actor: Uuid) -> Result<(), AppError> {
    let row: Option<SessionRow> = sqlx::query_as(
        "SELECT host_id, status, is_open FROM walk_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    let SessionRow { host_id, status, is_open } = row.ok_or(AppError::NotFound)?;
    if status != "active" {
        return Err(AppError::NotFound);
    }

    // Block gate: osoba zablokowana (w dowolną stronę) nie dołącza do spaceru
    // hosta — także otwartego (audyt 2026-07-10, wektor nękania przez open walks).
    if crate::repo::block::is_blocked_either(pool, host_id, actor).await? {
        return Err(AppError::Forbidden);
    }

    if !is_open {
        // Powracający uczestnik był już wpuszczony — gate tylko dla nowych.
        let was_member: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
            "SELECT left_at FROM walk_participants WHERE session_id = $1 AND user_id = $2",
        )
        .bind(session_id)
        .bind(actor)
        .fetch_optional(pool)
        .await
        .map_err(AppError::internal)?;

        if was_member.is_none() && !friend::are_friends(pool, host_id, actor).await? {
            return Err(AppError::Forbidden);
        }
    }

    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    join_in_tx(&mut tx, session_id, actor).await?;
    tx.commit().await.map_err(AppError::internal)?;
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
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, host_id FROM walk_sessions \
         WHERE join_code = $1 AND status = 'active' \
           AND started_at > now() - interval '24 hours'",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    let (session_id, host_id) = row.ok_or(AppError::NotFound)?;

    // Block gate — jak w [`join`]: kod nie omija blokady hosta.
    if crate::repo::block::is_blocked_either(pool, host_id, actor).await? {
        return Err(AppError::Forbidden);
    }

    // Wspólna transakcyjna ścieżka: cap bez wyścigu + rejoin po leave
    // (audyt B3.3 + N1). Join-by-code jest idempotentny: "already_joined"
    // zamieniamy na sukces ze zwrotem id sesji.
    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    match join_in_tx(&mut tx, session_id, actor).await {
        Ok(()) => {}
        Err(AppError::Conflict(ref code)) if code == "already_joined" => {}
        Err(e) => return Err(e),
    }
    tx.commit().await.map_err(AppError::internal)?;

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

/// Kick `target` from an active session (host only; audit B3.1).
///
/// Sets `left_at` (if still in) AND `kicked_at`, which permanently bars the
/// target from re-joining THIS session (the rejoin path checks `kicked_at`).
/// The WS forwarder cuts the live stream within its re-check interval.
///
/// Errors: 422 kicking yourself; 403 not the host / session not active;
/// 404 target was never a participant.
pub async fn kick(
    pool: &PgPool,
    session_id: Uuid,
    host: Uuid,
    target: Uuid,
) -> Result<(), AppError> {
    if host == target {
        return Err(AppError::Validation(vec![FieldError {
            field: "user_id".into(),
            message: "host cannot kick themselves — stop the walk instead".into(),
            code: "SELF_KICK".into(),
        }]));
    }

    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT host_id, status FROM walk_sessions WHERE id = $1 FOR UPDATE",
    )
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    match row {
        Some((host_id, status)) if host_id == host && status == "active" => {}
        Some(_) => return Err(AppError::Forbidden),
        None => return Err(AppError::NotFound),
    }

    let rows = sqlx::query(
        "UPDATE walk_participants \
         SET left_at = COALESCE(left_at, now()), kicked_at = now() \
         WHERE session_id = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(target)
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
        "SELECT id, host_id, status, join_code, started_at, ended_at, is_open, open_note \
         FROM walk_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::internal)?;

    let mut session = session.ok_or(AppError::NotFound)?;

    // join_code = klucz dystrybucji sesji; dostaje go tylko host (audyt B3.2 —
    // obcy z otwartego spaceru nie może rozsyłać kodu dalej).
    if session.host_id != actor {
        session.join_code = None;
    }

    // display_name w odpowiedzi: host (i uczestnicy) widzą KTO dołączył —
    // minimalna osłona open walks przed anonimowym dołączającym (audyt B3.1).
    let participants: Vec<ParticipantInfo> = sqlx::query_as(
        "SELECT wp.id, wp.session_id, wp.user_id, u.display_name, \
                wp.joined_at, wp.left_at, wp.total_meters, wp.total_points \
         FROM walk_participants wp \
         JOIN users u ON u.id = wp.user_id \
         WHERE wp.session_id = $1 ORDER BY wp.joined_at",
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

/// Toggle the "spaceruję — dołącz" visibility of an active session (host only).
///
/// `open_note = None` keeps the existing note. Returns 403 when the actor is
/// not the host or the session is no longer active (audit B3.4 — previously
/// the only way to go invisible was stopping the walk).
pub async fn set_open(
    pool: &PgPool,
    session_id: Uuid,
    host: Uuid,
    is_open: bool,
    open_note: Option<&str>,
) -> Result<(), AppError> {
    let open_note = open_note.map(str::trim).filter(|s| !s.is_empty());
    if let Some(note) = open_note {
        if note.chars().count() > 200 {
            return Err(AppError::Validation(vec![FieldError {
                field: "open_note".into(),
                message: "note must be at most 200 characters".into(),
                code: "INVALID_LENGTH".into(),
            }]));
        }
    }

    let rows = sqlx::query(
        "UPDATE walk_sessions \
         SET is_open = $3, \
             open_note = CASE WHEN $4::text IS NOT NULL THEN $4 ELSE open_note END \
         WHERE id = $1 AND host_id = $2 AND status = 'active'",
    )
    .bind(session_id)
    .bind(host)
    .bind(is_open)
    .bind(open_note)
    .execute(pool)
    .await
    .map_err(AppError::internal)?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

/// List currently-active open walks ("spaceruję — dołącz"), newest first.
///
/// Public within the app (any authenticated user). Sessions older than 12h
/// are hidden even if never stopped, so stale sessions do not linger.
pub async fn open_walks(pool: &PgPool) -> Result<Vec<OpenWalk>, AppError> {
    let walks: Vec<OpenWalk> = sqlx::query_as(
        "SELECT ws.id AS session_id, ws.host_id, u.display_name AS host_name, \
                ws.open_note, ws.started_at, \
                (SELECT count(*) FROM walk_participants wp \
                  WHERE wp.session_id = ws.id AND wp.left_at IS NULL) AS participants \
         FROM walk_sessions ws \
         JOIN users u ON u.id = ws.host_id \
         WHERE ws.is_open AND ws.status = 'active' \
           AND ws.started_at > now() - interval '12 hours' \
         ORDER BY ws.started_at DESC \
         LIMIT 50",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(walks)
}

/// List the caller's finished walks, newest first (server-side walk history).
pub async fn my_walks(pool: &PgPool, actor: Uuid, limit: i64) -> Result<Vec<MyWalk>, AppError> {
    let limit = limit.clamp(1, 100);

    let walks: Vec<MyWalk> = sqlx::query_as(
        "SELECT ws.id AS session_id, ws.started_at, ws.ended_at, \
                wp.total_meters, wp.total_points, \
                (ws.host_id = $1) AS is_host, \
                (SELECT count(*) - 1 FROM walk_participants w2 \
                  WHERE w2.session_id = ws.id) AS companions \
         FROM walk_participants wp \
         JOIN walk_sessions ws ON ws.id = wp.session_id \
         WHERE wp.user_id = $1 AND ws.status = 'finished' \
         ORDER BY ws.started_at DESC \
         LIMIT $2",
    )
    .bind(actor)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(walks)
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
