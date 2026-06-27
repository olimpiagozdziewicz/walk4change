use axum::{
    routing::{get, post},
    Router,
};

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

/// Minimal router used in unit tests that do not need application state.
pub fn router_health() -> Router {
    Router::new().route("/api/v1/health", get(|| async { "ok" }))
}

/// Full application router backed by [`AppState`].
pub fn router(state: AppState) -> Router {
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
}

async fn whoami(auth: AuthUser) -> axum::Json<serde_json::Value> {
    response::data(auth.id)
}
