//! In-memory token-bucket rate-limit filter.
//!
//! Each unique (IP address or auth principal) is tracked independently.
//! The bucket is refilled in a continuous fashion: on each request the
//! elapsed wall-clock time is converted to tokens and added to the bucket,
//! then one token is consumed.  When no tokens remain the request is
//! rejected with `429 Too Many Requests`.

use async_trait::async_trait;
use mofa_kernel::gateway::{
    FilterAction, FilterOrder, GatewayContext, GatewayError, GatewayFilter, GatewayResponse,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::warn;

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl Bucket {
    fn new(capacity: f64) -> Self {
        Self {
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    /// Attempt to consume one token, refilling based on elapsed time first.
    /// Returns `true` if the token was consumed (request allowed).
    fn try_consume(&mut self, rate_per_second: f64, burst_capacity: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let refill = elapsed.as_secs_f64() * rate_per_second;
        self.tokens = (self.tokens + refill).min(burst_capacity);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Rate-limit filter using a per-caller token-bucket algorithm.
pub struct RateLimitFilter {
    rate_per_second: f64,
    burst_capacity: f64,
    /// Retry-after duration sent in the `Retry-After` header on rejection.
    retry_after: Duration,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl RateLimitFilter {
    /// Create a new rate-limit filter.
    ///
    /// - `rate_per_second`: sustained request rate (tokens refilled per second).
    /// - `burst_capacity`: maximum number of tokens in the bucket.
    pub fn new(rate_per_second: u32, burst_capacity: u32) -> Self {
        Self {
            rate_per_second: rate_per_second as f64,
            burst_capacity: burst_capacity as f64,
            retry_after: Duration::from_secs(1),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn caller_id(ctx: &GatewayContext) -> String {
        // Prefer the authenticated principal; fall back to forwarded IP or a
        // placeholder for unauthenticated callers.
        ctx.auth_principal.clone().unwrap_or_else(|| {
            ctx.request
                .headers
                .get("x-forwarded-for")
                .or_else(|| ctx.request.headers.get("x-real-ip"))
                .cloned()
                .unwrap_or_else(|| "anonymous".to_string())
        })
    }
}

#[async_trait]
impl GatewayFilter for RateLimitFilter {
    fn name(&self) -> &str {
        "rate-limit"
    }

    fn order(&self) -> FilterOrder {
        FilterOrder::RATE_LIMIT
    }

    async fn on_request(&self, ctx: &mut GatewayContext) -> Result<FilterAction, GatewayError> {
        let caller = Self::caller_id(ctx);
        let allowed = {
            let mut buckets = self.buckets.lock().await;
            let bucket = buckets
                .entry(caller.clone())
                .or_insert_with(|| Bucket::new(self.burst_capacity));
            bucket.try_consume(self.rate_per_second, self.burst_capacity)
        };

        if allowed {
            Ok(FilterAction::Continue)
        } else {
            warn!(
                request_id = %ctx.request.id,
                caller = %caller,
                "rate limit exceeded"
            );
            Ok(FilterAction::Reject(
                429,
                format!(
                    "Rate limit exceeded. Retry after {} second(s).",
                    self.retry_after.as_secs()
                ),
            ))
        }
    }

    async fn on_response(
        &self,
        _ctx: &GatewayContext,
        resp: &mut GatewayResponse,
    ) -> Result<(), GatewayError> {
        // Annotate the response with the rate-limit policy for transparency.
        resp.headers.insert(
            "x-ratelimit-limit".to_string(),
            (self.burst_capacity as u32).to_string(),
        );
        Ok(())
    }
}
