//! Rate limiting implementation.
//!
//! This module provides rate limiting algorithms to prevent request floods
//! and ensure fair resource usage.
//!
//! # Implementation Status
//!
//! **Complete** - Rate limiting with Token Bucket and Sliding Window strategies implemented

use crate::error::{GatewayError, GatewayResult};
use crate::types::RateLimitStrategy;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limiter using token bucket algorithm.
pub struct TokenBucketRateLimiter {
    capacity: u64,
    tokens: Arc<RwLock<u64>>,
    refill_rate: u64, // tokens per second
    last_refill: Arc<RwLock<Instant>>,
}

impl TokenBucketRateLimiter {
    /// Create a new token bucket rate limiter.
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            tokens: Arc::new(RwLock::new(capacity)),
            refill_rate,
            last_refill: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Try to consume a token.
    pub async fn try_acquire(&self) -> GatewayResult<bool> {
        let mut tokens = self.tokens.write().await;
        let mut last_refill = self.last_refill.write().await;

        // Refill tokens based on elapsed time using sub-second precision.
        // The previous `elapsed.as_secs()` truncated fractional seconds, which
        // meant that high-frequency callers (< 1s apart) would never see a
        // refill even when `refill_rate` was large.
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);
        let elapsed_secs = elapsed.as_secs_f64();
        let tokens_to_add = (elapsed_secs * self.refill_rate as f64) as u64;

        if tokens_to_add > 0 {
            *tokens = (*tokens + tokens_to_add).min(self.capacity);
            // Advance last_refill by only the time accounted for, preserving
            // the fractional remainder for the next refill cycle.
            let secs_consumed = tokens_to_add as f64 / self.refill_rate as f64;
            *last_refill += Duration::from_secs_f64(secs_consumed);
        }

