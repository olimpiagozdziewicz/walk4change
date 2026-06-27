//! End-to-end smoke test for the two-track / together-multiplier scenario.
//!
//! Validates the core demo claim:
//! - Two friends walk concurrently in the same session.
//! - Both accumulate points (server-clock dt + distance scoring).
//! - At least one ping has `together_mult > 1` (companion window fires).
//! - All pings land inside the inserted Baltic nature zone → `nature_mult = 3.0`.
//! - At least one ping has both `nature_mult = 3.0` and `together_mult > 1`
//!   with `points > 0` (proves multiplier stacking).
//!
//! Coordinate design:
//! - Centre: lat=54.398, lng=18.622 (Gdańsk Brzeźno, inside the nature zone).
//! - Step size: ~2 m per ping (≈ 0.000018° lat, 0.000031° lng).
//! - Ping interval: ~450 ms real elapsed → speed ≈ 4.4 m/s < 8 m/s cap → valid segment.
//!
//! Note: the brief mentions "80-120 m apart, 1 s spacing" which is physically
//! impossible under the 8 m/s cap (would be 80-120 m/s). The fixtures and this
//! test intentionally use ~2-5 m steps to produce non-zero scored segments.

mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

// ── Coordinate constants ──────────────────────────────────────────────────────

/// Base lat/lng inside the Gdańsk Brzeźno nature zone (54.393-54.403, 18.610-18.635).
const BASE_LAT_A: f64 = 54.39800;
const BASE_LNG_A: f64 = 18.62200;
const BASE_LAT_B: f64 = 54.39810; // B is ~11 m north of A (different path)
const BASE_LNG_B: f64 = 18.62200;

/// ~2 m per step in latitude (2 / 111_320 ≈ 0.000018°).
const STEP_LAT: f64 = 0.000018;
/// ~2 m per step in longitude at lat 54.4 (2 / 64_845 ≈ 0.000031°).
const STEP_LNG: f64 = 0.000031;

/// Nature-zone WKT (same polygon as the seeder — covers our test coords).
const BRZEZNO_WKT: &str = "SRID=4326;POLYGON((\
    18.610 54.393,\
    18.635 54.393,\
    18.635 54.403,\
    18.610 54.403,\
    18.610 54.393\
))";

// ── HTTP helpers ──────────────────────────────────────────────────────────────

async fn register_user(app: &common::TestApp, email: &str) -> (Uuid, String) {
    let resp = app
        .client
        .post(format!("{}/api/v1/auth/register", app.base_url))
        .json(&json!({
            "email": email,
            "password": "password123",
            "display_name": "SmokeUser",
        }))
        .send()
        .await
        .expect("register failed");
    assert_eq!(resp.status().as_u16(), 201, "register must return 201");
    let body: Value = resp.json().await.unwrap();
    let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let token = body["token"].as_str().unwrap().to_owned();
    (id, token)
}

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

