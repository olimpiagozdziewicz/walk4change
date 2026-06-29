mod common;

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;
use walk4change_api::scoring::{
    config::ScoringConfig,
    repo::{score_ping, PingInput},
};

// ── seeding helpers ──────────────────────────────────────────────────────────

async fn seed_user(pool: &PgPool, email: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name) \
         VALUES ($1, $2, 'x', 'Tester')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("seed user");
    id
}

async fn seed_session(pool: &PgPool, host: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO walk_sessions (id, host_id, status) VALUES ($1, $2, 'active')",
    )
    .bind(id)
    .bind(host)
    .execute(pool)
    .await
    .expect("seed session");
    id
}

async fn seed_participant(pool: &PgPool, session: Uuid, user: Uuid) {
    sqlx::query(
        "INSERT INTO walk_participants (id, session_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(Uuid::new_v4())
    .bind(session)
    .bind(user)
    .execute(pool)
    .await
    .expect("seed participant");
}

/// Seed a rectangular active nature zone covering a small area around (lng,lat) origin.
async fn seed_nature_zone(pool: &PgPool, multiplier: Decimal) {
    // Box from (17.99,49.99) to (18.01,50.01) — covers the ~1 km test area.
    sqlx::query(
        "INSERT INTO nature_zones (id, name, geom, multiplier, active) \
         VALUES ($1, 'Test Park', \
            ST_GeogFromText('SRID=4326;POLYGON((17.99 49.99, 18.01 49.99, 18.01 50.01, 17.99 50.01, 17.99 49.99))'), \
            $2, true)",
    )
    .bind(Uuid::new_v4())
    .bind(multiplier)
    .execute(pool)
    .await
    .expect("seed nature zone");
}

/// Seed a location ping with controlled `received_at = now() - offset_secs` and `points`.
#[allow(clippy::too_many_arguments)]
async fn seed_ping(
    pool: &PgPool,
    session: Uuid,
    user: Uuid,
    seq: i32,
    lng: f64,
    lat: f64,
    received_offset_secs: f64,
    points: Decimal,
) {
    sqlx::query(
        "INSERT INTO location_pings \
            (id, session_id, user_id, geom, recorded_at, received_at, seq, points) \
         VALUES ($1, $2, $3, \
            ST_SetSRID(ST_MakePoint($4, $5), 4326)::geography, \
            now(), now() - make_interval(secs => $6), $7, $8)",
    )
    .bind(Uuid::new_v4())
    .bind(session)
    .bind(user)
    .bind(lng)
    .bind(lat)
    .bind(received_offset_secs)
    .bind(seq)
    .bind(points)
    .execute(pool)
    .await
    .expect("seed ping");
}

fn n(d: Decimal) -> Decimal {
    d.normalize()
}

const ORIGIN_LNG: f64 = 18.0;
const ORIGIN_LAT: f64 = 50.0;
// ~100 m east at latitude 50° (1° lng ≈ 71.7 km → 100 m ≈ 0.001395°).
const EAST_100M_LNG: f64 = 18.001_395;

// ── tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn first_ping_has_no_prev_and_scores_zero() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default();
    let user = seed_user(&app.pool, "score_first@example.com").await;
    let session = seed_session(&app.pool, user).await;
    seed_participant(&app.pool, session, user).await;

    let out = score_ping(
        &app.pool,
        &cfg,
        PingInput {
            session_id: session,
            user_id: user,
            seq: 1,
            lat: ORIGIN_LAT,
            lng: ORIGIN_LNG,
            recorded_at: chrono::Utc::now(),
            accuracy: None,
        },
    )
    .await
    .expect("score_ping ok")
    .expect("first ping should be inserted");

    assert_eq!(n(out.segment_meters), Decimal::ZERO, "no prev → 0 m");
    assert_eq!(n(out.points), Decimal::ZERO, "no prev → 0 points");
    assert_eq!(n(out.participant_total), Decimal::ZERO);

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM location_pings WHERE session_id = $1")
            .bind(session)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "ping must be persisted");
}

