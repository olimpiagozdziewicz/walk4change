use axum::{routing::get, Router};

pub fn router_health() -> Router {
    Router::new().route("/api/v1/health", get(|| async { "ok" }))
}