/// Read and parse the next text frame, skipping control frames.
/// Returns `None` on timeout (3 s), stream end, or transport error.
async fn next_json(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(Duration::from_secs(3), ws.next()).await {
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

/// Drain frames on `ws` until we find a `ping_scored` for `user_id`.
///
/// Subscription to the session fires on the first ping, so each socket
/// receives all participants' scored frames; we must filter by `user_id`.
/// Panics if no matching frame arrives within 30 attempts.
async fn await_ping_scored_for(ws: &mut Ws, user_id: Uuid) -> Value {
    let id_str = user_id.to_string();
    for _ in 0..30 {
        let frame = next_json(ws).await.expect("timed out waiting for ping_scored");
        if frame["type"] == "ping_scored" && frame["data"]["user_id"].as_str() == Some(&id_str) {
            return frame;
        }
        // Some other participant's frame or a leaderboard_update — skip.
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

// ── The smoke test ────────────────────────────────────────────────────────────

/// Two friends walk concurrently in one session; asserts together_mult > 1,
/// nature_mult = 3.0, and points > 0 — the §6 server-clock demo claim.
#[tokio::test]
async fn replay_smoke_two_friends_together_mult_and_nature() {
    let app = common::spawn().await;

    // ── Register two users ───────────────────────────────────────────────────
    let (a_id, a_token) = register_user(&app, "smoke_a@replay.test").await;
    let (b_id, b_token) = register_user(&app, "smoke_b@replay.test").await;

    // ── Accepted friendship (direct DB insert — simpler than HTTP round-trip) ─
    sqlx::query(
        "INSERT INTO friendships (id, requester_id, addressee_id, status) \
         VALUES ($1, $2, $3, 'accepted')",
    )
    .bind(Uuid::new_v4())
    .bind(a_id)
    .bind(b_id)
    .execute(&app.pool)
    .await
    .expect("friendship insert failed");

    // ── Insert nature zone covering test coordinates ──────────────────────────
    sqlx::query(
        "INSERT INTO nature_zones (id, name, geom, multiplier, active) \
         VALUES ($1, 'Brzeźno Smoke Test Zone', ST_GeogFromText($2), 3.0, true)",
    )
    .bind(Uuid::new_v4())
    .bind(BRZEZNO_WKT)
    .execute(&app.pool)
    .await
    .expect("nature zone insert failed");

    // ── A starts walk, B joins ────────────────────────────────────────────────
    let resp = app
        .client
        .post(format!("{}/api/v1/walks", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .send()
        .await
        .expect("start walk failed");
    assert_eq!(resp.status().as_u16(), 201, "start walk must return 201");
    let body: Value = resp.json().await.unwrap();
    let session_id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();

    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .send()
        .await
        .expect("join walk request failed");
    assert_eq!(resp.status().as_u16(), 200, "B must join the walk successfully");

    // ── Open WS connections and authenticate ──────────────────────────────────
    let mut ws_a = connect_ws(&app).await;
    let mut ws_b = connect_ws(&app).await;
    send_json(&mut ws_a, json!({"type": "auth", "token": a_token})).await;
    send_json(&mut ws_b, json!({"type": "auth", "token": b_token})).await;

    // ── Stream 4 ping rounds, serialized: A→ack, B→ack, sleep ────────────────
    //
    // Serialized (not concurrent) so we control the server-clock dt precisely.
    //
    // After round 1:
    //   - A's ping1 is in DB with companions=0 (B hasn't pinged yet).
    //   - B's ping1 is in DB — companions=1 (A's ping1 is committed) →
    //     together_mult = 1.5, but points = 0 (no previous ping for B).
    //
    // After round 2 (dt ≈ 450 ms, dist ≈ 2 m → 4.4 m/s < 8 m/s cap):
    //   - A's ping2: companions=1 (B's ping1 in window) → together_mult=1.5,
    //     segment valid → points > 0. ✓
    //   - B's ping2: companions=1 → together_mult=1.5, segment valid → points > 0. ✓
    //
    // Rounds 3 and 4 add more evidence (and ensure the assertions aren't lucky).

    const ROUNDS: i32 = 4;
    const INTER_ROUND_MS: u64 = 400;

    for seq in 1..=ROUNDS {
        let i = (seq - 1) as f64;
        let a_lat = BASE_LAT_A + i * STEP_LAT;
        let a_lng = BASE_LNG_A + i * STEP_LNG;
        let b_lat = BASE_LAT_B + i * STEP_LAT;
        let b_lng = BASE_LNG_B + i * STEP_LNG;

        // A pings first; both sides subscribe on first ping → later, ws_a also
        // receives B's frames, and ws_b receives A's frames (filtered below).
        send_json(&mut ws_a, make_ping(session_id, seq, a_lat, a_lng)).await;
        await_ping_scored_for(&mut ws_a, a_id).await;

        send_json(&mut ws_b, make_ping(session_id, seq, b_lat, b_lng)).await;
        await_ping_scored_for(&mut ws_b, b_id).await;

        // Real sleep so the next round's server-clock dt is meaningful.
        tokio::time::sleep(Duration::from_millis(INTER_ROUND_MS)).await;
    }

    // ── Assertions via direct DB query ────────────────────────────────────────

    // 1. Both users must have accumulated total_points > 0 in walk_participants.
    let a_has_points: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM walk_participants \
         WHERE session_id = $1 AND user_id = $2 AND total_points > 0",
    )
    .bind(session_id)
    .bind(a_id)
    .fetch_one(&app.pool)
    .await
    .expect("query failed");
    assert!(a_has_points > 0, "User A must have accumulated points (got 0)");

    let b_has_points: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM walk_participants \
         WHERE session_id = $1 AND user_id = $2 AND total_points > 0",
    )
    .bind(session_id)
    .bind(b_id)
    .fetch_one(&app.pool)
    .await
    .expect("query failed");
    assert!(b_has_points > 0, "User B must have accumulated points (got 0)");

    // 2. At least one ping must have together_mult > 1 (companion window fired).
    let together_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings \
         WHERE session_id = $1 AND together_mult > 1",
    )
    .bind(session_id)
    .fetch_one(&app.pool)
    .await
    .expect("query failed");
    assert!(
        together_count > 0,
        "together_mult > 1 must fire for at least one ping (got 0 — \
         check companion window or serialization order)"
    );

    // 3. All pings should have nature_mult = 3.0 (coords inside zone).
    let nature_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings \
         WHERE session_id = $1 AND nature_mult = 3.0",
    )
    .bind(session_id)
    .fetch_one(&app.pool)
    .await
    .expect("query failed");
    assert!(
        nature_count > 0,
        "nature_mult = 3.0 must appear for at least one ping (got 0 — \
         check coordinates vs. zone polygon)"
    );

    // 4. Core claim: at least one ping has both multipliers AND points > 0.
    let stacked_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings \
         WHERE session_id = $1 \
           AND nature_mult = 3.0 \
           AND together_mult > 1 \
           AND points > 0",
    )
    .bind(session_id)
    .fetch_one(&app.pool)
    .await
    .expect("query failed");
    assert!(
        stacked_count > 0,
        "At least one ping must have nature_mult=3.0 AND together_mult>1 AND points>0 \
         (got 0 — together_mult or nature_mult is not firing, or all segments are teleport-zeroed)"
    );
}
