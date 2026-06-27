//! Black-box end-to-end test for dynamic join/leave and the companion window.
//!
//! Uses `spawn_with_scoring(2)` — ping_window_secs = 2 s — so the together-mult
//! transition can be observed quickly.
//!
//! Scenario:
//!   Phase A (Ana solo, seq 1-3):
//!     Bek has not pinged yet → companions = 0 → together_mult = 1.0
//!
//!   Phase B (overlap, seq 4-7 for Ana, seq 1-4 for Bek):
//!     Bek pings before Ana in each round → Bek's ping is within 2 s when Ana scores
//!     → companions = 1 → together_mult = 1.5
//!
//!   Phase C (Ana solo again, seq 8-10):
//!     Bek calls /leave; Ana sleeps 3 s (> ping_window_secs) so Bek's last ping
//!     ages out of the 2-second window → companions = 0 → together_mult = 1.0
//!
//!   Negative: Bek tries to ping (seq 5) after leaving → server sends error frame
//!             AND no location_pings row is inserted (is_active_participant = false).
//!
//! Coordinate design (all inside Brzeźno nature zone):
//!   ~2 m steps, 400 ms inter-round sleep → speed ≈ 5 m/s < 8 m/s cap.

mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

// ── Coordinate constants ──────────────────────────────────────────────────────

const BASE_LAT: f64 = 54.39850;
const BASE_LNG: f64 = 18.62150;

/// ~2 m per step in latitude.
const STEP_LAT: f64 = 0.000018;
/// ~2 m per step in longitude at lat 54.4.
const STEP_LNG: f64 = 0.000031;

/// Slight north offset for Bek's base so the two tracks don't overlap exactly.
const BEK_LAT_OFFSET: f64 = 0.000050; // ~5.5 m

/// Nature zone WKT covering all test coordinates.
const NATURE_ZONE_WKT: &str = "SRID=4326;POLYGON((\
    18.610 54.393,\
    18.635 54.393,\
    18.635 54.403,\
    18.610 54.403,\
    18.610 54.393\
))";

// ── Ping window (matches spawn_with_scoring argument below) ───────────────────
const PING_WINDOW_SECS: u64 = 2;

// ── WebSocket helpers ─────────────────────────────────────────────────────────

async fn connect_ws(app: &common::TestApp) -> Ws {
    let ws_url = format!("{}/api/v1/ws", app.base_url.replacen("http", "ws", 1));
    connect_async(ws_url).await.expect("ws connect failed").0
}

async fn send_json(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string()))
        .await
        .expect("ws send failed");
}

/// Read the next JSON text frame, skipping control frames.
/// Returns `None` on timeout (5 s), stream end, or transport error.
async fn next_json(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Err(_) => return None,
            Ok(None) | Ok(Some(Err(_))) => return None,
            Ok(Some(Ok(Message::Text(t)))) => {
                return Some(serde_json::from_str(&t).expect("frame must be JSON"))
            }
            Ok(Some(Ok(Message::Close(_)))) => return None,
            Ok(Some(Ok(_))) => continue,
        }
    }
}

/// Read the next JSON frame with a shorter (2 s) timeout — for negative assertions.
async fn next_json_short(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(Duration::from_secs(2), ws.next()).await {
            Err(_) => return None,
            Ok(None) | Ok(Some(Err(_))) => return None,
            Ok(Some(Ok(Message::Text(t)))) => {
                return Some(serde_json::from_str(&t).expect("frame must be JSON"))
            }
            Ok(Some(Ok(Message::Close(_)))) => return None,
            Ok(Some(Ok(_))) => continue,
        }
    }
}

/// Drain frames on `ws` until a `ping_scored` for `user_id` arrives.
/// Panics after 50 attempts (covers accumulated broadcast frames from other users).
async fn await_ping_scored_for(ws: &mut Ws, user_id: Uuid) -> Value {
    let id_str = user_id.to_string();
    for _ in 0..50 {
        let frame = next_json(ws).await.expect("timed out waiting for ping_scored");
        if frame["type"] == "ping_scored" && frame["data"]["user_id"].as_str() == Some(&id_str) {
            return frame;
        }
        // Another participant's frame or leaderboard_update — skip.
    }
    panic!("never received ping_scored for user {user_id}");
}

fn make_ping(session_id: Uuid, seq: i32, lat: f64, lng: f64) -> Value {
    json!({
        "type": "ping",
        "session_id": session_id,
        "seq": seq,
        "lat": lat,
        "lng": lng,
        "recorded_at": chrono::Utc::now().to_rfc3339(),
    })
}

// ── HTTP helpers ──────────────────────────────────────────────────────────────

async fn register_user(app: &common::TestApp, email: &str) -> (Uuid, String) {
    let resp = app
        .client
        .post(format!("{}/api/v1/auth/register", app.base_url))
        .json(&json!({
            "email": email,
            "password": "password123",
            "display_name": "TestUser",
        }))
        .send()
        .await
        .expect("register failed");
    assert_eq!(resp.status().as_u16(), 201);
    let body: Value = resp.json().await.unwrap();
    let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let token = body["token"].as_str().unwrap().to_owned();
    (id, token)
}

