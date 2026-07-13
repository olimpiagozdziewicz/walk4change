mod common;

use serde_json::json;
use uuid::Uuid;

/// Register a user and return (user_id, token).
async fn register_user(app: &common::TestApp, email: &str) -> (Uuid, String) {
    let resp = app
        .client
        .post(format!("{}/api/v1/auth/register", app.base_url))
        .json(&json!({
            "email": email,
            "password": "password123",
            "display_name": "TestUser",
            "accepted_terms": true
        }))
        .send()
        .await
        .expect("register failed");
    assert_eq!(resp.status().as_u16(), 201, "register must return 201");
    let body: serde_json::Value = resp.json().await.unwrap();
    let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let token = body["token"].as_str().unwrap().to_owned();
    (id, token)
}

/// Establish a friendship: `a` sends request to `b_id`, `b` accepts.
async fn make_friends(app: &common::TestApp, a_token: &str, b_id: Uuid, b_token: &str) {
    // A sends request to B
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": b_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);

    // Get request_id from B's list
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let request_id = body["data"]["incoming_pending"][0]["request_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // B accepts
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

/// `POST /api/v1/walks` — host starts a walk. Returns (session_id, full response body).
async fn start_walk(app: &common::TestApp, host_token: &str) -> (Uuid, serde_json::Value) {
    let resp = app
        .client
        .post(format!("{}/api/v1/walks", app.base_url))
        .header("Authorization", format!("Bearer {host_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201, "start walk must return 201");
    // Capture the Location header before consuming the response body.
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body: serde_json::Value = resp.json().await.unwrap();
    let session_id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        location.unwrap(),
        format!("/api/v1/walks/{session_id}"),
        "Location header must point to the new session"
    );
    (session_id, body)
}

/// host starts → GET returns 1 participant + join_code present
#[tokio::test]
async fn host_start_returns_session_with_join_code_and_one_participant() {
    let app = common::spawn().await;
    let (host_id, host_token) = register_user(&app, "walk_host1@example.com").await;

    let (session_id, start_body) = start_walk(&app, &host_token).await;

    // join_code must be present and 8 chars
    let join_code = start_body["data"]["join_code"].as_str().unwrap();
    assert_eq!(join_code.len(), 8, "join_code must be 8 chars");

    // GET shows session + 1 participant (host)
    let resp = app
        .client
        .get(format!("{}/api/v1/walks/{session_id}", app.base_url))
        .header("Authorization", format!("Bearer {host_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let participants = body["data"]["participants"].as_array().unwrap();
    assert_eq!(participants.len(), 1, "should have 1 participant (host)");
    assert_eq!(
        participants[0]["user_id"].as_str().unwrap(),
        host_id.to_string(),
        "participant must be the host"
    );
}

/// non-friend join → 403
#[tokio::test]
async fn non_friend_join_returns_403() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "walk_host2@example.com").await;
    let (_, stranger_token) = register_user(&app, "walk_stranger2@example.com").await;

    let (session_id, _) = start_walk(&app, &host_token).await;

    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {stranger_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403, "non-friend join must be 403");
}

/// friend join → 200 & appears in participants
#[tokio::test]
async fn friend_join_returns_200_and_appears_in_participants() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "walk_host3@example.com").await;
    let (friend_id, friend_token) = register_user(&app, "walk_friend3@example.com").await;

    make_friends(&app, &host_token, friend_id, &friend_token).await;

    let (session_id, _) = start_walk(&app, &host_token).await;

    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "friend join must be 200");

    // GET from friend shows 2 participants
    let resp = app
        .client
        .get(format!("{}/api/v1/walks/{session_id}", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let participants = body["data"]["participants"].as_array().unwrap();
    assert_eq!(participants.len(), 2, "should have 2 participants");
    let ids: Vec<String> = participants
        .iter()
        .map(|p| p["user_id"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        ids.iter().any(|id| id == &friend_id.to_string()),
        "friend must be in participants"
    );
}

/// non-member GET → 403
#[tokio::test]
async fn non_member_get_returns_403() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "walk_host4@example.com").await;
    let (_, outsider_token) = register_user(&app, "walk_outsider4@example.com").await;

    let (session_id, _) = start_walk(&app, &host_token).await;

    let resp = app
        .client
        .get(format!("{}/api/v1/walks/{session_id}", app.base_url))
        .header("Authorization", format!("Bearer {outsider_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403, "non-member GET must be 403");
}

/// leave sets left_at (visible in GET participants)
#[tokio::test]
async fn leave_sets_left_at_in_participants() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "walk_host5@example.com").await;
    let (friend_id, friend_token) = register_user(&app, "walk_friend5@example.com").await;

    make_friends(&app, &host_token, friend_id, &friend_token).await;
    let (session_id, _) = start_walk(&app, &host_token).await;

    // Friend joins
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    // Friend leaves
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/leave", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 204, "leave must return 204");

    // GET from host — friend's left_at must be set
    let resp = app
        .client
        .get(format!("{}/api/v1/walks/{session_id}", app.base_url))
        .header("Authorization", format!("Bearer {host_token}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let participants = body["data"]["participants"].as_array().unwrap();
    let friend_p = participants
        .iter()
        .find(|p| p["user_id"].as_str().unwrap() == friend_id.to_string())
        .expect("friend must be in participants");
    assert!(
        friend_p["left_at"].as_str().is_some(),
        "left_at must be set for the friend who left"
    );
}

/// non-host stop → 403
#[tokio::test]
async fn non_host_stop_returns_403() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "walk_host6@example.com").await;
    let (friend_id, friend_token) = register_user(&app, "walk_friend6@example.com").await;

    make_friends(&app, &host_token, friend_id, &friend_token).await;
    let (session_id, _) = start_walk(&app, &host_token).await;

    // Friend joins
    let _ = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();

    // Friend tries to stop — must be 403
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/stop", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403, "non-host stop must be 403");
}

/// host stop → session finished AND each participant's user_totals.total_walks incremented
#[tokio::test]
async fn host_stop_finishes_session_and_bumps_total_walks() {
    let app = common::spawn().await;
    let (host_id, host_token) = register_user(&app, "walk_host7@example.com").await;
    let (friend_id, friend_token) = register_user(&app, "walk_friend7@example.com").await;

    make_friends(&app, &host_token, friend_id, &friend_token).await;
    let (session_id, _) = start_walk(&app, &host_token).await;

    // Friend joins
    let _ = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/join", app.base_url))
        .header("Authorization", format!("Bearer {friend_token}"))
        .send()
        .await
        .unwrap();

    // Host stops
    let resp = app
        .client
        .post(format!("{}/api/v1/walks/{session_id}/stop", app.base_url))
        .header("Authorization", format!("Bearer {host_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 204, "host stop must return 204");

    // Query DB — session must be finished
    let session_status: String = sqlx::query_scalar("SELECT status FROM walk_sessions WHERE id = $1")
        .bind(session_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(session_status, "finished", "session must be finished after stop");

    // Query DB — host's total_walks must be 1
    let host_walks: i32 = sqlx::query_scalar(
        "SELECT total_walks FROM user_totals WHERE user_id = $1",
    )
    .bind(host_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(host_walks, 1, "host's total_walks must be 1 after stop");

    // Query DB — friend's total_walks must be 1
    let friend_walks: i32 = sqlx::query_scalar(
        "SELECT total_walks FROM user_totals WHERE user_id = $1",
    )
    .bind(friend_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(friend_walks, 1, "friend's total_walks must be 1 after stop");
}

/// GET and track without token → 401
#[tokio::test]
async fn unauthenticated_requests_return_401() {
    let app = common::spawn().await;
    let (_, host_token) = register_user(&app, "walk_host8@example.com").await;
    let (session_id, _) = start_walk(&app, &host_token).await;

    // GET without token → 401
    let resp = app
        .client
        .get(format!("{}/api/v1/walks/{session_id}", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401, "GET without token must be 401");

    // track without token → 401
    let resp = app
        .client
        .get(format!("{}/api/v1/walks/{session_id}/track", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401, "track without token must be 401");
}

/// track returns ordered pings with correct lat/lng
#[tokio::test]
async fn track_returns_ordered_pings_with_lat_lng() {
    let app = common::spawn().await;
    let (host_id, host_token) = register_user(&app, "walk_host9@example.com").await;
    let (session_id, _) = start_walk(&app, &host_token).await;

    // Insert 3 pings directly via raw SQL.
    // ST_MakePoint(X, Y) = ST_MakePoint(lng, lat)
    let ping1_id = Uuid::new_v4();
    let ping2_id = Uuid::new_v4();
    let ping3_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO location_pings (id, session_id, user_id, geom, recorded_at, seq) \
         VALUES \
         ($1, $4, $5, ST_SetSRID(ST_MakePoint(10.0, 20.0), 4326)::geography, now(), 1), \
         ($2, $4, $5, ST_SetSRID(ST_MakePoint(11.0, 21.0), 4326)::geography, now(), 2), \
         ($3, $4, $5, ST_SetSRID(ST_MakePoint(12.0, 22.0), 4326)::geography, now(), 3)",
    )
    .bind(ping1_id)
    .bind(ping2_id)
    .bind(ping3_id)
    .bind(session_id)
    .bind(host_id)
    .execute(&app.pool)
    .await
    .unwrap();

    let resp = app
        .client
        .get(format!("{}/api/v1/walks/{session_id}/track", app.base_url))
        .header("Authorization", format!("Bearer {host_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let pings = body["data"].as_array().unwrap();
    assert_eq!(pings.len(), 3, "should return 3 pings");

    // Must be ordered by seq
    assert_eq!(pings[0]["seq"].as_i64().unwrap(), 1);
    assert_eq!(pings[1]["seq"].as_i64().unwrap(), 2);
    assert_eq!(pings[2]["seq"].as_i64().unwrap(), 3);

    // First ping: ST_MakePoint(10.0, 20.0) → lng=10, lat=20
    let lat0 = pings[0]["lat"].as_f64().unwrap();
    let lng0 = pings[0]["lng"].as_f64().unwrap();
    assert!(
        (lat0 - 20.0).abs() < 0.0001,
        "lat should be ~20.0, got {lat0}"
    );
    assert!(
        (lng0 - 10.0).abs() < 0.0001,
        "lng should be ~10.0, got {lng0}"
    );
}
