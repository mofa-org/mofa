//! Token-bucket rate limiter — concrete implementation of the kernel contract.
//!
//! # How it works
//!
//! Each unique key gets its own [`TokenBucket`].  Tokens refill lazily on
//! every `check_and_consume` call — no background timer, no spawned tasks.
//! The refill amount is proportional to how much wall-clock time has elapsed
//! since the last call, capped at the bucket capacity.
//!
//! # Keying strategies
//!
//! Two strategies are supported:
//!
//! - [`KeyStrategy::PerAgent`] — one bucket per `agent_id` (from the matched route)
//! - [`KeyStrategy::PerClient`] — one bucket per caller IP string
//!
//! The caller is responsible for passing the correct key to
//! [`TokenBucketRateLimiter::check_and_consume`].

use std::time::Instant;

use dashmap::DashMap;
pub use mofa_kernel::{GatewayRateLimiter, KeyStrategy, RateLimitDecision, RateLimiterConfig};

// ─────────────────────────────────────────────────────────────────────────────
// TokenBucket (internal per-key state)
// ─────────────────────────────────────────────────────────────────────────────

struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, refill_rate: u32) -> Self {
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            refill_rate: refill_rate as f64,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time, then attempt to consume one.
    fn try_consume(&mut self) -> RateLimitDecision {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            RateLimitDecision::Allowed {
                remaining: self.tokens as u32,
            }
        } else {
            let deficit = 1.0 - self.tokens;
            let wait_secs = deficit / self.refill_rate;
            let retry_after_ms = (wait_secs * 1000.0).ceil() as u64;
            RateLimitDecision::Denied { retry_after_ms }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TokenBucketRateLimiter
// ─────────────────────────────────────────────────────────────────────────────

/// Lock-free token-bucket rate limiter backed by [`DashMap`].
///
/// Each unique key gets its own bucket lazily created on first access.
/// Implements [`GatewayRateLimiter`] from `mofa-kernel`.
pub struct TokenBucketRateLimiter {
    buckets: DashMap<String, TokenBucket>,
    capacity: u32,
    refill_rate: u32,
}

impl TokenBucketRateLimiter {
    /// Create a new limiter with the given configuration.
    pub fn new(config: &RateLimiterConfig) -> Self {
        Self {
            buckets: DashMap::new(),
            capacity: config.capacity,
            refill_rate: config.refill_rate,
        }
    }
}

impl GatewayRateLimiter for TokenBucketRateLimiter {
    fn check_and_consume(&self, key: &str) -> RateLimitDecision {
        self.buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(self.capacity, self.refill_rate))
            .try_consume()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use super::*;

    fn limiter(capacity: u32, refill_rate: u32) -> TokenBucketRateLimiter {
        TokenBucketRateLimiter::new(&RateLimiterConfig {
            capacity,
            refill_rate,
            strategy: KeyStrategy::PerClient,
        })
    }

    // ── Basic behaviour ──────────────────────────────────────────────────────

    #[test]
    fn fresh_bucket_allows_up_to_capacity() {
        let rl = limiter(5, 1);
        for i in 0..5 {
            let decision = rl.check_and_consume("client-a");
            assert!(
                decision.is_allowed(),
                "expected Allowed on request {}, got {decision:?}",
                i + 1
            );
        }
    }

    #[test]
    fn excess_requests_are_denied() {
        let rl = limiter(3, 1);
        for _ in 0..3 {
            rl.check_and_consume("client-a");
        }
        let decision = rl.check_and_consume("client-a");
        assert!(
            matches!(decision, RateLimitDecision::Denied { .. }),
            "expected Denied, got {decision:?}"
        );
    }

    #[test]
    fn denied_carries_positive_retry_after() {
        let rl = limiter(1, 1);
        rl.check_and_consume("client-a"); // drain
        match rl.check_and_consume("client-a") {
            RateLimitDecision::Denied { retry_after_ms } => {
                assert!(retry_after_ms > 0, "retry_after_ms must be > 0");
            }
            other => panic!("expected Denied, got {other:?}"),
        }
    }

    #[test]
    fn remaining_decrements_correctly() {
        let rl = limiter(3, 1);
        match rl.check_and_consume("client-a") {
            RateLimitDecision::Allowed { remaining } => assert_eq!(remaining, 2),
            other => panic!("expected Allowed, got {other:?}"),
        }
        match rl.check_and_consume("client-a") {
            RateLimitDecision::Allowed { remaining } => assert_eq!(remaining, 1),
            other => panic!("expected Allowed, got {other:?}"),
        }
    }

    #[test]
    fn different_keys_have_independent_buckets() {
        let rl = limiter(1, 1);
        let a = rl.check_and_consume("agent-a");
        let b = rl.check_and_consume("agent-b");
        assert!(a.is_allowed());
        assert!(b.is_allowed());
    }

    // ── Refill ───────────────────────────────────────────────────────────────

    #[test]
    fn bucket_refills_after_elapsed_time() {
        let rl = limiter(1, 1000); // 1000 tokens/sec — refills very fast
        rl.check_and_consume("client-a"); // drain

        thread::sleep(Duration::from_millis(5));

        let decision = rl.check_and_consume("client-a");
        assert!(
            decision.is_allowed(),
            "expected bucket to have refilled, got {decision:?}"
        );
    }

    // ── Concurrency ───────────────────────────────────────────────────────────

    #[test]
    fn concurrent_access_does_not_exceed_capacity() {
        const CAPACITY: u32 = 50;
        const THREADS: usize = 20;
        const REQUESTS_PER_THREAD: usize = 10;

        let rl = Arc::new(TokenBucketRateLimiter::new(&RateLimiterConfig {
            capacity: CAPACITY,
            refill_rate: 0, // no refill during the test
            strategy: KeyStrategy::PerClient,
        }));

        let allowed = Arc::new(std::sync::atomic::AtomicU32::new(0));

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let rl = Arc::clone(&rl);
                let allowed = Arc::clone(&allowed);
                thread::spawn(move || {
                    for _ in 0..REQUESTS_PER_THREAD {
                        if rl.check_and_consume("shared-key").is_allowed() {
                            allowed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let total_allowed = allowed.load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            total_allowed <= CAPACITY,
            "allowed {total_allowed} requests but capacity is {CAPACITY}"
        );
    }

    // ── Config ────────────────────────────────────────────────────────────────

    #[test]
    fn rate_limiter_config_default() {
        let cfg = RateLimiterConfig::default();
        assert_eq!(cfg.capacity, 100);
        assert_eq!(cfg.refill_rate, 10);
        assert_eq!(cfg.strategy, KeyStrategy::PerClient);
    }

    #[test]
    fn rate_limit_decision_is_allowed_helper() {
        assert!(RateLimitDecision::Allowed { remaining: 5 }.is_allowed());
        assert!(
            !RateLimitDecision::Denied {
                retry_after_ms: 100
            }
            .is_allowed()
        );
    }
}
