mod common;

use uuid::Uuid;

/// No credential at all → 401.
#[tokio::test]
async fn whoami_without_token_returns_401() {
    let app = common::spawn().await;

    let resp = app
        .client
        .get(format!("{}/api/v1/_whoami", app.base_url))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 401);
}

/// Valid Bearer token → 200, body contains the expected UUID.
#[tokio::test]
async fn whoami_with_valid_bearer_returns_200_and_uuid() {
    let app = common::spawn().await;
    let user_id = Uuid::new_v4();
    let token = app.token_for(user_id);

    let resp = app
        .client
        .get(format!("{}/api/v1/_whoami", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 200);

    let body: serde_json::Value = resp.json().await.expect("body is not JSON");
    let returned_id = body["data"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("data field should be a UUID string");

    assert_eq!(returned_id, user_id);
}

/// Valid `wc_session` cookie → 200, body contains the expected UUID.
#[tokio::test]
async fn whoami_with_valid_cookie_returns_200_and_uuid() {
    let app = common::spawn().await;
    let user_id = Uuid::new_v4();
    let token = app.token_for(user_id);

    let resp = app
        .client
        .get(format!("{}/api/v1/_whoami", app.base_url))
        .header("Cookie", format!("wc_session={token}"))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 200);

    let body: serde_json::Value = resp.json().await.expect("body is not JSON");
    let returned_id = body["data"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("data field should be a UUID string");

    assert_eq!(returned_id, user_id);
}

/// Malformed / invalid token → 401.
#[tokio::test]
async fn whoami_with_invalid_token_returns_401() {
    let app = common::spawn().await;

    let resp = app
        .client
        .get(format!("{}/api/v1/_whoami", app.base_url))
        .header("Authorization", "Bearer not.a.valid.jwt")
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 401);
}
