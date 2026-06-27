//! Black-box end-to-end journey test.
//!
//! Exercises the full lifecycle through real HTTP + WebSocket against the running
//! app + local PostGIS:
//!
//!  1. Register two users (Ana, Bek).
//!  2. Ana sends a friend request; Bek accepts; assert accepted list.
//!  3. Ana starts a walk; Bek joins.
//!  4. Both open WS, auth, stream 15 interleaved GPS pings inside the nature zone.
//!  5. Assert points > 0 and at least one ping with together_mult=1.5 AND nature_mult=3.0.
//!  6. Ana stops the walk (204); assert finished + total_walks incremented.
//!  7. GET /leaderboard → both users present with display_name.
//!  8. Redeem a reward; assert spent_points + GET /me/redemptions.
//!
//! Coordinate design (safe under the 8 m/s cap):
//!   ~2 m steps, 400 ms inter-round sleep → speed ≈ 5 m/s.
//!   All coords inside Gdańsk Brzeźno polygon (18.610-18.635, 54.393-54.403).

mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

// ── Coordinate constants ──────────────────────────────────────────────────────

/// Ana's base position (slightly different from Bek's so each has a distinct track).
const BASE_LAT_A: f64 = 54.39900;
const BASE_LNG_A: f64 = 18.62300;

/// Bek's base position: ~10 m north of Ana.
const BASE_LAT_B: f64 = 54.39910;
const BASE_LNG_B: f64 = 18.62300;

/// ~2 m step in latitude (2 / 111_320 ≈ 0.000018°).
const STEP_LAT: f64 = 0.000018;
/// ~2 m step in longitude at lat 54.4 (2 / 64_845 ≈ 0.000031°).
const STEP_LNG: f64 = 0.000031;

/// Nature zone WKT — covers all test coordinates.
const NATURE_ZONE_WKT: &str = "SRID=4326;POLYGON((\
    18.610 54.393,\
    18.635 54.393,\
    18.635 54.403,\
    18.610 54.403,\
    18.610 54.393\
))";

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
            Ok(Some(Ok(_))) => continue, // skip control frames
        }
    }
}

