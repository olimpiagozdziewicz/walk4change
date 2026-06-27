use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod repo;
pub mod response;
pub mod routes;
pub mod scoring;
pub mod state;

use auth::extractor::AuthUser;
use state::AppState;

/// Maximum request body size (64 KiB).
const MAX_BODY_BYTES: usize = 65_536;

/// Minimal, stateless router for unit tests that do not need [`AppState`].
///
/// Only the `/api/v1/health` endpoint is mounted.  Use [`build_app`] for
/// all integration-test and production serving scenarios.
pub fn router_health() -> Router {
    Router::new().route("/api/v1/health", get(|| async { "ok" }))
}

/// Single canonical application builder.
///
/// Combines all routes (health, _whoami, auth, profile) and applies baseline
/// middleware: HTTP tracing via [`TraceLayer`] and a [`DefaultBodyLimit`] of
/// [`MAX_BODY_BYTES`] bytes.
pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(|| async { "ok" }))
        .route("/api/v1/_whoami", get(whoami))
        .route("/api/v1/auth/register", post(auth::handlers::register))
        .route("/api/v1/auth/login", post(auth::handlers::login))
        .route("/api/v1/auth/logout", post(auth::handlers::logout))
        .route(
            "/api/v1/me",
            get(routes::profile::get_me).patch(routes::profile::patch_me),
        )
        .with_state(state)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
}

async fn whoami(auth: AuthUser) -> axum::Json<serde_json::Value> {
    response::data(auth.id)
}
