//! Advanced health check configuration example.
//!
//! This example demonstrates how to configure health checking for
//! cluster nodes and customize check intervals and thresholds.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example advanced_health_checks --package mofa-gateway
//! ```

use mofa_gateway::gateway::HealthChecker;
use mofa_gateway::NodeId;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("Advanced Health Check Configuration Example");

    // Example 1: Frequent Health Checks (High Availability)
    tracing::info!("\n=== Example 1: Frequent Health Checks ===");
    let health_checker_frequent = HealthChecker::new(
        Duration::from_secs(5),    // Check interval: 5 seconds
        Duration::from_secs(1),    // Timeout: 1 second
        2,                         // Failure threshold: 2 consecutive failures
    );
    
    tracing::info!("Configuration: interval=5s, timeout=1s, threshold=2");
    tracing::info!("Best for: Critical services requiring fast failure detection");
    tracing::info!("Detects failures within ~10 seconds (2 checks × 5s)");
    
    health_checker_frequent.start().await?;

    // Example 2: Standard Health Checks (Balanced)
    tracing::info!("\n=== Example 2: Standard Health Checks ===");
    let health_checker_standard = HealthChecker::new(
        Duration::from_secs(10),   // Check interval: 10 seconds
        Duration::from_secs(2),   // Timeout: 2 seconds
        3,                        // Failure threshold: 3 consecutive failures
    );
    
    tracing::info!("Configuration: interval=10s, timeout=2s, threshold=3");
    tracing::info!("Best for: Most production workloads");
    tracing::info!("Detects failures within ~30 seconds (3 checks × 10s)");
    
    health_checker_standard.start().await?;

    // Example 3: Conservative Health Checks (Low Overhead)
    tracing::info!("\n=== Example 3: Conservative Health Checks ===");
    let health_checker_conservative = HealthChecker::new(
        Duration::from_secs(30),   // Check interval: 30 seconds
        Duration::from_secs(5),    // Timeout: 5 seconds
        3,                        // Failure threshold: 3 consecutive failures
    );
    
    tracing::info!("Configuration: interval=30s, timeout=5s, threshold=3");
    tracing::info!("Best for: Services with high health check overhead");
    tracing::info!("Detects failures within ~90 seconds (3 checks × 30s)");
    
    health_checker_conservative.start().await?;

    // Demonstrate health checking
    tracing::info!("\n=== Health Check Behavior ===");
    
    // Register some nodes
    let node1 = NodeId::new("node-1");
    let node2 = NodeId::new("node-2");
    let node3 = NodeId::new("node-3");
    
    health_checker_frequent.register_node(node1.clone()).await;
    health_checker_frequent.register_node(node2.clone()).await;
    health_checker_frequent.register_node(node3.clone()).await;
    
    tracing::info!("Registered nodes: node-1, node-2, node-3");
    
    // Check node status
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    tracing::info!("\nChecking node statuses:");
    if let Some(status) = health_checker_frequent.get_status(&node1).await {
        tracing::info!("Node 1 status: {:?}", status);
    }
    if let Some(status) = health_checker_frequent.get_status(&node2).await {
        tracing::info!("Node 2 status: {:?}", status);
    }
    if let Some(status) = health_checker_frequent.get_status(&node3).await {
        tracing::info!("Node 3 status: {:?}", status);
    }

    tracing::info!("\n=== Health Check Best Practices ===");
    tracing::info!("1. Set interval based on acceptable detection time");
    tracing::info!("2. Use shorter intervals for critical services");
    tracing::info!("3. Set timeout to prevent hanging checks");
    tracing::info!("4. Use failure threshold to avoid false positives");
    tracing::info!("5. Monitor health check metrics (success rate, latency)");
    tracing::info!("6. Alert on frequent health check failures");
    tracing::info!("7. Consider different intervals for different node types");

    tracing::info!("\nExample completed successfully!");
    Ok(())
}
