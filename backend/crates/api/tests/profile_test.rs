mod common;

use serde_json::json;

/// Helper: register a user, return the token.
async fn register_user(app: &common::TestApp, email: &str) -> String {
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
    assert_eq!(resp.status().as_u16(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    body["token"].as_str().unwrap().to_owned()
}

/// GET /api/v1/me without a token → 401.
#[tokio::test]
async fn get_me_without_token_returns_401() {
    let app = common::spawn().await;

    let resp = app
        .client
        .get(format!("{}/api/v1/me", app.base_url))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 401);
}

/// GET /api/v1/me with a valid token returns the authenticated user's profile.
#[tokio::test]
async fn get_me_returns_profile() {
    let app = common::spawn().await;
    let token = register_user(&app, "me_getter@example.com").await;

    let resp = app
        .client
        .get(format!("{}/api/v1/me", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.expect("body is not JSON");
    assert_eq!(
        body["data"]["email"].as_str(),
        Some("me_getter@example.com"),
        "email must match registered user"
    );
    assert_eq!(
        body["data"]["display_name"].as_str(),
        Some("TestUser"),
        "display_name must match"
    );
    assert!(body["data"]["id"].is_string(), "id must be present");
    assert!(body["data"]["created_at"].is_string(), "created_at must be present");
}

/// PATCH /api/v1/me updates display_name and bio; a follow-up GET reflects the changes.
#[tokio::test]
async fn patch_me_updates_display_name_and_bio() {
    let app = common::spawn().await;
    let token = register_user(&app, "patcher@example.com").await;

    let patch_resp = app
        .client
        .patch(format!("{}/api/v1/me", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "display_name": "Patched Name",
            "bio": "Hello world"
        }))
        .send()
        .await
        .expect("patch request failed");

    assert_eq!(patch_resp.status().as_u16(), 200);
    let patch_body: serde_json::Value = patch_resp.json().await.expect("patch body is not JSON");
    assert_eq!(
        patch_body["data"]["display_name"].as_str(),
        Some("Patched Name"),
        "display_name must be updated"
    );
    assert_eq!(
        patch_body["data"]["bio"].as_str(),
        Some("Hello world"),
        "bio must be updated"
    );

    // Follow-up GET must reflect the changes.
    let get_resp = app
        .client
        .get(format!("{}/api/v1/me", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get request failed");

    assert_eq!(get_resp.status().as_u16(), 200);
    let get_body: serde_json::Value = get_resp.json().await.expect("get body is not JSON");
    assert_eq!(
        get_body["data"]["display_name"].as_str(),
        Some("Patched Name"),
        "GET after PATCH must show updated display_name"
    );
    assert_eq!(
        get_body["data"]["bio"].as_str(),
        Some("Hello world"),
        "GET after PATCH must show updated bio"
    );
    // email should be unchanged
    assert_eq!(
        get_body["data"]["email"].as_str(),
        Some("patcher@example.com"),
        "email must be unchanged after PATCH"
    );
}

/// PATCH /api/v1/me with an empty display_name → 422 Validation error.
#[tokio::test]
async fn patch_me_with_empty_display_name_returns_422() {
    let app = common::spawn().await;
    let token = register_user(&app, "empty_name@example.com").await;

    let resp = app
        .client
        .patch(format!("{}/api/v1/me", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "display_name": "" }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 422);
}

/// PATCH /api/v1/me without token → 401.
#[tokio::test]
async fn patch_me_without_token_returns_401() {
    let app = common::spawn().await;

    let resp = app
        .client
        .patch(format!("{}/api/v1/me", app.base_url))
        .json(&json!({ "display_name": "Ghost" }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 401);
}

/// PATCH /api/v1/me with interests updates the interests array.
#[tokio::test]
async fn patch_me_updates_interests() {
    let app = common::spawn().await;
    let token = register_user(&app, "interests@example.com").await;

    let resp = app
        .client
        .patch(format!("{}/api/v1/me", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "interests": ["hiking", "nature"] }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.expect("body is not JSON");
    let interests = body["data"]["interests"].as_array().expect("interests must be array");
    assert_eq!(interests.len(), 2);
    assert!(interests.iter().any(|i| i.as_str() == Some("hiking")));
    assert!(interests.iter().any(|i| i.as_str() == Some("nature")));
}
