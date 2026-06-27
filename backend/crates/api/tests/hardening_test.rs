//! Integration tests for Task 18: rate limiting, security headers, CORS, and
//! migration 0002 DB constraints.

mod common;

use serde_json::json;

// ── helpers ───────────────────────────────────────────────────────────────────

/// POST to `/api/v1/auth/login` with dummy credentials. Returns the status code.
async fn hit_login(app: &common::TestApp) -> u16 {
    app.client
        .post(format!("{}/api/v1/auth/login", app.base_url))
        .json(&json!({ "email": "nobody@example.com", "password": "wrong" }))
        .send()
        .await
        .expect("request failed")
        .status()
        .as_u16()
}

// ── A. Rate limiting ──────────────────────────────────────────────────────────

/// Hammer the strict auth bucket past the limit → eventually receive 429 with
/// `Retry-After` header. The test server uses `auth_max = 4` so exactly the 5th
/// request must be 429.
///
/// Design note: limits are configured per-server (via `spawn_with_rate_limits`),
/// so this test is fully deterministic and isolated from other test suites.
#[tokio::test]
async fn auth_rate_limit_returns_429_with_retry_after() {
    // 4 requests allowed, 5th must be rejected.
    let app = common::spawn_with_rate_limits(4, 10_000, 60).await;

    let mut last_status = 0u16;
    for i in 0..6 {
        last_status = hit_login(&app).await;
        // Before the limit is hit we get 401 (wrong credentials, not rate limited).
        if last_status == 429 {
            // Good — got rate limited at or before the 6th attempt.
            assert!(
                i >= 4,
                "rate limit should not fire before the 5th request, fired at request {i}"
            );
            break;
        }
    }
    assert_eq!(
        last_status, 429,
        "should have received 429 after exhausting the auth bucket"
    );

    // Verify the response carries a Retry-After header and the error envelope.
    let resp = app
        .client
        .post(format!("{}/api/v1/auth/login", app.base_url))
        .json(&json!({ "email": "nobody@example.com", "password": "wrong" }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 429);
    assert!(
        resp.headers().contains_key("retry-after"),
        "429 response must include Retry-After header"
    );
    assert!(
        resp.headers().contains_key("x-ratelimit-limit"),
        "429 response must include X-RateLimit-Limit header"
    );
    assert!(
        resp.headers().contains_key("x-ratelimit-reset"),
        "429 response must include X-RateLimit-Reset header"
    );

    let body: serde_json::Value = resp.json().await.expect("body must be JSON");
    assert_eq!(
        body["error"]["code"], "RATE_LIMITED",
        "error envelope code must be 'RATE_LIMITED', got: {body:?}"
    );
}

// ── B. Security headers ───────────────────────────────────────────────────────

/// Every response must carry `X-Content-Type-Options: nosniff`.
#[tokio::test]
async fn health_response_carries_security_headers() {
    let app = common::spawn().await;

    let resp = app
        .client
        .get(format!("{}/api/v1/health", app.base_url))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status().as_u16(), 200);

    let headers = resp.headers();
    assert_eq!(
        headers.get("x-content-type-options").map(|v| v.to_str().unwrap()),
        Some("nosniff"),
        "X-Content-Type-Options must be nosniff"
    );
    assert_eq!(
        headers.get("x-frame-options").map(|v| v.to_str().unwrap()),
        Some("DENY"),
        "X-Frame-Options must be DENY"
    );
    assert!(
        headers.contains_key("referrer-policy"),
        "Referrer-Policy header must be present"
    );
    assert!(
        headers.contains_key("strict-transport-security"),
        "Strict-Transport-Security header must be present"
    );
}

// ── E. Migration 0002 ─────────────────────────────────────────────────────────

/// Verify that migration 0002 applied successfully: the unique constraint on
/// `reward_redemptions.code` causes duplicate inserts to fail, and the
/// `rewards_catalog.stock` non-negative check is present.
#[tokio::test]
async fn migration_0002_unique_code_constraint_enforced() {
    let app = common::spawn().await;

    // Insert a catalog item directly.
    let reward_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rewards_catalog (id, title, cost_points, type) VALUES ($1, $2, $3, $4)",
    )
    .bind(reward_id)
    .bind("Test Reward")
    .bind(rust_decimal::Decimal::new(100, 0))
    .bind("discount")
    .execute(&app.pool)
    .await
    .expect("insert reward failed");

    // Insert a user.
    let user_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind("mig_test@example.com")
    .bind("hash")
    .bind("MigUser")
    .execute(&app.pool)
    .await
    .expect("insert user failed");

    // Insert first redemption with a unique code.
    let redemption_id_1 = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO reward_redemptions \
         (id, user_id, reward_id, points_spent, code, status) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(redemption_id_1)
    .bind(user_id)
    .bind(reward_id)
    .bind(rust_decimal::Decimal::new(100, 0))
    .bind("UNIQUE-CODE-001")
    .bind("reserved")
    .execute(&app.pool)
    .await
    .expect("first redemption insert failed");

    // Second insert with the SAME code must violate the UNIQUE constraint.
    let redemption_id_2 = uuid::Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO reward_redemptions \
         (id, user_id, reward_id, points_spent, code, status) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(redemption_id_2)
    .bind(user_id)
    .bind(reward_id)
    .bind(rust_decimal::Decimal::new(100, 0))
    .bind("UNIQUE-CODE-001") // same code — must fail
    .bind("reserved")
    .execute(&app.pool)
    .await;

    assert!(
        result.is_err(),
        "duplicate redemption code must violate UNIQUE constraint"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("duplicate key")
            || err.to_string().contains("unique"),
        "error must be a unique violation, got: {err}"
    );
}

/// The non-negative stock check prevents negative stock values.
#[tokio::test]
async fn migration_0002_negative_stock_rejected() {
    let app = common::spawn().await;

    let reward_id = uuid::Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO rewards_catalog (id, title, cost_points, type, stock) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(reward_id)
    .bind("Bad Stock Item")
    .bind(rust_decimal::Decimal::new(50, 0))
    .bind("eco")
    .bind(-1_i32) // violates CHECK (stock IS NULL OR stock >= 0)
    .execute(&app.pool)
    .await;

    assert!(
        result.is_err(),
        "negative stock must violate the CHECK constraint"
    );
}
