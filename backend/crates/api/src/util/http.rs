//! Shared outbound HTTP client with timeouts (audit debt: every
//! `reqwest::Client::new()` call had NO timeout, so a hung Supabase endpoint
//! could pin a request handler / spawned task indefinitely).

use std::sync::LazyLock;
use std::time::Duration;

/// Total per-request timeout for calls to Supabase (Auth, Admin, Storage).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// TCP/TLS connect timeout.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Process-wide client (connection pooling + timeouts). Cheap to clone.
pub fn client() -> reqwest::Client {
    CLIENT.clone()
}
