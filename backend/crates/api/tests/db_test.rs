mod common;

#[tokio::test]
async fn migrations_apply_and_tables_exist() {
    let app = common::spawn().await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn postgis_extension_is_enabled() {
    let app = common::spawn().await;
    let dist: f64 = sqlx::query_scalar(
        "SELECT ST_Distance(ST_MakePoint(0,0)::geography, ST_MakePoint(0,1)::geography)",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(dist > 0.0, "expected positive ST_Distance, got {dist}");
}
