//! Advanced rate limiting configuration example.
//!
//! This example demonstrates how to configure different rate limiting
//! strategies and tune them for your use case.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example advanced_rate_limiting --package mofa-gateway
//! ```

use mofa_gateway::gateway::RateLimiter;
use mofa_gateway::RateLimitStrategy;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("Advanced Rate Limiting Configuration Example");

    // Example 1: Token Bucket - High Burst Capacity
    tracing::info!("\n=== Example 1: Token Bucket (High Burst) ===");
    let rate_limiter_burst = RateLimiter::new(RateLimitStrategy::TokenBucket {
        capacity: 100,      // Allow bursts of up to 100 requests
        refill_rate: 10,    // Refill at 10 tokens/second
    });
    
    tracing::info!("Configuration: capacity=100, refill_rate=10/sec");
    tracing::info!("Best for: APIs that need to handle traffic spikes");
    tracing::info!("Simulating burst traffic:");
    
    let mut allowed = 0;
    let mut denied = 0;
    for i in 0..120 {
        if rate_limiter_burst.try_acquire().await? {
            allowed += 1;
            if i < 10 {
                tracing::info!("Request {}: ✅ Allowed", i + 1);
            }
        } else {
            denied += 1;
            if denied <= 5 {
                tracing::info!("Request {}: ❌ Rate limited", i + 1);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tracing::info!("Results: {} allowed, {} denied", allowed, denied);

    // Example 2: Token Bucket - Steady Rate
    tracing::info!("\n=== Example 2: Token Bucket (Steady Rate) ===");
    let rate_limiter_steady = RateLimiter::new(RateLimitStrategy::TokenBucket {
        capacity: 20,       // Smaller burst capacity
        refill_rate: 5,     // Steady 5 requests/second
    });
    
    tracing::info!("Configuration: capacity=20, refill_rate=5/sec");
    tracing::info!("Best for: Consistent rate limiting without bursts");
    
    allowed = 0;
    denied = 0;
    for i in 0..30 {
        if rate_limiter_steady.try_acquire().await? {
            allowed += 1;
            if i < 5 {
                tracing::info!("Request {}: ✅ Allowed", i + 1);
            }
        } else {
            denied += 1;
            if denied <= 5 {
                tracing::info!("Request {}: ❌ Rate limited", i + 1);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tracing::info!("Results: {} allowed, {} denied", allowed, denied);

    // Example 3: Sliding Window - Strict Limits
    tracing::info!("\n=== Example 3: Sliding Window (Strict) ===");
    let rate_limiter_window = RateLimiter::new(RateLimitStrategy::SlidingWindow {
        window_size: Duration::from_secs(10),  // 10-second window
        max_requests: 50,                       // Max 50 requests per window
    });
    
    tracing::info!("Configuration: window=10s, max_requests=50");
    tracing::info!("Best for: Strict per-window limits, API quotas");
    tracing::info!("Simulating requests:");
    
    allowed = 0;
    denied = 0;
    for i in 0..60 {
        if rate_limiter_window.try_acquire().await? {
            allowed += 1;
            if i < 5 {
                tracing::info!("Request {}: ✅ Allowed", i + 1);
            }
        } else {
            denied += 1;
            if denied <= 5 {
                tracing::info!("Request {}: ❌ Rate limited", i + 1);
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    tracing::info!("Results: {} allowed, {} denied", allowed, denied);

    // Example 4: Sliding Window - Per-Minute Limit
    tracing::info!("\n=== Example 4: Sliding Window (Per-Minute) ===");
    let rate_limiter_minute = RateLimiter::new(RateLimitStrategy::SlidingWindow {
        window_size: Duration::from_secs(60),  // 1-minute window
        max_requests: 1000,                     // 1000 requests per minute
    });
    
    tracing::info!("Configuration: window=60s, max_requests=1000");
    tracing::info!("Best for: Per-minute API quotas, billing limits");

    tracing::info!("\n=== Rate Limiting Best Practices ===");
    tracing::info!("1. Token Bucket: Use for bursty traffic with smoothing");
    tracing::info!("2. Sliding Window: Use for strict per-period limits");
    tracing::info!("3. Start conservative and adjust based on metrics");
    tracing::info!("4. Monitor rejection rates and adjust accordingly");
    tracing::info!("5. Consider different limits for different endpoints");

    tracing::info!("\nExample completed successfully!");
    Ok(())
}