#[tokio::test]
async fn second_ping_in_nature_zone_scores() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default();
    let user = seed_user(&app.pool, "score_nature@example.com").await;
    let session = seed_session(&app.pool, user).await;
    seed_participant(&app.pool, session, user).await;
    seed_nature_zone(&app.pool, Decimal::new(3, 0)).await;

    // prev ping 100s ago at origin → dt ≈ 100s, speed ≈ 1 m/s (under cap).
    seed_ping(&app.pool, session, user, 1, ORIGIN_LNG, ORIGIN_LAT, 100.0, Decimal::ZERO).await;

    let out = score_ping(
        &app.pool,
        &cfg,
        PingInput {
            session_id: session,
            user_id: user,
            seq: 2,
            lat: ORIGIN_LAT,
            lng: EAST_100M_LNG,
            recorded_at: chrono::Utc::now(),
            accuracy: None,
        },
    )
    .await
    .expect("score_ping ok")
    .expect("ping inserted");

    assert_eq!(n(out.nature_mult), Decimal::new(3, 0), "inside 3x zone");
    assert_eq!(n(out.together_mult), Decimal::new(1, 0), "solo");
    assert!(
        out.segment_meters > Decimal::new(80, 0) && out.segment_meters < Decimal::new(120, 0),
        "segment should be ~100 m, got {}",
        out.segment_meters
    );
    // points == (segment/100) * 3 * 1
    let expected = out.segment_meters / Decimal::new(100, 0) * Decimal::new(3, 0);
    assert_eq!(n(out.points), n(expected));
    assert_eq!(n(out.participant_total), n(out.points));

    let user_total: Decimal =
        sqlx::query_scalar("SELECT total_points FROM user_totals WHERE user_id = $1")
            .bind(user)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(n(user_total), n(out.points), "user_totals upserted");
}

#[tokio::test]
async fn teleport_scores_zero() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default();
    let user = seed_user(&app.pool, "score_teleport@example.com").await;
    let session = seed_session(&app.pool, user).await;
    seed_participant(&app.pool, session, user).await;

    // prev ping 1s ago at origin; new ping ~100 km away → speed >> cap.
    seed_ping(&app.pool, session, user, 1, ORIGIN_LNG, ORIGIN_LAT, 1.0, Decimal::ZERO).await;

    let out = score_ping(
        &app.pool,
        &cfg,
        PingInput {
            session_id: session,
            user_id: user,
            seq: 2,
            lat: ORIGIN_LAT,
            lng: 19.4, // ~100 km east
            recorded_at: chrono::Utc::now(),
            accuracy: None,
        },
    )
    .await
    .expect("score_ping ok")
    .expect("ping inserted");

    assert_eq!(n(out.segment_meters), Decimal::ZERO, "teleport → 0 effective m");
    assert_eq!(n(out.points), Decimal::ZERO, "teleport → 0 points");
}

#[tokio::test]
async fn duplicate_seq_does_not_double_count() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default();
    let user = seed_user(&app.pool, "score_dup@example.com").await;
    let session = seed_session(&app.pool, user).await;
    seed_participant(&app.pool, session, user).await;
    seed_nature_zone(&app.pool, Decimal::new(3, 0)).await;
    seed_ping(&app.pool, session, user, 1, ORIGIN_LNG, ORIGIN_LAT, 100.0, Decimal::ZERO).await;

    let input = PingInput {
        session_id: session,
        user_id: user,
        seq: 2,
        lat: ORIGIN_LAT,
        lng: EAST_100M_LNG,
        recorded_at: chrono::Utc::now(),
    };

    let first = score_ping(&app.pool, &cfg, input.clone())
        .await
        .expect("ok")
        .expect("inserted");
    assert!(first.points > Decimal::ZERO);

    let total_after_first: Decimal =
        sqlx::query_scalar("SELECT total_points FROM walk_participants WHERE session_id=$1 AND user_id=$2")
            .bind(session)
            .bind(user)
            .fetch_one(&app.pool)
            .await
            .unwrap();

    let second = score_ping(&app.pool, &cfg, input).await.expect("ok");
    assert!(second.is_none(), "duplicate seq must return None");

    let total_after_dup: Decimal =
        sqlx::query_scalar("SELECT total_points FROM walk_participants WHERE session_id=$1 AND user_id=$2")
            .bind(session)
            .bind(user)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(n(total_after_first), n(total_after_dup), "no double count");
}

