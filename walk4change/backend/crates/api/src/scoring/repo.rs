use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, FieldError},
    scoring::{
        config::ScoringConfig,
        engine::{score_segment, SpatialInput},
    },
};

/// Input for scoring a single incoming GPS ping.
#[derive(Debug, Clone)]
pub struct PingInput {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub seq: i32,
    pub lat: f64,
    pub lng: f64,
    pub recorded_at: DateTime<Utc>,
}

/// Result of scoring a ping that was actually persisted.
#[derive(Debug, Clone)]
pub struct PingScore {
    pub seq: i32,
    pub lat: f64,
    pub lng: f64,
    /// Effective (speed-capped) distance credited for this segment.
    pub segment_meters: Decimal,
    pub companions: i32,
    pub nature_mult: Decimal,
    pub together_mult: Decimal,
    /// Final points after the per-second ceiling clamp.
    pub points: Decimal,
    pub participant_total: Decimal,
}

/// `dt` substituted when there is no previous ping, so the first ping of a
/// session is never treated as a teleport (huge gap → speed ≈ 0).
const NO_PREV_DT_SECS: f64 = 1.0e9;

/// Validate the request at the system boundary (cheap, no DB).
///
/// - `lat ∈ [-90, 90]`, `lng ∈ [-180, 180]`.
/// - `recorded_at` within `±cfg.recorded_at_tolerance_secs` of the server clock.
///   `recorded_at` is used only for sanity/ordering, never as the scoring clock.
fn validate(cfg: &ScoringConfig, input: &PingInput) -> Result<(), AppError> {
    let mut errors: Vec<FieldError> = Vec::new();

    if !(-90.0..=90.0).contains(&input.lat) {
        errors.push(FieldError {
            field: "lat".into(),
            message: "latitude must be between -90 and 90".into(),
            code: "OUT_OF_RANGE".into(),
        });
    }
    if !(-180.0..=180.0).contains(&input.lng) {
        errors.push(FieldError {
            field: "lng".into(),
            message: "longitude must be between -180 and 180".into(),
            code: "OUT_OF_RANGE".into(),
        });
    }

    let skew = (Utc::now() - input.recorded_at).num_seconds().abs();
    if skew > cfg.recorded_at_tolerance_secs {
        errors.push(FieldError {
            field: "recorded_at".into(),
            message: format!(
                "recorded_at is {skew}s from server time (max ±{}s)",
                cfg.recorded_at_tolerance_secs
            ),
            code: "CLOCK_SKEW".into(),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::Validation(errors))
    }
}

/// Score and persist a single GPS ping inside one DB transaction.
///
/// All clock-dependent reads (segment `dt`, companion window, per-second sum)
/// use the Postgres `now()` of the transaction, which is also the default
/// `received_at` of the inserted row — keeping every time comparison consistent
/// and unforgeable by the client.
///
/// Returns `Ok(None)` when the ping is a duplicate (same `(session_id,user_id,seq)`):
/// the insert is a no-op and no totals are touched (idempotent dedup).
pub async fn score_ping(
    pool: &PgPool,
    cfg: &ScoringConfig,
    input: PingInput,
) -> Result<Option<PingScore>, AppError> {
    validate(cfg, &input)?;

    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    // 1. Previous ping by seq (max seq < input.seq): distance + server-clock gap.
    let prev: Option<(f64, f64)> = sqlx::query_as(
        "SELECT \
            ST_Distance(geom, ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography)::float8 AS seg, \
            EXTRACT(EPOCH FROM (now() - received_at))::float8 AS dt \
         FROM location_pings \
         WHERE session_id = $1 AND user_id = $2 AND seq < $5 \
         ORDER BY seq DESC LIMIT 1",
    )
    .bind(input.session_id)
    .bind(input.user_id)
    .bind(input.lng)
    .bind(input.lat)
    .bind(input.seq)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    let (segment_meters, dt_secs) = match prev {
        Some((seg, dt)) => (seg, dt),
        None => (0.0, NO_PREV_DT_SECS),
    };

    // 2. Nature multiplier of the covering active zone (ST_Covers — geography).
    let nature_mult: Decimal = sqlx::query_scalar(
        "SELECT multiplier FROM nature_zones \
         WHERE active \
           AND ST_Covers(geom, ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography) \
         ORDER BY multiplier DESC LIMIT 1",
    )
    .bind(input.lng)
    .bind(input.lat)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::internal)?
    .unwrap_or(Decimal::ONE);

    // 3. Companions: other participants active within the ping window.
    let companions: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT user_id) FROM location_pings \
         WHERE session_id = $1 AND user_id <> $2 \
           AND received_at > now() - make_interval(secs => $3)",
    )
    .bind(input.session_id)
    .bind(input.user_id)
    .bind(cfg.ping_window_secs as f64)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::internal)?;
    let companions = companions as i32;

    // 4. Score the segment (teleport guard + multipliers).
    let scored = score_segment(
        cfg,
        &SpatialInput {
            segment_meters,
            dt_secs,
            nature_mult,
            companions,
        },
    );
    let effective_meters = scored.effective_meters;

    // 5. Per-second ceiling: clamp using points already awarded in the trailing
    //    1s window (computed BEFORE the insert so it excludes this ping).
    let awarded_last_sec: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(points), 0) FROM location_pings \
         WHERE user_id = $1 AND received_at > now() - interval '1 second'",
    )
    .bind(input.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    let points = if awarded_last_sec + scored.points > cfg.max_points_per_second {
        (cfg.max_points_per_second - awarded_last_sec).max(Decimal::ZERO)
    } else {
        scored.points
    };

    // 6. Insert the ping; duplicate (session,user,seq) → no-op, return None.
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO location_pings \
            (id, session_id, user_id, geom, recorded_at, seq, \
             segment_meters, companions, nature_mult, together_mult, points) \
         VALUES ($1, $2, $3, \
            ST_SetSRID(ST_MakePoint($4, $5), 4326)::geography, \
            $6, $7, $8, $9, $10, $11, $12) \
         ON CONFLICT (session_id, user_id, seq) DO NOTHING \
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(input.session_id)
    .bind(input.user_id)
    .bind(input.lng)
    .bind(input.lat)
    .bind(input.recorded_at)
    .bind(input.seq)
    .bind(effective_meters)
    .bind(companions)
    .bind(nature_mult)
    .bind(scored.together_mult)
    .bind(points)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    if inserted.is_none() {
        // Idempotent dedup: do not touch totals. Commit the (empty) tx.
        tx.commit().await.map_err(AppError::internal)?;
        return Ok(None);
    }

    // 7. Increment participant totals.
    let participant_total: Decimal = sqlx::query_scalar(
        "UPDATE walk_participants \
         SET total_meters = total_meters + $3, total_points = total_points + $4 \
         WHERE session_id = $1 AND user_id = $2 \
         RETURNING total_points",
    )
    .bind(input.session_id)
    .bind(input.user_id)
    .bind(effective_meters)
    .bind(points)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::internal("walk_participants row missing for scored ping"))?;

    // 8. Upsert user totals.
    sqlx::query(
        "INSERT INTO user_totals (user_id, total_points, total_meters) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (user_id) DO UPDATE SET \
            total_points = user_totals.total_points + $2, \
            total_meters = user_totals.total_meters + $3, \
            updated_at = now()",
    )
    .bind(input.user_id)
    .bind(points)
    .bind(effective_meters)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(Some(PingScore {
        seq: input.seq,
        lat: input.lat,
        lng: input.lng,
        segment_meters: effective_meters,
        companions,
        nature_mult,
        together_mult: scored.together_mult,
        points,
        participant_total,
    }))
}
