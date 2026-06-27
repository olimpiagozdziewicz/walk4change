use std::sync::Arc;
use sqlx::PgPool;
use crate::config::AppConfig;
use crate::ws::hub::Hub;

/// Shared application state threaded through every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<AppConfig>,
    pub hub: Hub,
}
