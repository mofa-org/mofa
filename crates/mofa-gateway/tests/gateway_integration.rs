//! Gateway integration tests.
//!
//! These tests verify that the gateway correctly routes requests, handles
//! load balancing, and integrates with the control plane.

use mofa_gateway::gateway::{Gateway, GatewayConfig};
use mofa_gateway::types::LoadBalancingAlgorithm;
use std::time::Duration;

#[tokio::test]
async fn test_gateway_startup() {
    let config = GatewayConfig {
        listen_addr: "127.0.0.1:0".parse().unwrap(), // Use port 0 for random port
        load_balancing: LoadBalancingAlgorithm::RoundRobin,
        enable_rate_limiting: true,
        enable_circuit_breakers: true,
    };

    let mut gateway = Gateway::new(config).await.unwrap();
    gateway.start().await.unwrap();

    // Gateway should start successfully
    // Note: We can't easily test the HTTP server without making actual requests,
    // but we can verify it starts without errors

    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn test_gateway_metrics_endpoint() {
    let config = GatewayConfig {
        listen_addr: "127.0.0.1:0".parse().unwrap(),
        load_balancing: LoadBalancingAlgorithm::RoundRobin,
        enable_rate_limiting: true,
        enable_circuit_breakers: true,
    };

    let mut gateway = Gateway::new(config).await.unwrap();
    gateway.start().await.unwrap();

    // Get metrics
    let metrics = gateway.metrics();
    let metrics_text = metrics.export().unwrap();

    // Verify metrics format
    assert!(
        metrics_text.contains("gateway_requests_total"),
        "Should contain request metrics"
    );
    assert!(
        metrics_text.contains("gateway_nodes_total"),
        "Should contain node metrics"
    );

    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn test_gateway_with_control_plane() {
    // This test would require setting up a control plane
    // For now, we just verify the API works
    let config = GatewayConfig {
        listen_addr: "127.0.0.1:0".parse().unwrap(),
        load_balancing: LoadBalancingAlgorithm::RoundRobin,
        enable_rate_limiting: true,
        enable_circuit_breakers: true,
    };

    let mut gateway = Gateway::with_control_plane(config, None).await.unwrap();
    gateway.start().await.unwrap();

    // Gateway should work with or without control plane
    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn test_load_balancer_algorithms() {
    use mofa_gateway::gateway::LoadBalancer;
    use mofa_gateway::types::{LoadBalancingAlgorithm, NodeId};

    // Test RoundRobin
    let lb_rr = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin);
    let node1 = NodeId::new("node-1");
    let node2 = NodeId::new("node-2");
    lb_rr.add_node(node1.clone()).await;
    lb_rr.add_node(node2.clone()).await;

    let selected1 = lb_rr.select_node().await.unwrap().unwrap();
    let selected2 = lb_rr.select_node().await.unwrap().unwrap();
    let selected3 = lb_rr.select_node().await.unwrap().unwrap();

    // Round-robin should cycle through nodes
    assert_eq!(selected1, node1);
    assert_eq!(selected2, node2);
    assert_eq!(selected3, node1); // Wraps around

    // Test LeastConnections
    let lb_lc = LoadBalancer::new(LoadBalancingAlgorithm::LeastConnections);
    lb_lc.add_node(node1.clone()).await;
    lb_lc.add_node(node2.clone()).await;

    // Initially should select first node
    let selected = lb_lc.select_node().await.unwrap().unwrap();
    assert_eq!(selected, node1);

    // Increment connections for node1
    lb_lc.increment_connections(&node1).await;
    lb_lc.increment_connections(&node1).await;

    // Now should select node2 (fewer connections)
    let selected = lb_lc.select_node().await.unwrap().unwrap();
    assert_eq!(selected, node2);
}

#[tokio::test]
async fn test_circuit_breaker_integration() {
    use mofa_gateway::gateway::{CircuitBreaker, CircuitBreakerRegistry};
    use mofa_gateway::types::NodeId;

    // Use shorter timeout for test (50ms)
    let registry = CircuitBreakerRegistry::new(3, 2, Duration::from_millis(50));
    let node_id = NodeId::new("node-1");

    let breaker = registry.get_or_create(&node_id).await;

    // Initially closed
    assert!(breaker.try_acquire().await.unwrap());

    // Record failures
    for _ in 0..3 {
        breaker.record_failure().await;
    }

    // Should be open now
    assert!(!breaker.try_acquire().await.unwrap());

    // Wait for timeout (longer than 50ms to ensure transition)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should transition to half-open
    assert!(breaker.try_acquire().await.unwrap());

    // Success should close it
    breaker.record_success().await;
    assert!(breaker.try_acquire().await.unwrap());
}
