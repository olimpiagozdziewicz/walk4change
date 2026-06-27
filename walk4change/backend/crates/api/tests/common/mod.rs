#![allow(dead_code)]

use std::sync::Arc;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::net::TcpListener;
use uuid::Uuid;
use walk4change_api::{auth::jwt, build_app, config::AppConfig, db, state::AppState};

/// Shared test app handle.
///
/// `_db_guard` holds a single-connection pool that owns the Postgres session
/// advisory lock (key 727274).  When `TestApp` is dropped the pool closes,
/// Postgres releases the lock, and the next queued test may proceed.
#[allow(dead_code)]
pub struct TestApp {
    pub pool: PgPool,
    pub base_url: String,
    pub client: reqwest::Client,
    pub config: Arc<AppConfig>,
    _db_guard: PgPool,
}

impl TestApp {
    /// Mint a valid signed JWT for any user id using the same secret
    /// the test server was started with.
    pub fn token_for(&self, user_id: Uuid) -> String {
        jwt::encode(&self.config, user_id).expect("failed to encode test token")
    }
}

/// Spawn a test instance of the API server against `TEST_DATABASE_URL`.
///
/// - Acquires a Postgres session advisory lock so that concurrent test
///   processes are serialized (one truncate + setup at a time).
/// - Connects a pool
/// - Runs pending migrations
/// - TRUNCATEs all tables for test isolation
/// - Binds the full router (with state) to an ephemeral port
/// - Returns pool, config, base URL, and a reqwest client
pub async fn spawn() -> TestApp {
    let db_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set for integration tests");

    // ── advisory lock ────────────────────────────────────────────────────────
    let guard_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("failed to connect guard pool for advisory lock");

    sqlx::query("SELECT pg_advisory_lock(727274)")
        .execute(&guard_pool)
        .await
        .expect("failed to acquire pg advisory lock");

    // ── main pool + migrations + truncate ────────────────────────────────────
    let pool = db::make_pool(&db_url)
        .await
        .expect("failed to connect to test database");

    db::run_migrations(&pool)
        .await
        .expect("migrations failed in test harness");

    sqlx::query(
        "TRUNCATE \
            reward_redemptions, user_totals, location_pings, walk_participants, \
            walk_sessions, friendships, nature_zones, users, rewards_catalog \
         RESTART IDENTITY CASCADE",
    )
    .execute(&pool)
    .await
    .expect("failed to truncate tables for test isolation");

    let mut config = AppConfig::test_default();
    config.database_url = db_url;
    config.jwt_secret = "test-secret-that-is-at-least-32-chars!!".into();
    let config = Arc::new(config);

    let state = AppState {
        pool: pool.clone(),
        config: Arc::clone(&config),
        hub: (),
    };

    let app = build_app(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind ephemeral port");
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    TestApp {
        pool,
        base_url,
        client: reqwest::Client::new(),
        config,
        _db_guard: guard_pool,
    }
}
