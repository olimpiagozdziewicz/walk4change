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
pub mod util;
pub mod ws;

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
        .route("/api/v1/friends/request", post(routes::friends::send_request))
        .route("/api/v1/friends/respond", post(routes::friends::respond))
        .route("/api/v1/friends", get(routes::friends::list))
        .route("/api/v1/walks", post(routes::walks::start_walk))
        .route("/api/v1/walks/:id", get(routes::walks::get_walk))
        .route("/api/v1/walks/:id/join", post(routes::walks::join_walk))
        .route("/api/v1/walks/:id/leave", post(routes::walks::leave_walk))
        .route("/api/v1/walks/:id/stop", post(routes::walks::stop_walk))
        .route("/api/v1/walks/:id/track", get(routes::walks::track_walk))
        .route(
            "/api/v1/leaderboard",
            get(routes::leaderboard::get_leaderboard),
        )
        .route("/api/v1/ws", get(ws::handler::ws_handler))
        .with_state(state)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
}

async fn whoami(auth: AuthUser) -> axum::Json<serde_json::Value> {
    response::data(auth.id)
}
