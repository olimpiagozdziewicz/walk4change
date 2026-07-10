use std::sync::Arc;

use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod mail;
pub mod models;
pub mod repo;
pub mod response;
pub mod routes;
pub mod scoring;
pub mod seed;
pub mod state;
pub mod util;
pub mod ws;

use state::AppState;
use util::ratelimit::RateLimiter;

/// Maximum request body size (64 KiB).
const MAX_BODY_BYTES: usize = 65_536;

/// Minimal, stateless router for unit tests that do not need [`AppState`].
///
/// Only the `/api/v1/health` endpoint is mounted.  Use [`build_app`] for
/// all integration-test and production serving scenarios.
pub fn router_health() -> Router {
    Router::new().route("/api/v1/health", get(|| async { "ok" }))
}

/// Extract the real client IP for rate limiting.
///
/// Behind Azure App Service the socket peer is the platform front-end, so all
/// users would share one rate-limit bucket (audit 2026-07-10: ~5 chat users
/// would exhaust the global bucket; worse, the auth bucket is 10/min). The
/// front-end APPENDS the true client IP as the LAST entry of
/// `X-Forwarded-For`, so taking the last valid entry is spoof-proof (anything
/// client-supplied sits earlier in the list). Falls back to the socket peer
/// when the header is absent/unparsable (e.g. direct connections in tests).
fn client_ip(request: &Request<Body>) -> Option<std::net::IpAddr> {
    let from_xff = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|value| {
            let last = value.split(',').next_back()?.trim();
            // Azure may append ":port"; IPv6 may come bracketed "[::1]:port".
            let host = if let Some(stripped) = last.strip_prefix('[') {
                stripped.split(']').next().unwrap_or(stripped)
            } else if last.matches(':').count() == 1 {
                // exactly one ':' => IPv4:port (bare IPv6 has ≥2 colons)
                last.split(':').next().unwrap_or(last)
            } else {
                last
            };
            host.parse::<std::net::IpAddr>().ok()
        });

    from_xff.or_else(|| {
        use axum::extract::ConnectInfo;
        use std::net::SocketAddr;
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
    })
}

/// Middleware: add security headers to every response.
async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=63072000"),
    );
    response
}

/// Build the CORS layer from a list of allowed origin strings.
///
/// Empty list → no `Access-Control-Allow-Origin` headers (browser cross-origin
/// blocked). Non-empty list → exact-match allowlist with credentials enabled.
fn build_cors(cors_origins: &[String]) -> CorsLayer {
    if cors_origins.is_empty() {
        return CorsLayer::new();
    }

    let origins: Vec<HeaderValue> = cors_origins
        .iter()
        .filter_map(|o| o.trim().parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(
            tower_http::cors::AllowOrigin::predicate(move |origin: &HeaderValue, _| {
                origins.contains(origin)
            }),
        )
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
        ])
}

