mod common;

use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

// ── seeding helpers ──────────────────────────────────────────────────────────

/// Insert a user + matching user_totals row directly into the DB.
async fn seed_user_with_points(
    pool: &PgPool,
    email: &str,
    display_name: &str,
    total_points: Decimal,
) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name) \
         VALUES ($1, $2, 'x', $3)",
    )
    .bind(user_id)
    .bind(email)
    .bind(display_name)
    .execute(pool)
    .await
    .expect("seed user");

    sqlx::query(
        "INSERT INTO user_totals (user_id, total_points) \
         VALUES ($1, $2)",
    )
    .bind(user_id)
    .bind(total_points)
    .execute(pool)
    .await
    .expect("seed user_totals");

    user_id
}

/// Insert a bare user (no totals) for auth purposes only.
async fn seed_bare_user(pool: &PgPool, email: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name) \
         VALUES ($1, $2, 'x', 'Auth')",
    )
    .bind(user_id)
    .bind(email)
    .execute(pool)
    .await
    .expect("seed bare user");
    user_id
}

// ── tests ────────────────────────────────────────────────────────────────────

/// `GET /api/v1/leaderboard` without a token → 401.
#[tokio::test]
async fn leaderboard_requires_auth() {
    let app = common::spawn().await;

    let resp = app
        .client
        .get(format!("{}/api/v1/leaderboard", app.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 401);
}

/// Page 1 of 2: returns 2 entries in descending order; meta reflects the full 3-row total.
#[tokio::test]
async fn leaderboard_page1_returns_top_entries_ordered_desc() {
    let app = common::spawn().await;

    seed_user_with_points(&app.pool, "lb_alice@example.com", "Alice", Decimal::new(300, 0)).await;
    seed_user_with_points(
        &app.pool,
        "lb_charlie@example.com",
        "Charlie",
        Decimal::new(200, 0),
    )
    .await;
    seed_user_with_points(&app.pool, "lb_bob@example.com", "Bob", Decimal::new(100, 0)).await;

    // Any valid user can auth; use a bare user (no totals row).
    let requester = seed_bare_user(&app.pool, "lb_req@example.com").await;
    let token = app.token_for(requester);

    let resp = app
        .client
        .get(format!("{}/api/v1/leaderboard?page=1&per_page=2", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let data = &body["data"];
    assert!(data.is_array(), "data must be an array, got {body:?}");
    let entries = data.as_array().unwrap();
    assert_eq!(entries.len(), 2, "page 1 / per_page 2 must return 2 entries");

    // First place: Alice (300 pts).
    assert_eq!(entries[0]["display_name"], "Alice", "first must be Alice");
    // Second place: Charlie (200 pts).
    assert_eq!(entries[1]["display_name"], "Charlie", "second must be Charlie");

    let meta = &body["meta"];
    assert_eq!(meta["total"], 3, "meta.total must be 3");
    assert_eq!(meta["page"], 1);
    assert_eq!(meta["per_page"], 2);
    assert_eq!(meta["total_pages"], 2);
}

/// Page 2 of 2: returns the single remaining entry.
#[tokio::test]
async fn leaderboard_page2_returns_remaining_entry() {
    let app = common::spawn().await;

    let alice =
        seed_user_with_points(&app.pool, "lb2_alice@example.com", "Alice", Decimal::new(300, 0))
            .await;
    seed_user_with_points(
        &app.pool,
        "lb2_charlie@example.com",
        "Charlie",
        Decimal::new(200, 0),
    )
    .await;
    seed_user_with_points(&app.pool, "lb2_bob@example.com", "Bob", Decimal::new(100, 0)).await;

    let token = app.token_for(alice);

    let resp = app
        .client
        .get(format!("{}/api/v1/leaderboard?page=2&per_page=2", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let entries = body["data"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "page 2 must have the one remaining entry");
    assert_eq!(entries[0]["display_name"], "Bob", "last entry must be Bob");
}

/// `per_page > 100` → 422 Validation error.
#[tokio::test]
async fn leaderboard_per_page_over_100_returns_422() {
    let app = common::spawn().await;

    let user = seed_bare_user(&app.pool, "lb_cap@example.com").await;
    let token = app.token_for(user);

    let resp = app
        .client
        .get(format!("{}/api/v1/leaderboard?per_page=1000", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 422);
}

/// `page < 1` → 422 Validation error.
#[tokio::test]
async fn leaderboard_page_zero_returns_422() {
    let app = common::spawn().await;

    let user = seed_bare_user(&app.pool, "lb_pg0@example.com").await;
    let token = app.token_for(user);

    let resp = app
        .client
        .get(format!("{}/api/v1/leaderboard?page=0", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 422);
}

/// Default (no query params) → 200 with sensible defaults applied.
#[tokio::test]
async fn leaderboard_defaults_work() {
    let app = common::spawn().await;

    let user = seed_bare_user(&app.pool, "lb_default@example.com").await;
    let token = app.token_for(user);

    let resp = app
        .client
        .get(format!("{}/api/v1/leaderboard", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["meta"]["page"], json!(1));
    assert_eq!(body["meta"]["per_page"], json!(20));
}
