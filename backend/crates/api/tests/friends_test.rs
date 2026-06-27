mod common;

use serde_json::json;
use uuid::Uuid;
use walk4change_api::repo::friend as friend_repo;

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
    let body: serde_json::Value = resp.json().await.unwrap();
    let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let token = body["token"].as_str().unwrap().to_owned();
    (id, token)
}

/// POST /api/v1/friends/request — A requests B → 201.
/// B's GET /friends shows incoming_pending containing A.
#[tokio::test]
async fn send_request_and_b_sees_incoming_pending() {
    let app = common::spawn().await;
    let (a_id, a_token) = register_user(&app, "friend_a@example.com").await;
    let (b_id, b_token) = register_user(&app, "friend_b@example.com").await;

    // A sends a request to B.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": b_id }))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status().as_u16(), 201, "send_request must return 201");

    // B lists friends — should see incoming_pending with A's id.
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .send()
        .await
        .expect("get friends failed");
    assert_eq!(resp.status().as_u16(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let incoming = body["data"]["incoming_pending"].as_array().expect("incoming_pending is array");
    assert_eq!(incoming.len(), 1, "B must have 1 incoming request");
    assert_eq!(
        incoming[0]["user"]["id"].as_str().unwrap(),
        a_id.to_string(),
        "incoming request must be from A"
    );

    // A lists friends — should see outgoing_pending with B's id.
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .send()
        .await
        .expect("get friends failed");
    assert_eq!(resp.status().as_u16(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let outgoing = body["data"]["outgoing_pending"].as_array().expect("outgoing_pending is array");
    assert_eq!(outgoing.len(), 1, "A must have 1 outgoing request");
    assert_eq!(
        outgoing[0]["user"]["id"].as_str().unwrap(),
        b_id.to_string(),
        "outgoing request must be to B"
    );
}

/// B accepts A's request → are_friends true; both see each other in accepted.
#[tokio::test]
async fn accept_request_marks_friends() {
    let app = common::spawn().await;
    let (a_id, a_token) = register_user(&app, "accept_a@example.com").await;
    let (b_id, b_token) = register_user(&app, "accept_b@example.com").await;

    // A sends request.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": b_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);

    // Get the request_id from B's incoming_pending list.
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
        .expect("request_id must be present");

    // B accepts.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/respond", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .json(&json!({ "request_id": request_id, "accept": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "respond must return 200");

    // are_friends(A, B) must be true via repo.
    let friends = friend_repo::are_friends(&app.pool, a_id, b_id)
        .await
        .expect("are_friends failed");
    assert!(friends, "A and B must be friends after acceptance");

    // also direction-agnostic: are_friends(B, A)
    let friends_reverse = friend_repo::are_friends(&app.pool, b_id, a_id)
        .await
        .expect("are_friends reverse failed");
    assert!(friends_reverse, "are_friends must be symmetric");

    // A's GET /friends shows B in accepted.
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let a_accepted = body["data"]["accepted"].as_array().unwrap();
    assert_eq!(a_accepted.len(), 1, "A must see B in accepted");
    assert_eq!(a_accepted[0]["id"].as_str().unwrap(), b_id.to_string());

    // B's GET /friends shows A in accepted.
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let b_accepted = body["data"]["accepted"].as_array().unwrap();
    assert_eq!(b_accepted.len(), 1, "B must see A in accepted");
    assert_eq!(b_accepted[0]["id"].as_str().unwrap(), a_id.to_string());
}

/// After A→B pending, B→A → 409 (direction-agnostic duplicate).
#[tokio::test]
async fn reverse_direction_request_returns_409() {
    let app = common::spawn().await;
    let (a_id, a_token) = register_user(&app, "dup_a@example.com").await;
    let (b_id, b_token) = register_user(&app, "dup_b@example.com").await;

    // A → B pending.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": b_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);

    // B → A — must be 409 even though it's the REVERSE direction.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .json(&json!({ "addressee_id": a_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 409, "reverse-direction duplicate must be 409");
}

/// Responding to a request as a non-addressee → 403.
#[tokio::test]
async fn respond_by_non_addressee_returns_403() {
    let app = common::spawn().await;
    let (_, a_token) = register_user(&app, "na_a@example.com").await;
    let (b_id, _b_token) = register_user(&app, "na_b@example.com").await;
    let (_, c_token) = register_user(&app, "na_c@example.com").await;

    // A → B.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": b_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);

    // A → B request; get request_id through the repo.
    let row = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM friendships WHERE addressee_id = $1 AND status = 'pending'",
    )
    .bind(b_id)
    .fetch_one(&app.pool)
    .await
    .expect("fetch request_id failed");

    // C tries to accept A→B — must be 403.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/respond", app.base_url))
        .header("Authorization", format!("Bearer {c_token}"))
        .json(&json!({ "request_id": row, "accept": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403, "non-addressee respond must be 403");
}

/// Self-request → 422.
#[tokio::test]
async fn self_request_returns_422() {
    let app = common::spawn().await;
    let (a_id, a_token) = register_user(&app, "self_a@example.com").await;

    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": a_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 422, "self-request must be 422");
}

/// Decline (accept=false) removes the pending row; it disappears from both lists.
#[tokio::test]
async fn decline_removes_pending_request() {
    let app = common::spawn().await;
    let (_, a_token) = register_user(&app, "dec_a@example.com").await;
    let (b_id, b_token) = register_user(&app, "dec_b@example.com").await;

    // A → B.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/request", app.base_url))
        .header("Authorization", format!("Bearer {a_token}"))
        .json(&json!({ "addressee_id": b_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);

    // Get request_id from B's list.
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

    // B declines.
    let resp = app
        .client
        .post(format!("{}/api/v1/friends/respond", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .json(&json!({ "request_id": request_id, "accept": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "decline must return 200");

    // B's list must show no pending requests.
    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .header("Authorization", format!("Bearer {b_token}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let incoming = body["data"]["incoming_pending"].as_array().unwrap();
    assert!(incoming.is_empty(), "after decline, incoming_pending must be empty");
    let accepted = body["data"]["accepted"].as_array().unwrap();
    assert!(accepted.is_empty(), "after decline, accepted must be empty");
}

/// Unauthenticated GET /friends → 401.
#[tokio::test]
async fn list_friends_unauthenticated_returns_401() {
    let app = common::spawn().await;

    let resp = app
        .client
        .get(format!("{}/api/v1/friends", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}
