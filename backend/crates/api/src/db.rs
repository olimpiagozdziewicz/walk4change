use sqlx::{PgPool, postgres::PgPoolOptions};

/// Build a connection pool capped at 10 connections.
pub async fn make_pool(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(url)
        .await
}

/// Apply all pending SQLx migrations from the `migrations/` directory.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::migrate!("../../migrations")
        .run(pool)
        .await
        .map_err(|e| sqlx::Error::Migrate(Box::new(e)))
}
