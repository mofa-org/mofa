//! Simple integration tests that work with the current implementation.
//!
//! These tests verify basic functionality without requiring complex cluster setup.

use mofa_gateway::control_plane::state_machine::ReplicatedStateMachine;
use mofa_gateway::types::{NodeAddress, NodeId, StateMachineCommand};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[tokio::test]
async fn test_state_machine_agent_registration() {
    use std::sync::Arc;
    use tokio::sync::RwLock;
    
    let sm = Arc::new(RwLock::new(ReplicatedStateMachine::new()));

    // Register an agent
    let mut metadata = HashMap::new();
    metadata.insert("type".to_string(), "test-agent".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());

    let command = StateMachineCommand::RegisterAgent {
        agent_id: "agent-1".to_string(),
        metadata: metadata.clone(),
    };

    sm.write().await.apply(command).await.unwrap();

    // Verify agent is registered
    let agents = sm.read().await.get_agents().await;
    assert!(agents.contains_key("agent-1"));
    assert_eq!(agents["agent-1"].agent_type, "test-agent");
}

#[tokio::test]
async fn test_state_machine_node_management() {
    let sm = ReplicatedStateMachine::new();

    let node_id = NodeId::new("node-1");
    let address = NodeAddress {
        control_plane: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        gateway: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9090),
    };

    // Add node
    let add_cmd = StateMachineCommand::AddNode {
        node_id: node_id.clone(),
        address: address.clone(),
    };
    sm.apply(add_cmd).await.unwrap();

    // Verify node is in membership
    {
        let membership = sm.membership();
        let membership_guard = membership.read().await;
        assert!(membership_guard.has_node(&node_id));
    } // Release the lock before applying next command

    // Remove node
    let remove_cmd = StateMachineCommand::RemoveNode {
        node_id: node_id.clone(),
    };
    sm.apply(remove_cmd).await.unwrap();

    // Verify node is removed
    {
        let membership = sm.membership();
        let membership_guard = membership.read().await;
        assert!(!membership_guard.has_node(&node_id));
    }
}

#[tokio::test]
async fn test_state_machine_config_updates() {
    let sm = ReplicatedStateMachine::new();

    // Update config
    let config_cmd = StateMachineCommand::UpdateConfig {
        key: "test_key".to_string(),
        value: "test_value".to_string(),
    };
    sm.apply(config_cmd).await.unwrap();

    // Verify config is updated
    let config = sm.get_config().await;
    assert_eq!(config.get("test_key"), Some(&"test_value".to_string()));
}

#[tokio::test]
async fn test_gateway_metrics_collection() {
    use mofa_gateway::observability::GatewayMetrics;
    use std::time::Duration;

    let metrics = GatewayMetrics::new();

    // Test request metrics
    metrics.increment_requests();
    assert_eq!(metrics.requests_total.get() as u64, 1);

    metrics.record_request_duration(Duration::from_millis(100));
    // Duration is recorded in histogram, verify it doesn't panic

    metrics.increment_errors();
    assert_eq!(metrics.requests_errors_total.get() as u64, 1);

    // Test node metrics
    metrics.update_node_counts(5, 4, 1);
    assert_eq!(metrics.nodes_total.get() as u64, 5);
    assert_eq!(metrics.nodes_healthy.get() as u64, 4);
    assert_eq!(metrics.nodes_unhealthy.get() as u64, 1);

    // Test consensus metrics
    metrics.update_consensus_term(5);
    assert_eq!(metrics.consensus_term.get() as u64, 5);

    metrics.increment_leader_elections();
    assert_eq!(metrics.consensus_leader_elections_total.get() as u64, 1);

    // Test agent metrics
    metrics.update_agent_count(10);
    assert_eq!(metrics.agents_registered.get() as u64, 10);

    // Test export
    let exported = metrics.export().unwrap();
    assert!(exported.contains("gateway_requests_total"));
    assert!(exported.contains("gateway_nodes_total"));
    assert!(exported.contains("gateway_consensus_term"));
}

#[tokio::test]
async fn test_load_balancer_integration() {
    use mofa_gateway::gateway::LoadBalancer;
    use mofa_gateway::types::LoadBalancingAlgorithm;

    let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin);

    // Add nodes
    let node1 = NodeId::new("node-1");
    let node2 = NodeId::new("node-2");
    let node3 = NodeId::new("node-3");

    lb.add_node(node1.clone()).await;
    lb.add_node(node2.clone()).await;
    lb.add_node(node3.clone()).await;

    // Test round-robin selection
    let selected1 = lb.select_node().await.unwrap().unwrap();
    let selected2 = lb.select_node().await.unwrap().unwrap();
    let selected3 = lb.select_node().await.unwrap().unwrap();
    let selected4 = lb.select_node().await.unwrap().unwrap();

    assert_eq!(selected1, node1);
    assert_eq!(selected2, node2);
    assert_eq!(selected3, node3);
    assert_eq!(selected4, node1); // Wraps around

    // Test connection counting
    lb.increment_connections(&node1).await;
    lb.increment_connections(&node1).await;
    lb.increment_connections(&node2).await;

    // With least connections, should select node3 (0 connections) or node2 (1 connection)
    let lb_lc = LoadBalancer::new(LoadBalancingAlgorithm::LeastConnections);
    lb_lc.add_node(node1.clone()).await;
    lb_lc.add_node(node2.clone()).await;
    lb_lc.add_node(node3.clone()).await;

    lb_lc.increment_connections(&node1).await;
    lb_lc.increment_connections(&node1).await;
    lb_lc.increment_connections(&node2).await;

    let selected = lb_lc.select_node().await.unwrap().unwrap();
    assert_eq!(selected, node3); // Should select node with least connections
}

#[tokio::test]
async fn test_health_checker_integration() {
    use mofa_gateway::gateway::HealthChecker;
    use mofa_gateway::types::NodeStatus;
    use std::time::Duration;

    let checker = HealthChecker::new(
        Duration::from_secs(5),
        Duration::from_secs(1),
        3,
    );

    let node_id = NodeId::new("node-1");
    checker.register_node(node_id.clone()).await;

    // Check node
    let is_healthy = checker.check_node(&node_id).await.unwrap();
    assert!(is_healthy);

    // Verify status
    let status = checker.get_status(&node_id).await;
    assert_eq!(status, Some(NodeStatus::Healthy));

    // Unregister node
    checker.unregister_node(&node_id).await;
    let status_after = checker.get_status(&node_id).await;
    assert_eq!(status_after, None);
}

#[tokio::test]
async fn test_circuit_breaker_integration() {
    use mofa_gateway::gateway::{CircuitBreaker, CircuitBreakerRegistry};
    use mofa_gateway::types::NodeId;
    use std::time::Duration;

    let registry = CircuitBreakerRegistry::new(3, 2, Duration::from_secs(5));
    let node_id = NodeId::new("node-1");

    let breaker = registry.get_or_create(&node_id).await;

    // Initially closed
    assert!(breaker.try_acquire().await.unwrap());

    // Record failures until circuit opens
    for _ in 0..3 {
        breaker.record_failure().await;
    }

    // Circuit should be open
    assert!(!breaker.try_acquire().await.unwrap());

    // Circuit is open - verify we can't acquire
    assert!(!breaker.try_acquire().await.unwrap());

    // Record success to reset failure count (but circuit may still be open due to timeout)
    breaker.record_success().await;
    
    // Note: In a real scenario, we'd wait for the timeout period before the circuit
    // transitions to half-open. For this test, we just verify the basic open/close behavior.
}
