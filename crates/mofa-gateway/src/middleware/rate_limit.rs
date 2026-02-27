//! Per-client rate limiting middleware

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Sliding-window token-bucket state per client
struct ClientState {
    /// Number of requests made in the current window
    count: u64,
    /// Start of the current window
    window_start: Instant,
}

/// Per-client IP rate limiter
///
/// Uses a fixed window algorithm: each client gets `max_requests` requests
/// per `window` duration. When the window expires the counter resets.
pub struct RateLimiter {
    clients: Arc<DashMap<String, ClientState>>,
    max_requests: u64,
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// * `max_requests` - allowed requests per window
    /// * `window`       - window duration
    pub fn new(max_requests: u64, window: Duration) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            max_requests,
            window,
        }
    }

    /// Return `true` if the request from `client_key` is allowed.
    ///
    /// The key is typically the client IP address, but can be any string
    /// (e.g. API key, user ID).
    pub fn check(&self, client_key: &str) -> bool {
        let now = Instant::now();

        let mut entry = self.clients.entry(client_key.to_string()).or_insert_with(|| ClientState {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(entry.window_start) >= self.window {
            entry.count = 0;
            entry.window_start = now;
        }

        if entry.count < self.max_requests {
            entry.count += 1;
            true
        } else {
            false
        }
    }

    /// Remove stale entries to keep memory usage bounded.
    ///
    /// Call this periodically (e.g. every minute) from a background task.
    pub fn gc(&self) {
        let now = Instant::now();
        self.clients.retain(|_, state| {
            now.duration_since(state.window_start) < self.window * 2
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max_requests() {
        let rl = RateLimiter::new(3, Duration::from_secs(60));
        assert!(rl.check("client1"));
        assert!(rl.check("client1"));
        assert!(rl.check("client1"));
        assert!(!rl.check("client1")); // 4th request denied
    }

    #[test]
    fn different_clients_are_independent() {
        let rl = RateLimiter::new(1, Duration::from_secs(60));
        assert!(rl.check("a"));
        assert!(!rl.check("a"));
        assert!(rl.check("b")); // different client, fresh limit
    }
}
