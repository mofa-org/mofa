//! Rate Limiter for Review Requests
//!
//! Token bucket algorithm for throttling review requests per tenant

use mofa_kernel::hitl::HitlError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Token bucket rate limiter
pub struct RateLimiter {
    /// Tokens per second
    tokens_per_sec: f64,
    /// Maximum bucket size
    max_tokens: f64,
    /// Per-tenant buckets
    buckets: Arc<Mutex<std::collections::HashMap<String, TokenBucket>>>,
}

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `tokens_per_sec` - Rate of token refill per second
    /// * `max_tokens` - Maximum bucket size
    pub fn new(tokens_per_sec: f64, max_tokens: f64) -> Self {
        Self {
            tokens_per_sec,
            max_tokens,
            buckets: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Check if a request is allowed (non-blocking)
    pub async fn check(&self, tenant_id: &str) -> Result<(), HitlError> {
        let mut buckets = self.buckets.lock().await;
        let bucket = buckets
            .entry(tenant_id.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.max_tokens,
                last_refill: Instant::now(),
            });

        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill);
        let tokens_to_add = elapsed.as_secs_f64() * self.tokens_per_sec;
        bucket.tokens = (bucket.tokens + tokens_to_add).min(self.max_tokens);
        bucket.last_refill = now;

        // Check if we have tokens
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            // Calculate retry after
            let tokens_needed = 1.0 - bucket.tokens;
            let secs_needed = (tokens_needed / self.tokens_per_sec).ceil() as u64;
            Err(HitlError::RateLimitExceeded {
                tenant_id: tenant_id.to_string(),
                retry_after_secs: secs_needed,
            })
        }
    }

    /// Clean up old buckets (optional maintenance)
    pub async fn cleanup_old_buckets(&self, max_age: Duration) {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();
        buckets.retain(|_, bucket| now.duration_since(bucket.last_refill) < max_age);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        // Default: 10 requests per second, max 100 tokens
        Self::new(10.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_rate_limiter_allows_requests() {
        let limiter = RateLimiter::new(10.0, 100.0);

        // First request should succeed
        let result = limiter.check("tenant-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_enforces_limit() {
        let limiter = RateLimiter::new(2.0, 2.0); // 2 req/sec, max 2 tokens

        // First 2 requests should succeed
        assert!(limiter.check("tenant-1").await.is_ok());
        assert!(limiter.check("tenant-1").await.is_ok());

        // Third request should be rate limited
        let result = limiter.check("tenant-1").await;
        assert!(result.is_err());
        if let Err(HitlError::RateLimitExceeded {
            retry_after_secs, ..
        }) = result
        {
            assert!(retry_after_secs > 0);
        } else {
            panic!("Expected RateLimitExceeded error");
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_refills_tokens() {
        let limiter = RateLimiter::new(10.0, 10.0); // 10 req/sec, max 10 tokens

        // Exhaust tokens
        for _ in 0..10 {
            assert!(limiter.check("tenant-1").await.is_ok());
        }

        // Should be rate limited
        assert!(limiter.check("tenant-1").await.is_err());

        // Wait for refill (1 second should refill 10 tokens)
        sleep(Duration::from_millis(1100)).await;

        // Should succeed again
        assert!(limiter.check("tenant-1").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_tenant_isolation() {
        let limiter = RateLimiter::new(2.0, 2.0);

        // Exhaust tenant-1's bucket
        assert!(limiter.check("tenant-1").await.is_ok());
        assert!(limiter.check("tenant-1").await.is_ok());
        assert!(limiter.check("tenant-1").await.is_err());

        // tenant-2 should still have tokens
        assert!(limiter.check("tenant-2").await.is_ok());
        assert!(limiter.check("tenant-2").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_max_tokens_cap() {
        let limiter = RateLimiter::new(100.0, 5.0); // High refill rate, low max

        // Should only allow max_tokens requests even if refill rate is high
        for _ in 0..5 {
            assert!(limiter.check("tenant-1").await.is_ok());
        }

        // Should be rate limited (max tokens reached)
        assert!(limiter.check("tenant-1").await.is_err());
    }
}