/// Single canonical application builder.
///
/// Combines all routes and applies baseline middleware in order (outermost first):
/// 1. [`TraceLayer`] — HTTP request/response tracing
/// 2. Security headers — `X-Content-Type-Options`, `X-Frame-Options`, HSTS, Referrer-Policy
/// 3. CORS — exact origin allowlist with credentials (from config)
/// 4. Rate limiting — strict for `/api/v1/auth/*`, moderate global
/// 5. [`DefaultBodyLimit`] — 64 KiB cap on request bodies
pub fn build_app(state: AppState) -> Router {
    // ── rate limiters (created per build_app call for test isolation) ─────────
    let auth_limiter = Arc::new(RateLimiter::new(
        state.config.rate_limit_auth_max,
        state.config.rate_limit_window_secs,
    ));
    let global_limiter = Arc::new(RateLimiter::new(
        state.config.rate_limit_global_max,
        state.config.rate_limit_window_secs,
    ));

    // ── CORS layer ─────────────────────────────────────────────────────────────
    let cors = build_cors(&state.config.cors_origins);

    // ── rate-limit middleware closure ──────────────────────────────────────────
    let rate_limit_layer = {
        let auth_lim = Arc::clone(&auth_limiter);
        let glob_lim = Arc::clone(&global_limiter);
        middleware::from_fn(move |req: Request<Body>, next: Next| {
            let auth_lim = Arc::clone(&auth_lim);
            let glob_lim = Arc::clone(&glob_lim);
            async move {
                let ip = client_ip(&req).unwrap_or_else(|| {
                    // Fallback when both XFF and ConnectInfo are absent
                    // (e.g. health checks in unit tests).
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                });

                let is_auth = req.uri().path().starts_with("/api/v1/auth/");
                let limiter = if is_auth { &auth_lim } else { &glob_lim };

                match limiter.check(ip) {
                    Ok(()) => next.run(req).await,
                    Err(retry_after) => {
                        let limit = limiter.max_requests();
                        let reset_epoch = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                            .saturating_add(retry_after);

                        let mut resp = axum::Json(serde_json::json!({
                            "error": {
                                "code": "RATE_LIMITED",
                                "message": format!(
                                    "Rate limit exceeded. Retry after {} seconds.",
                                    retry_after
                                )
                            }
                        }))
                        .into_response();
                        *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
                        let headers = resp.headers_mut();
                        headers.insert(
                            header::RETRY_AFTER,
                            retry_after
                                .to_string()
                                .parse()
                                .unwrap_or(HeaderValue::from_static("60")),
                        );
                        headers.insert(
                            "x-ratelimit-limit",
                            limit
                                .to_string()
                                .parse()
                                .unwrap_or(HeaderValue::from_static("0")),
                        );
                        headers.insert(
                            "x-ratelimit-remaining",
                            HeaderValue::from_static("0"),
                        );
                        headers.insert(
                            "x-ratelimit-reset",
                            reset_epoch
                                .to_string()
                                .parse()
                                .unwrap_or(HeaderValue::from_static("0")),
                        );
                        resp
                    }
                }
            }
        })
    };

    Router::new()
        .route("/api/v1/health", get(|| async { "ok" }))
        .route("/api/v1/auth/register", post(auth::handlers::register))
        .route("/api/v1/auth/login", post(auth::handlers::login))
        .route("/api/v1/auth/logout", post(auth::handlers::logout))
        .route("/api/v1/auth/magic/request", post(auth::handlers::magic_request))
        .route("/api/v1/auth/magic/verify", post(auth::handlers::magic_verify))
        .route("/api/v1/auth/supabase", post(auth::handlers::supabase_exchange))
        .route(
            "/api/v1/me",
            get(routes::profile::get_me).patch(routes::profile::patch_me),
        )
        .route("/api/v1/me/stats", get(routes::stats::get_me_stats))
        .route(
            "/api/v1/eco/reports",
            get(routes::eco::list_reports).post(routes::eco::create_report),
        )
        .route(
            "/api/v1/me/eco-reports",
            get(routes::eco::list_my_reports),
        )
        .route("/api/v1/eco/reports/:id/like", post(routes::eco::toggle_like))
        .route(
            "/api/v1/eco/reports/:id/comments",
            get(routes::eco::list_comments).post(routes::eco::create_comment),
        )
        .route("/api/v1/friends/request", post(routes::friends::send_request))
        .route("/api/v1/friends/respond", post(routes::friends::respond))
        .route("/api/v1/friends", get(routes::friends::list))
        .route(
            "/api/v1/friends/:user_id",
            axum::routing::delete(routes::friends::remove_friend),
        )
        .route("/api/v1/users/search", get(routes::users::search))
        .route(
            "/api/v1/conversations",
            get(routes::messages::list_conversations),
        )
        .route(
            "/api/v1/messages/:user_id",
            get(routes::messages::get_conversation).post(routes::messages::send_message),
        )
        .route("/api/v1/walks", post(routes::walks::start_walk))
        .route("/api/v1/walks/open", get(routes::walks::open_walks))
        .route("/api/v1/me/walks", get(routes::walks::my_walks))
        .route("/api/v1/walks/join-by-code", post(routes::walks::join_by_code))
        .route(
            "/api/v1/walks/:id",
            get(routes::walks::get_walk).patch(routes::walks::patch_walk),
        )
        .route("/api/v1/walks/:id/join", post(routes::walks::join_walk))
        .route("/api/v1/walks/:id/leave", post(routes::walks::leave_walk))
        .route("/api/v1/walks/:id/stop", post(routes::walks::stop_walk))
        .route("/api/v1/walks/:id/track", get(routes::walks::track_walk))
        .route(
            "/api/v1/leaderboard",
            get(routes::leaderboard::get_leaderboard),
        )
        .route("/api/v1/rewards", get(routes::rewards::list_rewards))
        .route(
            "/api/v1/rewards/:id/redeem",
            post(routes::rewards::redeem_reward),
        )
        .route(
            "/api/v1/me/redemptions",
            get(routes::rewards::list_my_redemptions),
        )
        .route("/api/v1/ws", get(ws::handler::ws_handler))
        .with_state(state)
        // Innermost layer — body limit applied before routing logic.
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        // Rate-limiting (per-IP, two tiers).
        .layer(rate_limit_layer)
        // CORS preflight / headers.
        .layer(cors)
        // Security response headers on every reply.
        .layer(middleware::from_fn(security_headers))
        // Outermost — traces the full request round-trip including all middleware.
        .layer(TraceLayer::new_for_http())
}
