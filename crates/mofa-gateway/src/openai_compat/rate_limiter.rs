//! Token-bucket rate limiter for the inference gateway.
//!
//! Each unique client IP gets its own token bucket. Buckets are refilled
//! once per minute. The rate limiter is wrapped in `Arc<Mutex<…>>` so it
//! can be shared across concurrent axum request handlers.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Instant;

/// A single token bucket for one client IP.
#[derive(Debug)]
struct Bucket {
    /// Remaining tokens in the current window.
    tokens: u32,
    /// When this bucket's window started.
    window_start: Instant,
}

/// Per-IP token-bucket rate limiter.
///
/// # Usage
///
/// ```rust,ignore
/// use std::sync::{Arc, Mutex};
/// use mofa_gateway::openai_compat::rate_limiter::TokenBucketLimiter;
///
/// let limiter = Arc::new(Mutex::new(TokenBucketLimiter::new(60)));
/// let allowed = limiter.lock().unwrap().check_and_consume("127.0.0.1".parse().unwrap());
/// assert!(allowed);
/// ```
#[derive(Debug)]
pub struct TokenBucketLimiter {
    buckets: HashMap<IpAddr, Bucket>,
    /// Maximum requests allowed per 60-second window.
    rpm: u32,
}

impl TokenBucketLimiter {
    /// Create a new limiter with the given requests-per-minute budget.
    pub fn new(rpm: u32) -> Self {
        Self {
            buckets: HashMap::new(),
            rpm,
        }
    }

    /// Attempt to consume one token for `client`.
    ///
    /// Returns `true` if the request is allowed, `false` if the bucket is
    /// exhausted for the current 60-second window.
    pub fn check_and_consume(&mut self, client: IpAddr) -> bool {
        const WINDOW_SECS: u64 = 60;

        let rpm = self.rpm;
        let now = Instant::now();

        let bucket = self.buckets.entry(client).or_insert_with(|| Bucket {
            tokens: rpm,
            window_start: now,
        });

        // Refill if the 60-second window has elapsed.
        if now.duration_since(bucket.window_start).as_secs() >= WINDOW_SECS {
            bucket.tokens = rpm;
            bucket.window_start = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Returns remaining tokens for `client` without consuming one.
    #[allow(dead_code)]
    pub fn remaining(&mut self, client: IpAddr) -> u32 {
        const WINDOW_SECS: u64 = 60;
        let rpm = self.rpm;
        let now = Instant::now();

        let bucket = self.buckets.entry(client).or_insert_with(|| Bucket {
            tokens: rpm,
            window_start: now,
        });

        if now.duration_since(bucket.window_start).as_secs() >= WINDOW_SECS {
            bucket.tokens = rpm;
            bucket.window_start = now;
        }

        bucket.tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::str::FromStr;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }

    #[test]
    fn test_bucket_allows_up_to_rpm() {
        let mut limiter = TokenBucketLimiter::new(5);
        let client = ip("192.168.1.1");
        for _ in 0..5 {
            assert!(limiter.check_and_consume(client), "should allow up to rpm");
        }
    }

    #[test]
    fn test_bucket_rejects_over_limit() {
        let mut limiter = TokenBucketLimiter::new(3);
        let client = ip("10.0.0.1");
        for _ in 0..3 {
            limiter.check_and_consume(client);
        }
        assert!(
            !limiter.check_and_consume(client),
            "4th request should be rejected"
        );
    }

    #[test]
    fn test_different_ips_get_separate_buckets() {
        let mut limiter = TokenBucketLimiter::new(1);
        let c1 = ip("1.2.3.4");
        let c2 = ip("5.6.7.8");

        assert!(limiter.check_and_consume(c1));
        assert!(
            limiter.check_and_consume(c2),
            "different IP should have its own bucket"
        );
    }

    #[test]
    fn test_remaining_starts_at_rpm() {
        let mut limiter = TokenBucketLimiter::new(10);
        let client = ip("1.1.1.1");
        assert_eq!(limiter.remaining(client), 10);
    }

    #[test]
    fn test_remaining_decrements_after_consume() {
        let mut limiter = TokenBucketLimiter::new(5);
        let client = ip("2.2.2.2");
        limiter.check_and_consume(client);
        limiter.check_and_consume(client);
        assert_eq!(limiter.remaining(client), 3);
    }
}
