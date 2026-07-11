//! In-memory fixed-window rate limiter, generic over the bucket key.
//!
//! Keyed by `IpAddr` (default) for the HTTP middleware in [`crate::build_app`]
//! (auth + global tiers), and by `Uuid` for the per-ACCOUNT social-action
//! limits (audit 2026-07-10 N3 — the per-IP tier does not stop one account
//! from spamming messages/friend-requests/comments, and sockpuppets on
//! different IPs each got a fresh bucket).
//!
//! Uses `std::time::Instant` (monotonic) for window tracking.

use std::collections::HashMap;
use std::hash::Hash;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use uuid::Uuid;

struct Window {
    count: u32,
    start: Instant,
}

/// Thread-safe fixed-window rate limiter keyed by `K` (IP, account id, …).
pub struct RateLimiter<K: Eq + Hash + Copy = IpAddr> {
    buckets: Mutex<HashMap<K, Window>>,
    max_requests: u32,
    window: Duration,
    /// Monotonically increasing counter used to schedule periodic pruning.
    call_count: AtomicU64,
}

impl<K: Eq + Hash + Copy> RateLimiter<K> {
    /// Create a new limiter allowing `max_requests` per `window_secs` seconds.
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            max_requests,
            window: Duration::from_secs(window_secs),
            call_count: AtomicU64::new(0),
        }
    }

    /// Return the configured request limit for this window.
    pub fn max_requests(&self) -> u32 {
        self.max_requests
    }

    /// Check whether `ip` is within quota.
    ///
    /// Returns `Ok(())` if the request may proceed (counter is incremented).
    /// Returns `Err(retry_after_secs)` if quota is exhausted (≥1 second).
    pub fn check(&self, ip: K) -> Result<(), u64> {
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let now = Instant::now();

        // Opportunistically prune every 256 calls to bound map growth.
        // Pruning before accessing the current entry avoids borrow conflicts;
        // any stale entry for `ip` will simply be re-created fresh below.
        let prev = self.call_count.fetch_add(1, Ordering::Relaxed);
        if prev % 256 == 0 {
            let window = self.window;
            buckets.retain(|_, w| now.duration_since(w.start) < window);
        }

        let entry = buckets.entry(ip).or_insert_with(|| Window {
            count: 0,
            start: now,
        });

        // Roll the window when it has fully elapsed.
        if now.duration_since(entry.start) >= self.window {
            entry.count = 0;
            entry.start = now;
        }

        if entry.count >= self.max_requests {
            let elapsed = now.duration_since(entry.start);
            let remaining = self.window.saturating_sub(elapsed);
            // Report at least 1 second so clients never get Retry-After: 0.
            let retry_after = remaining.as_secs().max(1);
            return Err(retry_after);
        }

        entry.count += 1;
        Ok(())
    }
}

// ── per-ACCOUNT social-action limits (audit 2026-07-10 N3) ────────────────────
//
// Process-wide statics: the limits are account-scoped, so unlike the per-IP
// tiers they need no per-`build_app` isolation (tests create fresh random
// users, each with its own bucket). Windows are 60 s fixed.

/// Direct messages: 30/min per account (audit B1.6 recommendation).
static MESSAGE_LIMITER: LazyLock<RateLimiter<Uuid>> =
    LazyLock::new(|| RateLimiter::new(30, 60));

/// Friend requests: 15/min per account (invite-spam is the cheapest harassment).
static FRIEND_REQUEST_LIMITER: LazyLock<RateLimiter<Uuid>> =
    LazyLock::new(|| RateLimiter::new(15, 60));

/// Eco feed writes (comments + like toggles combined): 30/min per account.
static ECO_ACTION_LIMITER: LazyLock<RateLimiter<Uuid>> =
    LazyLock::new(|| RateLimiter::new(30, 60));

/// Per-account quota for `POST /messages/:user_id`.
pub fn check_message_quota(user: Uuid) -> Result<(), u64> {
    MESSAGE_LIMITER.check(user)
}

/// Per-account quota for `POST /friends/request`.
pub fn check_friend_request_quota(user: Uuid) -> Result<(), u64> {
    FRIEND_REQUEST_LIMITER.check(user)
}

/// Per-account quota for eco comment/like writes.
pub fn check_eco_action_quota(user: Uuid) -> Result<(), u64> {
    ECO_ACTION_LIMITER.check(user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn allows_up_to_max_requests() {
        let lim = RateLimiter::new(3, 60);
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert!(lim.check(ip).is_ok());
        assert!(lim.check(ip).is_ok());
        assert!(lim.check(ip).is_ok());
        assert!(lim.check(ip).is_err());
    }

    #[test]
    fn different_ips_have_separate_buckets() {
        let lim = RateLimiter::new(1, 60);
        let ip_a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip_b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        assert!(lim.check(ip_a).is_ok());
        assert!(lim.check(ip_b).is_ok());
        assert!(lim.check(ip_a).is_err());
    }

    #[test]
    fn retry_after_is_at_least_one() {
        let lim = RateLimiter::new(1, 60);
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        lim.check(ip).unwrap();
        let retry = lim.check(ip).unwrap_err();
        assert!(retry >= 1);
    }

    #[test]
    fn prune_removes_stale_entries() {
        let lim = RateLimiter::new(100, 60);
        let ip_stale1 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip_stale2 = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let ip_fresh = IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3));

        // Directly insert stale entries whose window has long expired.
        {
            let mut buckets = lim.buckets.lock().unwrap();
            buckets.insert(
                ip_stale1,
                Window {
                    count: 5,
                    start: Instant::now() - Duration::from_secs(120),
                },
            );
            buckets.insert(
                ip_stale2,
                Window {
                    count: 3,
                    start: Instant::now() - Duration::from_secs(61),
                },
            );
        }

        // call_count starts at 0, so the very first check() triggers a prune
        // (prev == 0, and 0 % 256 == 0).
        lim.check(ip_fresh).unwrap();

        let remaining = lim.buckets.lock().unwrap().len();
        assert_eq!(remaining, 1, "stale entries should have been pruned; only ip_fresh remains");
    }
}