/// Drain frames on `ws` until a `ping_scored` for `user_id` arrives.
/// Other participants' frames are skipped.  Panics after 40 attempts.
async fn await_ping_scored_for(ws: &mut Ws, user_id: Uuid) -> Value {
    let id_str = user_id.to_string();
    for _ in 0..40 {
        let frame = next_json(ws).await.expect("timed out waiting for ping_scored");
        if frame["type"] == "ping_scored" && frame["data"]["user_id"].as_str() == Some(&id_str) {
            return frame;
        }
        // Another participant's frame or a leaderboard_update — skip.
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

async fn register_user(
    app: &common::TestApp,
    email: &str,
    display_name: &str,
) -> (Uuid, String) {
    let resp = app
        .client
        .post(format!("{}/api/v1/auth/register", app.base_url))
        .json(&json!({
            "email": email,
            "password": "password123",
            "display_name": display_name,
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

// ── The journey test ──────────────────────────────────────────────────────────

/// Full user journey as a single black-box end-to-end test.
#[tokio::test]
async fn full_user_journey() {
    let app = common::spawn().await;

    // ── 1. Register Ana and Bek ───────────────────────────────────────────────
    let (ana_id, ana_token) = register_user(&app, "journey_ana@e2e.test", "Ana").await;
    let (bek_id, bek_token) = register_user(&app, "journey_bek@e2e.test", "Bek").await;

    // ── 2. Friendship: Ana → Bek, Bek accepts; assert accepted list ───────────
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .json(&json!({ "addressee_id": bek_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201, "friend request must return 201");

    // Bek gets the request_id from his incoming list.
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {bek_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.unwrap();
    let request_id = body["data"]["incoming_pending"][0]["request_id"]
        .as_str()
        .expect("request_id must be present in incoming_pending")
        .to_owned();

    // Bek accepts.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/respond", app.base_url))
        .header("Authorization", format!("Bearer {bek_token}"))
        .json(&json!({ "request_id": request_id, "accept": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "respond must return 200");

    // Assert Ana's accepted list contains Bek.
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.unwrap();
    let accepted = body["data"]["accepted"]
        .as_array()
        .expect("accepted must be an array");
    assert!(
        accepted
            .iter()
            .any(|e| e["id"].as_str().unwrap_or("") == bek_id.to_string()),
        "Ana's accepted list must include Bek"
    );

    // ── 3. Ana starts walk; Bek joins ────────────────────────────────────────
    let resp = app
        .client
        .post(format!("{}/api/v1/walks", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201, "start walk must return 201");
    let body: Value = resp.json().await.unwrap();
    let session_id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();

    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {bek_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "Bek must join successfully (200)");

    // Insert a nature zone covering all test coordinates (multiplier = 3.0).
    sqlx::query(
        "INSERT INTO nature_zones (id, name, geom, multiplier, active) \
         VALUES ($1, 'E2E Journey Test Zone', ST_GeogFromText($2), 3.0, true)",
    )
    .bind(Uuid::new_v4())
    .bind(NATURE_ZONE_WKT)
    .execute(&app.pool)
    .await
    .expect("nature zone insert failed");

    // ── 4. Both open WS, auth, stream 15 interleaved rounds ──────────────────
    //
    // Serialised order per round: A pings → A ack → B pings → B ack → sleep 400ms.
    //
    // Round 1: A seq=1 (no prev → 0 pts); B seq=1 (A's ping just committed → companion=1,
    //          together_mult=1.5, but still no prev for B → 0 pts).
    // Round 2+: each user has a prev ping ~400 ms ago, ~2 m away → speed ≈ 5 m/s < cap;
    //           companion window = 60 s (default) → always sees the other → 1.5 mult;
    //           points = (2/100) * 1.5 * 3.0 = 0.09 per round.
    // After 15 rounds: 14 scored rounds × 0.09 = 1.26 pts each (well above reward cost 1).
    let mut ws_a = connect_ws(&app).await;
    let mut ws_b = connect_ws(&app).await;
    send_json(&mut ws_a, json!({"type": "auth", "token": ana_token})).await;
    send_json(&mut ws_b, json!({"type": "auth", "token": bek_token})).await;

    const ROUNDS: i32 = 15;
    const SLEEP_MS: u64 = 400;

    for seq in 1..=ROUNDS {
        let i = (seq - 1) as f64;
        let a_lat = BASE_LAT_A + i * STEP_LAT;
        let a_lng = BASE_LNG_A + i * STEP_LNG;
        let b_lat = BASE_LAT_B + i * STEP_LAT;
        let b_lng = BASE_LNG_B + i * STEP_LNG;

        // Ana pings; wait for her scored frame.
        send_json(&mut ws_a, make_ping(session_id, seq, a_lat, a_lng)).await;
        await_ping_scored_for(&mut ws_a, ana_id).await;

        // Bek pings; wait for his scored frame.
        send_json(&mut ws_b, make_ping(session_id, seq, b_lat, b_lng)).await;
        await_ping_scored_for(&mut ws_b, bek_id).await;

        // Real sleep so the next round's server-clock dt produces valid speed.
        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await;
    }

    // ── 5. Assert: both accrued points; at least one ping stacks both mults ──
    let a_with_points: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings WHERE session_id=$1 AND user_id=$2 AND points>0",
    )
    .bind(session_id)
    .bind(ana_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(
        a_with_points > 0,
        "Ana must have at least one ping with points > 0"
    );

    let b_with_points: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings WHERE session_id=$1 AND user_id=$2 AND points>0",
    )
    .bind(session_id)
    .bind(bek_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(
        b_with_points > 0,
        "Bek must have at least one ping with points > 0"
    );

    // Core stacking claim: together_mult=1.5 AND nature_mult=3.0 AND points>0.
    let stacked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM location_pings \
         WHERE session_id=$1 AND together_mult=1.5 AND nature_mult=3.0 AND points>0",
    )
    .bind(session_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(
        stacked > 0,
        "At least one ping must have together_mult=1.5 AND nature_mult=3.0 AND points>0 \
         (multiplier stacking not working — check companion window or nature zone coordinates)"
    );

    // ── 6. Ana stops the walk ────────────────────────────────────────────────
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/stop", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 204, "stop must return 204");

    // Session must be finished.
    let status: String =
        sqlx::query_scalar("SELECT status FROM walk_sessions WHERE id=$1")
            .bind(session_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(status, "finished", "session must be finished after stop");

    // Both participants' total_walks must have incremented.
    let ana_walks: i32 =
        sqlx::query_scalar("SELECT total_walks FROM user_totals WHERE user_id=$1")
            .bind(ana_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(ana_walks, 1, "Ana's total_walks must be 1 after stop");

    let bek_walks: i32 =
        sqlx::query_scalar("SELECT total_walks FROM user_totals WHERE user_id=$1")
            .bind(bek_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(bek_walks, 1, "Bek's total_walks must be 1 after stop");

    // ── 7. GET /leaderboard → both present, with display_name ────────────────
    let resp = app
        .client
        .get(format!("{}/api/v1/leaderboard?per_page=50", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.unwrap();
    let entries = body["data"].as_array().expect("leaderboard data must be array");

    let ana_entry = entries
        .iter()
        .find(|e| e["user_id"].as_str() == Some(&ana_id.to_string()))
        .expect("Ana must appear in leaderboard");
    let bek_entry = entries
        .iter()
        .find(|e| e["user_id"].as_str() == Some(&bek_id.to_string()))
        .expect("Bek must appear in leaderboard");

    assert!(
        ana_entry["display_name"].as_str().is_some(),
        "Ana's leaderboard entry must have display_name"
    );
    assert!(
        bek_entry["display_name"].as_str().is_some(),
        "Bek's leaderboard entry must have display_name"
    );

    // Entries are ordered by total_points DESC; both must have non-null total_points.
    assert!(
        ana_entry.get("total_points").is_some(),
        "Ana's leaderboard entry must have total_points"
    );
    assert!(
        bek_entry.get("total_points").is_some(),
        "Bek's leaderboard entry must have total_points"
    );

    // Assert ordering: the array must be non-increasing in total_points.
    // total_points serialises as a JSON string (rust_decimal with plain serde feature).
    let parsed_totals: Vec<Decimal> = entries
        .iter()
        .filter_map(|e| {
            e["total_points"]
                .as_str()
                .and_then(|s| s.parse::<Decimal>().ok())
        })
        .collect();
    for window in parsed_totals.windows(2) {
        assert!(
            window[0] >= window[1],
            "leaderboard must be ordered by total_points DESC, but {} < {} was found",
            window[0],
            window[1]
        );
    }

    // ── 8. Redeem a reward ────────────────────────────────────────────────────

    // Read Ana's actual earned points from DB (avoids fragile point-math assumptions).
    let ana_total: Decimal =
        sqlx::query_scalar("SELECT total_points FROM user_totals WHERE user_id=$1")
            .bind(ana_id)
            .fetch_one(&app.pool)
            .await
            .expect("user_totals must exist for Ana (scoring upserts it)");
    assert!(
        ana_total > Decimal::ZERO,
        "Ana must have total_points > 0 before redemption (scoring did not fire?)"
    );

    // Reward cost = half of Ana's earned points, rounded to 4 dp.
    // This is always ≤ total_points and > 0.
    let reward_cost = (ana_total / Decimal::from(2))
        .round_dp(4)
        .max(Decimal::new(1, 4)); // floor to 0.0001 minimum

    let reward_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rewards_catalog (id, title, cost_points, type, active) \
         VALUES ($1, 'E2E Journey Reward', $2, 'discount', true)",
    )
    .bind(reward_id)
    .bind(reward_cost)
    .execute(&app.pool)
    .await
    .expect("insert reward failed");

    // POST /rewards/:id/redeem → 201 with a non-empty code.
    let resp = app
        .client
        .post(format!("{}/api/v1/rewards/{reward_id}/redeem", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201, "redeem must return 201");

    let body: Value = resp.json().await.unwrap();
    let code = body["data"]["code"]
        .as_str()
        .expect("redemption code must be present");
    assert!(!code.is_empty(), "redemption code must not be empty");

    // Ana's spent_points must equal reward_cost.
    let spent: Decimal =
        sqlx::query_scalar("SELECT spent_points FROM user_totals WHERE user_id=$1")
            .bind(ana_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(
        (spent - reward_cost).abs() < Decimal::new(1, 4),
        "spent_points must equal reward_cost ({reward_cost}), got {spent}"
    );

    // GET /me/redemptions must list the redemption.
    let resp = app
        .client
        .get(format!("{}/api/v1/me/redemptions", app.base_url))
        .header("Authorization", format!("Bearer {ana_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.unwrap();
    let redemptions = body["data"].as_array().expect("redemptions data must be array");
    assert_eq!(redemptions.len(), 1, "Ana must have exactly one redemption");
    assert_eq!(
        redemptions[0]["code"].as_str().unwrap(),
        code,
        "returned code must match redemption list"
    );
}
