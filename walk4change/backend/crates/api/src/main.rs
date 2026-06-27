use std::sync::Arc;
use walk4change_api::{build_app, config::AppConfig, db, state::AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = AppConfig::from_env().expect("invalid configuration");
    let pool = db::make_pool(&config.database_url)
        .await
        .expect("failed to connect to database");

    db::run_migrations(&pool)
        .await
        .expect("migrations failed");

    let bind_addr = config.bind_addr.clone();
    let state = AppState {
        pool,
        config: Arc::new(config),
        hub: (),
    };

    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|_| panic!("failed to bind to {bind_addr}"));

    tracing::info!("listening on {bind_addr}");
    axum::serve(listener, app).await.unwrap();
}