// ── The test ──────────────────────────────────────────────────────────────────

/// Dynamic join/leave: asserts that together_mult follows the companion window
/// exactly — 1.0 solo → 1.5 overlap → 1.0 solo again after the window expires.
#[tokio::test]
async fn dynamic_join_leave_partial_overlap() {
    // 2-second companion window so Phase C transition is observable without a long wait.
    let app = common::spawn_with_scoring(PING_WINDOW_SECS as i64).await;

    // ── Register and befriend ─────────────────────────────────────────────────
    let (ana_id, ana_token) = register_user(&app, "dyn_ana@e2e.test").await;
    let (bek_id, bek_token) = register_user(&app, "dyn_bek@e2e.test").await;

    // Establish friendship directly via DB insert (same pattern as replay_smoke_test).
    sqlx::query(
        "INSERT INTO friendships (id, requester_id, addressee_id, status) \
         VALUES ($1, $2, $3, 'accepted')",
    )
    .bind(Uuid::new_v4())
    .bind(ana_id)
    .bind(bek_id)
    .execute(&app.pool)
    .await
    .expect("friendship insert failed");

    // ── Ana starts walk; Bek joins ────────────────────────────────────────────
    let resp = app
        .client
        .post(format!("{}/api/v1/walks", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    let body: Value = resp.json().await.unwrap();
    let session_id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();

    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {bek_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "Bek must join successfully");

    // Insert nature zone (multiplier 3.0) covering test coordinates.
    sqlx::query(
        "INSERT INTO nature_zones (id, name, geom, multiplier, active) \
         VALUES ($1, 'Dynamic Test Zone', ST_GeogFromText($2), 3.0, true)",
    )
    .bind(Uuid::new_v4())
    .bind(NATURE_ZONE_WKT)
    .execute(&app.pool)
    .await
    .expect("nature zone insert failed");

    // ── Open WS connections ───────────────────────────────────────────────────
    let mut ws_a = connect_ws(&app).await;
    let mut ws_b = connect_ws(&app).await;
    send_json(&mut ws_a, json!({"type": "auth", "token": ana_token})).await;
    send_json(&mut ws_b, json!({"type": "auth", "token": bek_token})).await;

    // ── Seq counters and coordinate helpers ───────────────────────────────────
    let mut ana_seq: i32 = 0; // incremented before each Ana ping
    let mut bek_seq: i32 = 0; // incremented before each Bek ping

    let ana_coord = |seq: i32| -> (f64, f64) {
        let i = (seq - 1) as f64;
        (BASE_LAT + i * STEP_LAT, BASE_LNG + i * STEP_LNG)
    };
    let bek_coord = |seq: i32| -> (f64, f64) {
        let i = (seq - 1) as f64;
        (BASE_LAT + BEK_LAT_OFFSET + i * STEP_LAT, BASE_LNG + i * STEP_LNG)
    };

    // ── Phase A: Ana solo, 3 pings ────────────────────────────────────────────
    //   Bek has NOT pinged → companion count for Ana = 0 → together_mult = 1.0.
    for _ in 0..3 {
        ana_seq += 1;
        let (lat, lng) = ana_coord(ana_seq);
        send_json(&mut ws_a, make_ping(session_id, ana_seq, lat, lng)).await;
        await_ping_scored_for(&mut ws_a, ana_id).await;
        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    // ── Phase B: both ping (Bek first, then Ana), 4 rounds ───────────────────
    //   Bek pings first each round so Bek's ping is committed (within the 2 s window)
    //   when Ana's ping is scored → Ana's companion count = 1 → together_mult = 1.5.
    for _ in 0..4 {
        bek_seq += 1;
        let (b_lat, b_lng) = bek_coord(bek_seq);
        send_json(&mut ws_b, make_ping(session_id, bek_seq, b_lat, b_lng)).await;
        await_ping_scored_for(&mut ws_b, bek_id).await;

        ana_seq += 1;
        let (a_lat, a_lng) = ana_coord(ana_seq);
        send_json(&mut ws_a, make_ping(session_id, ana_seq, a_lat, a_lng)).await;
        await_ping_scored_for(&mut ws_a, ana_id).await;

        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    // Ana's Phase B seqs: 4, 5, 6, 7  |  Bek's Phase B seqs: 1, 2, 3, 4

    // ── Bek leaves the walk ───────────────────────────────────────────────────
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/leave", app.base_url))
        .header("Authorization", format!("Bearer {bek_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 204, "Bek leave must return 204");

    // Wait long enough for Bek's last ping to age out of the 2-second companion
    // window.  3 s > ping_window_secs (2 s), so companions = 0 for Ana's next pings.
    let window_clear_ms = (PING_WINDOW_SECS + 1) * 1000; // 3 000 ms
    tokio::time::sleep(Duration::from_millis(window_clear_ms)).await;

    // ── Phase C: Ana solo again, 3 pings ─────────────────────────────────────
    //   Bek's last ping received_at is now > 3 s old (> 2 s window) AND he left.
    //   together_mult must revert to 1.0.
    for _ in 0..3 {
        ana_seq += 1;
        let (lat, lng) = ana_coord(ana_seq);
        send_json(&mut ws_a, make_ping(session_id, ana_seq, lat, lng)).await;
        await_ping_scored_for(&mut ws_a, ana_id).await;
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    // Ana's Phase C seqs: 8, 9, 10

    // ── Assertions: Ana's together_mult sequence ──────────────────────────────
    //
    // Query location_pings for Ana, ordered by seq.
    // Expected together_mult sequence:
    //   seq 1..=3  → 1.0  (Phase A, solo)
    //   seq 4..=7  → 1.5  (Phase B, companion Bek within window)
    //   seq 8..=10 → 1.0  (Phase C, Bek's ping aged out)

    #[derive(sqlx::FromRow)]
    struct PingRow {
        seq: i32,
        together_mult: Decimal,
        nature_mult: Decimal,
    }

    let pings: Vec<PingRow> = sqlx::query_as(
        "SELECT seq, together_mult, nature_mult \
         FROM location_pings \
         WHERE session_id=$1 AND user_id=$2 \
         ORDER BY seq ASC",
    )
    .bind(session_id)
    .bind(ana_id)
    .fetch_all(&app.pool)
    .await
    .expect("query location_pings for Ana failed");

    assert_eq!(
        pings.len(),
        10,
        "Ana must have exactly 10 pings (seq 1-10), got {}",
        pings.len()
    );

    let solo = Decimal::new(1, 0);   // 1.0
    let pair = Decimal::new(15, 1);  // 1.5
    let nature_3 = Decimal::new(3, 0);

    for p in &pings {
        // All coords are inside the nature zone → nature_mult must be 3.0.
        assert_eq!(
            p.nature_mult.normalize(),
            nature_3,
            "seq {} must have nature_mult=3.0, got {}",
            p.seq,
            p.nature_mult
        );

        let expected_together = if p.seq <= 3 {
            // Phase A: solo
            solo
        } else if p.seq <= 7 {
            // Phase B: together with Bek
            pair
        } else {
            // Phase C: solo again (window expired)
            solo
        };

        assert_eq!(
            p.together_mult.normalize(),
            expected_together,
            "seq {} must have together_mult={} (Phase {}), got {}",
            p.seq,
            expected_together,
            if p.seq <= 3 { 'A' } else if p.seq <= 7 { 'B' } else { 'C' },
            p.together_mult
        );
    }

    // Phase B pings (seq 2..=7) must have points > 0 (they have a prev ping).
    // seq 1 has points = 0 (no prev); seq 4 also = 0 (no prev for that user in phase B
    // — but wait, Ana's seq 4 DOES have a prev ping: seq 3 in Phase A, ~400 ms ago).
    // Actually seq 1 for Ana is 0 (first ever ping); all others should score.
    let scored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings \
         WHERE session_id=$1 AND user_id=$2 AND seq > 1 AND points > 0",
    )
    .bind(session_id)
    .bind(ana_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(
        scored_count >= 5, // at least Phase B seqs 4-7 plus some Phase C pings
        "Ana must have at least 5 scored pings (seq > 1 with points > 0), got {scored_count}"
    );

    // ── Negative test: Bek cannot ping after leaving ──────────────────────────
    //
    // Server must reject with an error frame (is_active_participant = false).
    // No location_pings row must be created for Bek's post-leave seq.
    let bek_post_leave_seq = bek_seq + 1; // seq 5 (one past Bek's last Phase B seq)
    let (b_lat, b_lng) = bek_coord(bek_post_leave_seq);
    send_json(
        &mut ws_b,
        make_ping(session_id, bek_post_leave_seq, b_lat, b_lng),
    )
    .await;

    // Drain ws_b looking for an error frame; skip any queued session/leaderboard frames.
    let mut received_error = false;
    for _ in 0..10 {
        match next_json_short(&mut ws_b).await {
            Some(v) if v["type"] == "error" => {
                received_error = true;
                break;
            }
            Some(_) => continue, // queued frame from an earlier broadcast
            None => break,       // timed out — no more frames
        }
    }

    // Primary assertion: no row in location_pings for Bek's post-leave ping.
    let post_leave_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings \
         WHERE session_id=$1 AND user_id=$2 AND seq=$3",
    )
    .bind(session_id)
    .bind(bek_id)
    .bind(bek_post_leave_seq)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        post_leave_rows, 0,
        "Bek's post-leave ping (seq {bek_post_leave_seq}) must not be inserted into location_pings"
    );

    // Secondary: the server should have sent an error frame.
    assert!(
        received_error,
        "Server must send an error frame when a departed participant pings"
    );
}