#[tokio::test]
async fn companions_apply_together_multiplier() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default();
    let actor = seed_user(&app.pool, "score_actor@example.com").await;
    let friend = seed_user(&app.pool, "score_friend@example.com").await;
    let session = seed_session(&app.pool, actor).await;
    seed_participant(&app.pool, session, actor).await;
    seed_participant(&app.pool, session, friend).await;

    // friend has a recent ping (within window) → counts as a companion.
    seed_ping(&app.pool, session, friend, 1, ORIGIN_LNG, ORIGIN_LAT, 0.0, Decimal::ZERO).await;
    // actor prev ping 100s ago.
    seed_ping(&app.pool, session, actor, 1, ORIGIN_LNG, ORIGIN_LAT, 100.0, Decimal::ZERO).await;

    let out = score_ping(
        &app.pool,
        &cfg,
        PingInput {
            session_id: session,
            user_id: actor,
            seq: 2,
            lat: ORIGIN_LAT,
            lng: EAST_100M_LNG,
            recorded_at: chrono::Utc::now(),
            accuracy: None,
        },
    )
    .await
    .expect("ok")
    .expect("inserted");

    assert!(out.companions >= 1, "friend should count as companion");
    assert_eq!(n(out.together_mult), Decimal::new(15, 1), "pair → 1.5x");
}

#[tokio::test]
async fn per_second_ceiling_clamps_points() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default(); // max_points_per_second = 5.0
    let user = seed_user(&app.pool, "score_ceiling@example.com").await;
    let session = seed_session(&app.pool, user).await;
    seed_participant(&app.pool, session, user).await;
    seed_nature_zone(&app.pool, Decimal::new(3, 0)).await;

    // Distance-prev: seq 1, 100s ago (outside the 1s sum window → contributes 0).
    seed_ping(&app.pool, session, user, 1, ORIGIN_LNG, ORIGIN_LAT, 100.0, Decimal::ZERO).await;
    // Filler with a HIGHER seq so it is NOT chosen as prev, but recent (within 1s)
    // so it counts toward awarded_last_sec = 4.8.
    seed_ping(&app.pool, session, user, 100, ORIGIN_LNG, ORIGIN_LAT, 0.0, Decimal::new(48, 1)).await;

    // Raw points would be ~3.0 (100m/100 * 3 * solo); 4.8 + 3.0 > 5.0 → clamp to 0.2.
    let out = score_ping(
        &app.pool,
        &cfg,
        PingInput {
            session_id: session,
            user_id: user,
            seq: 2,
            lat: ORIGIN_LAT,
            lng: EAST_100M_LNG,
            recorded_at: chrono::Utc::now(),
            accuracy: None,
        },
    )
    .await
    .expect("ok")
    .expect("inserted");

    assert_eq!(n(out.points), n(Decimal::new(2, 1)), "clamped to 5.0 - 4.8 = 0.2");
}

#[tokio::test]
async fn recorded_at_out_of_tolerance_is_rejected() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default(); // tolerance 45s
    let user = seed_user(&app.pool, "score_skew@example.com").await;
    let session = seed_session(&app.pool, user).await;
    seed_participant(&app.pool, session, user).await;

    let err = score_ping(
        &app.pool,
        &cfg,
        PingInput {
            session_id: session,
            user_id: user,
            seq: 1,
            lat: ORIGIN_LAT,
            lng: ORIGIN_LNG,
            recorded_at: chrono::Utc::now() - chrono::Duration::seconds(600),
        },
    )
    .await;

    assert!(
        matches!(err, Err(walk4change_api::error::AppError::Validation(_))),
        "stale recorded_at must be a Validation error"
    );
}

#[tokio::test]
async fn invalid_coords_are_rejected() {
    let app = common::spawn().await;
    let cfg = ScoringConfig::default();
    let user = seed_user(&app.pool, "score_coords@example.com").await;
    let session = seed_session(&app.pool, user).await;
    seed_participant(&app.pool, session, user).await;

    let err = score_ping(
        &app.pool,
        &cfg,
        PingInput {
            session_id: session,
            user_id: user,
            seq: 1,
            lat: 999.0,
            lng: ORIGIN_LNG,
            recorded_at: chrono::Utc::now(),
            accuracy: None,
        },
    )
    .await;

    assert!(matches!(
        err,
        Err(walk4change_api::error::AppError::Validation(_))
    ));
}
