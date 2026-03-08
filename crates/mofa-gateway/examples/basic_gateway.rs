//! Basic gateway example.
//!
//! This example demonstrates how to set up and use the MoFA gateway
//! for request routing and load balancing.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example basic_gateway --package mofa-gateway
//! ```

use mofa_gateway::gateway::{LoadBalancer, RateLimiter};
use mofa_gateway::{LoadBalancingAlgorithm, NodeId, RateLimitStrategy};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("Starting MoFA Gateway example");

    // Create load balancer
    let load_balancer = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin);
    
    // Add some nodes
    load_balancer.add_node(NodeId::new("node-1")).await;
    load_balancer.add_node(NodeId::new("node-2")).await;
    load_balancer.add_node(NodeId::new("node-3")).await;

    // Test load balancing
    tracing::info!("Testing load balancing...");
    for i in 0..5 {
        if let Some(node) = load_balancer.select_node().await? {
            tracing::info!("Request {} routed to {}", i, node);
        }
    }

    // Create rate limiter
    let rate_limiter = RateLimiter::new(RateLimitStrategy::TokenBucket {
        capacity: 10,
        refill_rate: 2, // 2 tokens per second
    });

    // Test rate limiting
    tracing::info!("Testing rate limiting...");
    for i in 0..15 {
        let allowed = rate_limiter.try_acquire().await?;
        if allowed {
            tracing::info!("Request {} allowed", i);
        } else {
            tracing::info!("Request {} rate limited", i);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tracing::info!("Example completed successfully");
    Ok(())
}
