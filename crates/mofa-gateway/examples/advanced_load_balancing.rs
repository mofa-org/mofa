//! Advanced load balancing configuration example.
//! 
//! This example demonstrates how to configure different load balancing
//! algorithms and customize their behavior.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example advanced_load_balancing --package mofa-gateway
//! ```

use mofa_gateway::gateway::LoadBalancer;
use mofa_gateway::{LoadBalancingAlgorithm, NodeId};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("Advanced Load Balancing Configuration Example");

    // Example 1: Round-Robin (default)
    tracing::info!("\n=== Example 1: Round-Robin ===");
    let load_balancer_rr = Arc::new(LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin));
    tracing::info!("Round-robin distributes requests evenly across all nodes");
    tracing::info!("Best for: Uniform node capacity, stateless requests");

    // Example 2: Least Connections
    tracing::info!("\n=== Example 2: Least Connections ===");
    let load_balancer_lc = Arc::new(LoadBalancer::new(LoadBalancingAlgorithm::LeastConnections));
    tracing::info!("Least-connections routes to node with fewest active connections");
    tracing::info!("Best for: Long-lived connections, stateful workloads");

    // Example 3: Weighted Round-Robin
    tracing::info!("\n=== Example 3: Weighted Round-Robin ===");
    let load_balancer_wrr = Arc::new(LoadBalancer::new(LoadBalancingAlgorithm::WeightedRoundRobin));
    tracing::info!("Weighted round-robin distributes based on node capacity");
    tracing::info!("Best for: Heterogeneous node capacities");

    // Example 4: Random
    tracing::info!("\n=== Example 4: Random ===");
    let load_balancer_rand = Arc::new(LoadBalancer::new(LoadBalancingAlgorithm::Random));
    tracing::info!("Random selection for uniform distribution");
    tracing::info!("Best for: Simple scenarios, testing");

    // Demonstrate load balancing with multiple nodes
    tracing::info!("\n=== Load Balancing Simulation ===");
    
    // Add nodes to the round-robin load balancer
    load_balancer_rr.add_node(NodeId::new("node-1")).await;
    load_balancer_rr.add_node(NodeId::new("node-2")).await;
    load_balancer_rr.add_node(NodeId::new("node-3")).await;
    
    tracing::info!("Routing 10 requests with Round-Robin:");
    for i in 0..10 {
        if let Some(node) = load_balancer_rr.select_node().await? {
            tracing::info!("Request {} -> {}", i + 1, node);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    tracing::info!("\nExample completed successfully!");
    Ok(())
}
