mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

// ── HTTP helpers (mirror walks_test.rs) ───────────────────────────────────────

/// Register a user and return (user_id, token).
async fn register_user(app: &common::TestApp, email: &str) -> (Uuid, String) {
    let resp = app
        .client
        .post(format!("{}/api/v1/auth/register", app.base_url))
        .json(&json!({
            "email": email,
            "password": "password123",
            "display_name": "TestUser"
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

/// Establish a friendship: `a` sends request to `b_id`, `b` accepts.
async fn make_friends(app: &common::TestApp, a_token: &str, b_id: Uuid, b_token: &str) {
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": b_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);

    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let request_id = body["data"]["incoming_pending"][0]["request_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let resp = app
        .client
        .post(format!("{}/api/v1/friends/respond", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .json(&json!({ "request_id": request_id, "accept": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

/// `POST /api/v1/walks` — host starts a walk. Returns session_id.
async fn start_walk(app: &common::TestApp, host_token: &str) -> Uuid {
    let resp = app
        .client
        .post(format!("{}/api/v1/walks", app.base_url))
        .header("Authorization", format!("Bearer {host_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201, "start walk must return 201");
    let body: Value = resp.json().await.unwrap();
    body["data"]["id"].as_str().unwrap().parse().unwrap()
}

/// Friend joins a walk via HTTP.
async fn join_walk(app: &common::TestApp, session_id: Uuid, token: &str) {
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "friend join must be 200");
}

// ── WebSocket helpers ─────────────────────────────────────────────────────────

async fn connect_ws(app: &common::TestApp) -> Ws {
    let ws_url = format!("{}/api/v1/ws", app.base_url.replacen("http", "ws", 1));
    let (ws, _resp) = connect_async(ws_url).await.expect("ws connect failed");
    ws
}

async fn send_json(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string()))
        .await
        .expect("ws send failed");
}

/// Read the next application (JSON text) frame, skipping control frames.
/// Returns `None` if the connection closes or no frame arrives within 5s.
async fn next_json(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Err(_) => return None,          // read timeout
            Ok(None) => return None,        // stream ended
            Ok(Some(Ok(Message::Text(t)))) => {
                return Some(serde_json::from_str(&t).expect("frame must be JSON"))
            }
            Ok(Some(Ok(Message::Close(_)))) => return None,
            Ok(Some(Err(_))) => return None,
            Ok(Some(Ok(_))) => continue, // ping/pong/binary/frame
        }
    }
}

fn ping_frame(session_id: Uuid, seq: i32, lat: f64, lng: f64) -> Value {
    json!({
        "type": "ping",
        "session_id": session_id,
        "seq": seq,
        "lat": lat,
        "lng": lng,
        "recorded_at": chrono::Utc::now().to_rfc3339(),
    })
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// First frame is not `Auth` → server rejects (error frame and/or close).
#[tokio::test]
async fn non_auth_first_frame_is_rejected() {
    let app = common::spawn().await;
    let (_, token) = register_user(&app, "ws_noauth@example.com").await;
    let session_id = start_walk(&app, &token).await;

    let mut ws = connect_ws(&app).await;
    // Send a ping BEFORE authenticating.
    send_json(&mut ws, ping_frame(session_id, 1, 52.0, 21.0)).await;

    // Acceptable outcomes: an error frame (then close) OR an immediate close.
    match next_json(&mut ws).await {
        Some(v) => assert_eq!(
            v["type"], "error",
            "pre-auth frame must yield an error, got {v:?}"
        ),
        None => {} // closed immediately — also acceptable
    }
}

/// Auth, then ping into own active session → receive a `ping_scored` frame.
#[tokio::test]
async fn auth_then_ping_own_session_receives_ping_scored() {
    let app = common::spawn().await;
    let (_, token) = register_user(&app, "ws_self@example.com").await;
    let session_id = start_walk(&app, &token).await;

    let mut ws = connect_ws(&app).await;
    send_json(&mut ws, json!({ "type": "auth", "token": token })).await;
    send_json(&mut ws, ping_frame(session_id, 1, 52.0, 21.0)).await;

    let frame = next_json(&mut ws)
        .await
        .expect("must receive a ping_scored frame");
    assert_eq!(frame["type"], "ping_scored", "got {frame:?}");
    assert!(
        frame["data"].get("points").is_some(),
        "ping_scored data must carry a points field, got {frame:?}"
    );
}

/// A member who subscribes receives another participant's `ping_scored`.
#[tokio::test]
async fn subscribed_member_receives_other_participants_ping() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "ws_host@example.com").await;
    let (friend_id, friend_token) = register_user(&app, "ws_friend@example.com").await;

    make_friends(&app, &host_token, friend_id, &friend_token).await;
    let session_id = start_walk(&app, &host_token).await;
    join_walk(&app, session_id, &friend_token).await;

    // B (friend) connects, auths, subscribes.
    let mut b = connect_ws(&app).await;
    send_json(&mut b, json!({ "type": "auth", "token": friend_token })).await;
    send_json(&mut b, json!({ "type": "subscribe", "session_id": session_id })).await;

    // Give B's subscription time to register before A publishes.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // A (host) connects, auths, pings.
    let mut a = connect_ws(&app).await;
    send_json(&mut a, json!({ "type": "auth", "token": host_token })).await;
    send_json(&mut a, ping_frame(session_id, 1, 52.0, 21.0)).await;

    let frame = next_json(&mut b)
        .await
        .expect("subscriber B must receive a ping_scored frame");
    assert_eq!(frame["type"], "ping_scored", "got {frame:?}");
}

/// After a scored ping, the `leaderboard_update` broadcast carries an ARRAY
/// whose elements have `user_id`, `display_name`, and `total_points`.
#[tokio::test]
async fn leaderboard_update_has_array_shape() {
    let app = common::spawn().await;
    let (_, token) = register_user(&app, "ws_lb_shape@example.com").await;
    let session_id = start_walk(&app, &token).await;

    // Subscribe to leaderboard on a dedicated connection.
    let mut sub = connect_ws(&app).await;
    send_json(&mut sub, serde_json::json!({ "type": "auth", "token": token })).await;
    send_json(&mut sub, serde_json::json!({ "type": "subscribe_leaderboard" })).await;

    // Allow the subscription to register before the ping fires.
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Trigger a ping from a separate connection (host is an active participant).
    let mut pinger = connect_ws(&app).await;
    send_json(&mut pinger, serde_json::json!({ "type": "auth", "token": token })).await;
    send_json(
        &mut pinger,
        ping_frame(session_id, 1, 52.0, 21.0),
    )
    .await;

    // Drain frames on `sub` until we see a leaderboard_update (bounded wait).
    // The subscriber is only subscribed to the leaderboard channel, so all
    // frames it receives should be leaderboard_update.
    let mut received: Option<serde_json::Value> = None;
    for _ in 0..10 {
        match next_json(&mut sub).await {
            Some(v) if v["type"] == "leaderboard_update" => {
                received = Some(v);
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }

    let frame = received.expect("must receive a leaderboard_update frame within the timeout");

    let data = &frame["data"];
    assert!(
        data.is_array(),
        "leaderboard_update data must be a JSON array, got: {frame:?}"
    );

    // If the array is non-empty (it will have at least the pinging user's row
    // since score_ping upserts user_totals even on first/zero-point ping),
    // verify the entry shape.
    let entries = data.as_array().unwrap();
    assert!(
        !entries.is_empty(),
        "leaderboard array must contain at least the active user's entry"
    );
    let entry = &entries[0];
    assert!(
        entry.get("user_id").is_some(),
        "entry must have user_id, got: {entry:?}"
    );
    assert!(
        entry.get("display_name").is_some(),
        "entry must have display_name, got: {entry:?}"
    );
    assert!(
        entry.get("total_points").is_some(),
        "entry must have total_points, got: {entry:?}"
    );
}

/// Subscribing to a session you are not a member of → error frame.
#[tokio::test]
async fn subscribe_non_member_session_yields_error() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "ws_owner@example.com").await;
    let (_, outsider_token) = register_user(&app, "ws_outsider@example.com").await;

    let session_id = start_walk(&app, &host_token).await;

    let mut ws = connect_ws(&app).await;
    send_json(&mut ws, json!({ "type": "auth", "token": outsider_token })).await;
    send_json(&mut ws, json!({ "type": "subscribe", "session_id": session_id })).await;

    let frame = next_json(&mut ws)
        .await
        .expect("non-member subscribe must yield a frame");
    assert_eq!(
        frame["type"], "error",
        "non-member subscribe must be an error, got {frame:?}"
    );
}

/// Spec §271: a participant who LEFT the session cannot subscribe to the live WS stream.
#[tokio::test]
async fn left_participant_subscribe_yields_error() {
    let app = common::spawn().await;
    let (host_id, host_token) = register_user(&app, "ws_left_host@example.com").await;
    let (friend_id, friend_token) = register_user(&app, "ws_left_friend@example.com").await;

    make_friends(&app, &host_token, friend_id, &friend_token).await;
    let session_id = start_walk(&app, &host_token).await;
    join_walk(&app, session_id, &friend_token).await;

    // Friend leaves the session (left_at is now set).
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/leave", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 204, "leave must return 204");

    // Friend tries to subscribe to the live stream — must be rejected.
    let mut ws = connect_ws(&app).await;
    send_json(&mut ws, json!({ "type": "auth", "token": friend_token })).await;
    send_json(&mut ws, json!({ "type": "subscribe", "session_id": session_id })).await;

    let frame = next_json(&mut ws)
        .await
        .expect("left participant subscribe must yield an error frame");
    assert_eq!(
        frame["type"], "error",
        "left participant subscribe must be an error, got {frame:?}"
    );

    // Suppress unused-variable warnings for host_id (only friend_id matters here).
    let _ = host_id;
}

/// Like `next_json` but with a 1-second timeout, for negative assertions.
async fn next_json_short(ws: &mut Ws) -> Option<Value> {
    loop {
        match tokio::time::timeout(Duration::from_secs(1), ws.next()).await {
            Err(_) => return None,
            Ok(None) => return None,
            Ok(Some(Ok(Message::Text(t)))) => {
                return Some(serde_json::from_str(&t).expect("frame must be JSON"))
            }
            Ok(Some(Ok(Message::Close(_)))) => return None,
            Ok(Some(Err(_))) => return None,
            Ok(Some(Ok(_))) => continue,
        }
    }
}

/// Spec §271: live subscription is revoked when the subscriber leaves mid-session.
///
/// Flow: friend subscribes → host pings seq=1 → assert friend receives it (stream
/// is live) → friend leaves (204) → short DB-propagation delay → host pings
/// seq=2 → assert friend receives NO frame within 1 s.
///
/// Using distinct seq numbers prevents dedup (score_ping silently drops seq=1
/// if it was already stored), which would cause a spurious pass.
#[tokio::test]
async fn leave_revokes_subscription_no_frames_after_leave() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "ws_rev_host@example.com").await;
    let (friend_id, friend_token) = register_user(&app, "ws_rev_friend@example.com").await;

    make_friends(&app, &host_token, friend_id, &friend_token).await;
    let session_id = start_walk(&app, &host_token).await;
    join_walk(&app, session_id, &friend_token).await;

    // Friend subscribes to the live session stream.
    let mut subscriber = connect_ws(&app).await;
    send_json(&mut subscriber, json!({ "type": "auth", "token": friend_token })).await;
    send_json(&mut subscriber, json!({ "type": "subscribe", "session_id": session_id })).await;

    // Allow subscription to register before the first ping.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Host pings seq=1 — subscriber must receive it (proves the stream is live).
    let mut host_ws = connect_ws(&app).await;
    send_json(&mut host_ws, json!({ "type": "auth", "token": host_token })).await;
    send_json(&mut host_ws, ping_frame(session_id, 1, 52.0, 21.0)).await;

    let first_frame = next_json(&mut subscriber)
        .await
        .expect("subscriber must receive ping_scored for seq=1");
    assert_eq!(
        first_frame["type"], "ping_scored",
        "expected ping_scored, got: {first_frame:?}"
    );

    // Friend leaves the session.
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/leave", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 204, "leave must return 204");

    // Allow the DB write (left_at) to propagate before the next ping.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Host pings seq=2 — departed subscriber must NOT receive this frame.
    send_json(&mut host_ws, ping_frame(session_id, 2, 52.001, 21.001)).await;

    let second_frame = next_json_short(&mut subscriber).await;
    assert!(
        second_frame.is_none(),
        "departed subscriber must NOT receive ping_scored after leaving, got: {second_frame:?}"
    );
}
