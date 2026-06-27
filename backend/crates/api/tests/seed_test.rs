mod common;

use walk4change_api::{config::AppConfig, seed};

/// Integration test: seed → assert DB state → seed again → assert idempotency.
#[tokio::test]
async fn seed_creates_demo_data_and_is_idempotent() {
    let app = common::spawn().await;
    let cfg = AppConfig::test_default();

    // ── First run ────────────────────────────────────────────────────────────
    let result = seed::run(&app.pool, &cfg)
        .await
        .expect("seed::run must succeed on first call");

    assert!(
        result.user_ids.len() >= 2,
        "seed must create at least 2 demo users, got {}",
        result.user_ids.len()
    );

    // Capture counts after first run for idempotency check.
    let user_count1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&app.pool)
        .await
        .expect("query user count");

    let friendship_count1: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM friendships WHERE status = 'accepted'",
    )
    .fetch_one(&app.pool)
    .await
    .expect("query friendships");

    assert!(
        friendship_count1 >= 1,
        "at least 1 accepted friendship must exist after seed"
    );

    // At least one active nature zone exists.
    let zone_count1: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM nature_zones WHERE active = true")
            .fetch_one(&app.pool)
            .await
            .expect("query nature_zones");

    assert!(
        zone_count1 >= 1,
        "at least 1 active nature_zone must exist after seed, got {}",
        zone_count1
    );
    assert_eq!(result.zone_count, zone_count1 as u32, "SeedResult.zone_count must match DB");

    // At least three rewards (discount, eco, sponsor spec).
    let reward_count1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rewards_catalog")
        .fetch_one(&app.pool)
        .await
        .expect("query rewards_catalog");

    assert!(
        reward_count1 >= 3,
        "at least 3 rewards (discount/eco/sponsor) must exist after seed, got {}",
        reward_count1
    );
    assert_eq!(result.reward_count, reward_count1 as u32, "SeedResult.reward_count must match DB");

    // ── Idempotency: second run must not error or duplicate ──────────────────
    let result2 = seed::run(&app.pool, &cfg)
        .await
        .expect("seed::run must succeed on second (idempotent) call");

    let user_count2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&app.pool)
        .await
        .expect("user count after second run");

    assert_eq!(
        user_count2, user_count1,
        "second seed run must not add new users; expected {} but got {}",
        user_count1, user_count2
    );

    let friendship_count2: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM friendships WHERE status = 'accepted'",
    )
    .fetch_one(&app.pool)
    .await
    .expect("friendship count after second run");

    assert_eq!(
        friendship_count2, friendship_count1,
        "second seed run must not add new friendships; expected {} but got {}",
        friendship_count1, friendship_count2
    );

    let zone_count2: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM nature_zones WHERE active = true")
            .fetch_one(&app.pool)
            .await
            .expect("zone count after second run");

    assert_eq!(zone_count2, zone_count1, "second seed run must not add more zones");
    assert_eq!(result2.zone_count, result.zone_count, "zone_count must be stable");

    let reward_count2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rewards_catalog")
        .fetch_one(&app.pool)
        .await
        .expect("reward count after second run");

    assert_eq!(reward_count2, reward_count1, "second seed run must not add more rewards");
    assert_eq!(result2.reward_count, result.reward_count, "reward_count must be stable");
}
