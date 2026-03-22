//! Per-client rate limiting middleware

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use dashmap::DashMap;
use tokio::sync::Mutex;

use super::{Middleware, Next, RequestContext, ResponseContext};

/// Sliding-window token-bucket state per client
struct ClientState {
    /// Number of requests made in the current window
    count: u64,
    /// Start of the current window
    start: std::time::Instant,
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
        use std::time::Instant;
        let now = Instant::now();

        let mut entry = self.clients.entry(client_key.to_string()).or_insert_with(|| ClientState {
            count: 0,
            start: now,
        });

        // Reset window if expired
        if now.duration_since(entry.start) >= self.window {
            entry.count = 0;
            entry.start = now;
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
    #[allow(dead_code)]
    pub fn gc(&self) {
        use std::time::Instant;
        let now = Instant::now();
        self.clients.retain(|_, state| {
            now.duration_since(state.start) < self.window * 2
        });
    }
}

/// Gateway rate limiter wrapped as a middleware.
#[derive(Clone)]
pub struct GatewayRateLimiter {
    /// The rate limiter instance.
    limiter: Arc<RateLimiter>,
}

impl GatewayRateLimiter {
    /// Create a new gateway rate limiter.
    pub fn new(rpm: u32) -> Self {
        Self {
            limiter: Arc::new(RateLimiter::new(rpm as u64, Duration::from_secs(60))),
        }
    }

    /// Check if a request from the given IP is allowed.
    pub fn check(&self, ip: &str) -> bool {
        self.limiter.check(ip)
    }
}

#[async_trait]
impl Middleware for GatewayRateLimiter {
    async fn handle(&self, ctx: RequestContext, next: Next<'_>) -> ResponseContext {
        // Extract client IP from the request context
        let client_key = ctx
            .client_ip
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Check rate limit
        if !self.check(&client_key) {
            // Return 429 Too Many Requests
            let body = serde_json::json!({
                "error": {
                    "message": "Rate limit exceeded. Please try again later.",
                    "type": "rate_limit_error",
                    "code": "rate_limit_exceeded"
                }
            });
            let response = Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap();
            return ResponseContext::new(response);
        }

        // Continue to next middleware/handler
        next.run(ctx).await
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
