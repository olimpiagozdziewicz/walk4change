mod common;

use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;
use walk4change_api::repo::reward as reward_repo;

/// Insert a bare `users` row directly (bypassing the HTTP register flow) so we
/// fully control points and avoid friendship setup. Returns the new user id.
async fn seed_user(pool: &sqlx::PgPool, email: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name) \
         VALUES ($1, $2, 'x', 'Seed')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("seed user");
    id
}

/// Give a user a `user_totals` row with the supplied total_points (spent = 0).
async fn seed_totals(pool: &sqlx::PgPool, user_id: Uuid, total_points: i64) {
    sqlx::query(
        "INSERT INTO user_totals (user_id, total_points, spent_points) VALUES ($1, $2, 0)",
    )
    .bind(user_id)
    .bind(Decimal::from(total_points))
    .execute(pool)
    .await
    .expect("seed totals");
}

/// Seed a reward. `stock = None` means unlimited. Returns the reward id.
async fn seed_reward(pool: &sqlx::PgPool, cost: i64, stock: Option<i32>) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rewards_catalog (id, title, cost_points, type, stock, active) \
         VALUES ($1, 'Test Reward', $2, 'discount', $3, true)",
    )
    .bind(id)
    .bind(Decimal::from(cost))
    .bind(stock)
    .execute(pool)
    .await
    .expect("seed reward");
    id
}

async fn spent_points(pool: &sqlx::PgPool, user_id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT spent_points FROM user_totals WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("query spent_points")
}

async fn stock_of(pool: &sqlx::PgPool, reward_id: Uuid) -> Option<i32> {
    sqlx::query_scalar("SELECT stock FROM rewards_catalog WHERE id = $1")
        .bind(reward_id)
        .fetch_one(pool)
        .await
        .expect("query stock")
}

#[tokio::test]
async fn redeem_succeeds_spends_points_and_returns_code() {
    let app = common::spawn().await;
    let user = seed_user(&app.pool, "redeemer@example.com").await;
    seed_totals(&app.pool, user, 100).await;
    let reward = seed_reward(&app.pool, 50, Some(1)).await;
    let token = app.token_for(user);

    // HTTP path: POST /api/v1/rewards/:id/redeem → 201 + Location.
    let resp = app
        .client
        .post(format!("{}/api/v1/rewards/{}/redeem", app.base_url, reward))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201, "redeem must return 201");
    assert_eq!(
        resp.headers()
            .get("location")
            .and_then(|v| v.to_str().ok()),
        Some("/api/v1/me/redemptions"),
        "Location header must point at redemptions",
    );

    let body: serde_json::Value = resp.json().await.unwrap();
    let code = body["data"]["code"].as_str().expect("code present");
    assert!(code.len() >= 16, "code must be >=16 chars, got {}", code.len());
    assert_eq!(body["data"]["status"], "reserved");
    assert_eq!(body["data"]["reward_id"], json!(reward.to_string()));

    // spent_points must equal the reward cost.
    assert_eq!(spent_points(&app.pool, user).await, Decimal::from(50));
    // stock must have been claimed.
    assert_eq!(stock_of(&app.pool, reward).await, Some(0));
}

#[tokio::test]
async fn second_redeem_of_sold_out_reward_is_unavailable() {
    let app = common::spawn().await;
    let user = seed_user(&app.pool, "soldout@example.com").await;
    seed_totals(&app.pool, user, 200).await;
    let reward = seed_reward(&app.pool, 50, Some(1)).await;

    // First redeem wins.
    reward_repo::redeem(&app.pool, reward, user)
        .await
        .expect("first redeem should succeed");

    // Second redeem of now-stock-0 reward → Conflict("unavailable").
    let err = reward_repo::redeem(&app.pool, reward, user)
        .await
        .expect_err("second redeem must fail");
    match err {
        walk4change_api::error::AppError::Conflict(msg) => assert_eq!(msg, "unavailable"),
        other => panic!("expected Conflict(unavailable), got {other:?}"),
    }
    assert_eq!(stock_of(&app.pool, reward).await, Some(0));
}

#[tokio::test]
async fn insufficient_points_rolls_back_stock() {
    let app = common::spawn().await;
    let user = seed_user(&app.pool, "broke@example.com").await;
    seed_totals(&app.pool, user, 10).await;
    let reward = seed_reward(&app.pool, 50, Some(1)).await;

    let err = reward_repo::redeem(&app.pool, reward, user)
        .await
        .expect_err("redeem must fail for insufficient points");
    match err {
        walk4change_api::error::AppError::Conflict(msg) => {
            assert_eq!(msg, "insufficient_points")
        }
        other => panic!("expected Conflict(insufficient_points), got {other:?}"),
    }

    // Rollback must have restored stock and left spent_points at 0.
    assert_eq!(
        stock_of(&app.pool, reward).await,
        Some(1),
        "stock must be restored after rollback"
    );
    assert_eq!(spent_points(&app.pool, user).await, Decimal::from(0));
}

