//! Thin binary wrapper around [`walk4change_api::seed::run`].
//!
//! Usage:
//!   cargo run -p walk4change-api --bin seed
//!
//! Reads DATABASE_URL and JWT_SECRET (plus other config) from the environment
//! or a `.env` file.  Set SEED_PASSWORD to control the demo-user password;
//! otherwise a random password is generated and printed once.
//!
//! Idempotent: safe to run repeatedly.

use walk4change_api::{auth::jwt, config::AppConfig, db, seed};

#[tokio::main]
async fn main() {
    // Initialise tracing so sqlx and app errors appear in the terminal.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = AppConfig::from_env().expect("Failed to load config — check env vars");

    let pool = db::make_pool(&cfg.database_url)
        .await
        .expect("Failed to connect to the database");

    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    let result = seed::run(&pool, &cfg)
        .await
        .expect("Seed failed");

    let labels = ["Ana (ana@demo.walk4change)", "Bek (bek@demo.walk4change)"];

    println!("=== walk4change demo seed ===");
    println!("Demo users:");
    for (i, id) in result.user_ids.iter().enumerate() {
        let label = labels.get(i).copied().unwrap_or("user");
        let token = jwt::encode(&cfg, *id)
            .unwrap_or_else(|_| "<token error>".into());
        println!("  {label}");
        println!("    id    = {id}");
        println!("    token = {token}");
    }
    println!("Demo password: {}", result.password);
    println!("Active nature zones : {}", result.zone_count);
    println!("Rewards in catalog  : {}", result.reward_count);
    println!("Done — idempotent, safe to run again.");
}