        // Try to consume a token
        if *tokens > 0 {
            *tokens -= 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Rate limiter using sliding window algorithm.
pub struct SlidingWindowRateLimiter {
    window_size: Duration,
    max_requests: u64,
    requests: Arc<RwLock<Vec<Instant>>>,
}

impl SlidingWindowRateLimiter {
    /// Create a new sliding window rate limiter.
    pub fn new(window_size: Duration, max_requests: u64) -> Self {
        Self {
            window_size,
            max_requests,
            requests: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Try to allow a request.
    pub async fn try_acquire(&self) -> GatewayResult<bool> {
        let mut requests = self.requests.write().await;
        let now = Instant::now();

        // Remove old requests outside the window
        requests.retain(|&time| now.duration_since(time) < self.window_size);

        // Check if we're under the limit
        if requests.len() < self.max_requests as usize {
            requests.push(now);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Rate limiter that supports multiple strategies.
pub struct RateLimiter {
    strategy: RateLimitStrategy,
    token_bucket: Option<TokenBucketRateLimiter>,
    sliding_window: Option<SlidingWindowRateLimiter>,
    // Per-key rate limiters (for per-user/IP limiting)
    per_key_limiters: Arc<RwLock<HashMap<String, Arc<dyn RateLimiterTrait + Send + Sync>>>>,
}

#[async_trait::async_trait]
trait RateLimiterTrait {
    async fn try_acquire(&self) -> GatewayResult<bool>;
}

#[async_trait::async_trait]
impl RateLimiterTrait for TokenBucketRateLimiter {
    async fn try_acquire(&self) -> GatewayResult<bool> {
        TokenBucketRateLimiter::try_acquire(self).await
    }
}

#[async_trait::async_trait]
impl RateLimiterTrait for SlidingWindowRateLimiter {
    async fn try_acquire(&self) -> GatewayResult<bool> {
        SlidingWindowRateLimiter::try_acquire(self).await
    }
}

impl RateLimiter {
    /// Create a new rate limiter with the given strategy.
    pub fn new(strategy: RateLimitStrategy) -> Self {
        let (token_bucket, sliding_window) = match strategy {
            RateLimitStrategy::TokenBucket {
                capacity,
                refill_rate,
            } => (
                Some(TokenBucketRateLimiter::new(capacity, refill_rate)),
                None,
            ),
            RateLimitStrategy::SlidingWindow {
                window_size,
                max_requests,
            } => (
                None,
                Some(SlidingWindowRateLimiter::new(window_size, max_requests)),
            ),
        };

        Self {
            strategy,
            token_bucket,
            sliding_window,
            per_key_limiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Try to allow a request (global rate limit).
    pub async fn try_acquire(&self) -> GatewayResult<bool> {
        if let Some(ref tb) = self.token_bucket {
            tb.try_acquire().await
        } else if let Some(ref sw) = self.sliding_window {
            sw.try_acquire().await
        } else {
            Ok(true) // No rate limiting
        }
    }

    /// Try to allow a request for a specific key (per-user/IP rate limiting).
    pub async fn try_acquire_key(&self, key: &str) -> GatewayResult<bool> {
        // Get or create a per-key rate limiter without holding the write lock
        // across `.await` on the limiter itself.
        let limiter = {
            let mut limiters = self.per_key_limiters.write().await;

            let entry = limiters.entry(key.to_string()).or_insert_with(|| {
                // Create a new rate limiter for this key based on the strategy
                match &self.strategy {
                    RateLimitStrategy::TokenBucket {
                        capacity,
                        refill_rate,
                    } => Arc::new(TokenBucketRateLimiter::new(*capacity, *refill_rate))
                        as Arc<dyn RateLimiterTrait + Send + Sync>,
                    RateLimitStrategy::SlidingWindow {
                        window_size,
                        max_requests,
                    } => Arc::new(SlidingWindowRateLimiter::new(*window_size, *max_requests))
                        as Arc<dyn RateLimiterTrait + Send + Sync>,
                }
            });

            Arc::clone(entry)
        };

        // Use the per-key limiter outside of the lock to avoid holding a write
        // guard across `.await`.
        limiter.try_acquire().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_bucket() {
        let limiter = TokenBucketRateLimiter::new(5, 1); // 5 capacity, 1 token/sec

        // Should allow 5 requests immediately
        for _ in 0..5 {
            assert!(limiter.try_acquire().await.expect("failed"));
        }

        // 6th request should be denied
        assert!(!limiter.try_acquire().await.expect("failed"));

        // Wait for refill
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(limiter.try_acquire().await.expect("failed"));
    }

    #[tokio::test]
    async fn test_token_bucket_sub_second_refill() {
        // With as_secs() truncation, a 10 token/sec bucket that is checked every
        // 500ms would NEVER refill because 0.5.as_secs() == 0.
        let limiter = TokenBucketRateLimiter::new(10, 10); // 10 capacity, 10 tokens/sec

        // Drain all tokens
        for _ in 0..10 {
            assert!(limiter.try_acquire().await.expect("failed"));
        }
        assert!(!limiter.try_acquire().await.expect("failed"));

        // Wait 500ms — should refill ~5 tokens with sub-second precision
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            limiter.try_acquire().await.expect("failed"),
            "sub-second refill should have added tokens"
        );
    }

    #[tokio::test]
    async fn test_sliding_window() {
        let limiter = SlidingWindowRateLimiter::new(Duration::from_secs(1), 3);

        // Should allow 3 requests
        for _ in 0..3 {
            assert!(limiter.try_acquire().await.expect("failed"));
        }

        // 4th request should be denied
        assert!(!limiter.try_acquire().await.expect("failed"));

        // Wait for window to slide
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(limiter.try_acquire().await.expect("failed"));
    }

    #[tokio::test]
    async fn test_per_key_rate_limiting() {
        let limiter = RateLimiter::new(RateLimitStrategy::TokenBucket {
            capacity: 5,
            refill_rate: 1,
        });

        // Different keys should have independent limits
        assert!(limiter.try_acquire_key("user-1").await.expect("failed"));
        assert!(limiter.try_acquire_key("user-2").await.expect("failed"));

        // Exhaust user-1's limit
        for _ in 0..4 {
            assert!(limiter.try_acquire_key("user-1").await.expect("failed"));
        }
        // user-1 should be rate limited now
        assert!(!limiter.try_acquire_key("user-1").await.expect("failed"));

        // But user-2 should still be able to make requests
        assert!(limiter.try_acquire_key("user-2").await.expect("failed"));
    }
}