#[tokio::test]
async fn list_redemptions_shows_redemption() {
    let app = common::spawn().await;
    let user = seed_user(&app.pool, "lister@example.com").await;
    seed_totals(&app.pool, user, 100).await;
    let reward = seed_reward(&app.pool, 50, Some(5)).await;

    let redemption = reward_repo::redeem(&app.pool, reward, user)
        .await
        .expect("redeem");

    // Repo path.
    let list = reward_repo::list_redemptions(&app.pool, user)
        .await
        .expect("list redemptions");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, redemption.id);
    assert_eq!(list[0].reward_id, reward);

    // HTTP path.
    let token = app.token_for(user);
    let resp = app
        .client
        .get(format!("{}/api/v1/me/redemptions", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"][0]["code"], json!(redemption.code));
}

#[tokio::test]
async fn list_rewards_returns_active_only() {
    let app = common::spawn().await;
    let active = seed_reward(&app.pool, 50, Some(1)).await;
    // Inactive reward — must be excluded.
    let inactive = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rewards_catalog (id, title, cost_points, type, active) \
         VALUES ($1, 'Inactive', 10, 'eco', false)",
    )
    .bind(inactive)
    .execute(&app.pool)
    .await
    .unwrap();

    let user = seed_user(&app.pool, "browser@example.com").await;
    let token = app.token_for(user);
    let resp = app
        .client
        .get(format!("{}/api/v1/rewards", app.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body["data"].as_array().unwrap();
    assert_eq!(arr.len(), 1, "only active rewards listed");
    assert_eq!(arr[0]["id"], json!(active.to_string()));
    assert_eq!(arr[0]["type"], "discount", "type field serialised as `type`");
}

#[tokio::test]
async fn unlimited_stock_stays_null() {
    let app = common::spawn().await;
    let user = seed_user(&app.pool, "unlimited@example.com").await;
    seed_totals(&app.pool, user, 500).await;
    let reward = seed_reward(&app.pool, 50, None).await;

    reward_repo::redeem(&app.pool, reward, user)
        .await
        .expect("redeem unlimited");
    // Unlimited stock must remain NULL (not decremented to -1 / NULL-1).
    assert_eq!(stock_of(&app.pool, reward).await, None);
    assert_eq!(spent_points(&app.pool, user).await, Decimal::from(50));

    // A second redeem still works since stock is unlimited.
    reward_repo::redeem(&app.pool, reward, user)
        .await
        .expect("second redeem unlimited");
    assert_eq!(stock_of(&app.pool, reward).await, None);
    assert_eq!(spent_points(&app.pool, user).await, Decimal::from(100));
}

#[tokio::test]
async fn concurrent_redeem_no_oversell() {
    let app = common::spawn().await;
    let user_a = seed_user(&app.pool, "race_a@example.com").await;
    let user_b = seed_user(&app.pool, "race_b@example.com").await;
    seed_totals(&app.pool, user_a, 100).await;
    seed_totals(&app.pool, user_b, 100).await;
    // Single unit of stock; two different users contend on the reward row.
    let reward = seed_reward(&app.pool, 50, Some(1)).await;

    let pool_a = app.pool.clone();
    let pool_b = app.pool.clone();
    let (res_a, res_b) = tokio::join!(
        reward_repo::redeem(&pool_a, reward, user_a),
        reward_repo::redeem(&pool_b, reward, user_b),
    );

    let oks = [res_a.is_ok(), res_b.is_ok()]
        .iter()
        .filter(|b| **b)
        .count();
    let errs = [&res_a, &res_b].iter().filter(|r| r.is_err()).count();
    assert_eq!(oks, 1, "exactly one redeem must succeed");
    assert_eq!(errs, 1, "exactly one redeem must fail");

    // The failure must be the unavailable conflict.
    let err = if res_a.is_err() { res_a.unwrap_err() } else { res_b.unwrap_err() };
    match err {
        walk4change_api::error::AppError::Conflict(msg) => assert_eq!(msg, "unavailable"),
        other => panic!("expected Conflict(unavailable), got {other:?}"),
    }

    // Stock must end at exactly 0 — never -1.
    assert_eq!(stock_of(&app.pool, reward).await, Some(0), "no oversell");
}
