//! Multi-node cluster integration tests.
//!
//! These tests verify that multiple control plane nodes can work together
//! in a distributed cluster, including leader election, state replication,
//! and failover scenarios.

use mofa_gateway::consensus::storage::RaftStorage;
use mofa_gateway::consensus::transport_impl::{ConsensusHandler, InMemoryTransport};
use mofa_gateway::control_plane::{ControlPlane, ControlPlaneConfig};
use mofa_gateway::types::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Handler wrapper for ConsensusEngine
struct EngineHandler {
    engine: Arc<mofa_gateway::consensus::engine::ConsensusEngine>,
}

#[async_trait::async_trait]
impl ConsensusHandler for EngineHandler {
    async fn handle_request_vote(
        &self,
        request: mofa_gateway::consensus::transport::RequestVoteRequest,
    ) -> mofa_gateway::error::ConsensusResult<mofa_gateway::consensus::transport::RequestVoteResponse> {
        self.engine.handle_request_vote(request).await
    }

    async fn handle_append_entries(
        &self,
        request: mofa_gateway::consensus::transport::AppendEntriesRequest,
    ) -> mofa_gateway::error::ConsensusResult<mofa_gateway::consensus::transport::AppendEntriesResponse> {
        self.engine.handle_append_entries(request).await
    }
}

/// Helper struct to manage a test cluster.
struct TestCluster {
    nodes: Vec<(NodeId, Arc<ControlPlane>)>,
    transport: Arc<InMemoryTransport>,
}

impl TestCluster {
    /// Create a new test cluster with N nodes.
    async fn new(num_nodes: usize) -> Self {
        let mut nodes = Vec::new();
        let transport = Arc::new(InMemoryTransport::new());
        
        // Create node IDs
        let node_ids: Vec<NodeId> = (0..num_nodes)
            .map(|i| NodeId::new(&format!("node-{}", i + 1)))
            .collect();
        
        // Create control plane instances
        for (idx, node_id) in node_ids.iter().enumerate() {
            let storage = Arc::new(RaftStorage::new());
            let config = ControlPlaneConfig {
                node_id: node_id.clone(),
                cluster_nodes: node_ids.clone(),
                storage_path: format!("/tmp/test_cluster/node-{}", idx + 1),
                election_timeout_ms: 150,
                heartbeat_interval_ms: 50,
            };
            
            let cp = ControlPlane::new(config, storage, Arc::clone(&transport) as _)
                .await
                .unwrap();
            
            // Register handler with transport (need to access consensus engine)
            // Note: We'll register after creating all nodes since we need the Arc
            
            nodes.push((node_id.clone(), Arc::new(cp)));
        }
        
        // Register handlers for all nodes
        for (node_id, cp) in &nodes {
            let engine = cp.consensus();
            let handler = Arc::new(EngineHandler {
                engine: Arc::clone(engine),
            });
            transport.register_handler(node_id.clone(), handler).await;
        }
        
        Self { nodes, transport }
    }
    
    /// Start all nodes in the cluster.
    async fn start_all(&self) {
        // Start nodes with small delays to prevent simultaneous candidate transitions
        for (idx, (node_id, cp)) in self.nodes.iter().enumerate() {
            cp.start().await.unwrap();
            tracing::debug!("Started node {}", node_id);
            // Small delay between starts to stagger election timeouts
            if idx < self.nodes.len() - 1 {
                sleep(Duration::from_millis(50)).await;
            }
        }
        // Give nodes time to initialize
        sleep(Duration::from_millis(200)).await;
    }
    
    /// Stop all nodes in the cluster.
    async fn stop_all(&self) {
        for (node_id, cp) in &self.nodes {
            let _ = cp.stop().await;
            tracing::debug!("Stopped node {}", node_id);
        }
    }
    
    /// Get a node by index.
    fn get_node(&self, idx: usize) -> Option<&(NodeId, Arc<ControlPlane>)> {
        self.nodes.get(idx)
    }
    
    /// Get the leader node (if any).
    async fn get_leader(&self) -> Option<(NodeId, Arc<ControlPlane>)> {
        for (node_id, cp) in &self.nodes {
            if cp.is_leader().await {
                return Some((node_id.clone(), Arc::clone(cp)));
            }
        }
        None
    }
}

#[tokio::test]
async fn test_three_node_cluster_startup() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    // Verify at least one node becomes leader
    // Wait longer for election (election timeout is 150-300ms, so 2 seconds should be enough)
    // Check multiple times to catch when leader is elected
    let mut leader = None;
    for _ in 0..20 {
        sleep(Duration::from_millis(200)).await;
        leader = cluster.get_leader().await;
        if leader.is_some() {
            break;
        }
    }
    assert!(leader.is_some(), "Expected a leader to be elected after 4 seconds");
    
    cluster.stop_all().await;
}

#[tokio::test]
async fn test_leader_election() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    // Wait for leader election and stability
    // Check multiple times to ensure we have a stable leader
    let mut leader_count = 0;
    for _ in 0..20 {
        sleep(Duration::from_millis(200)).await;
        leader_count = 0;
        for (node_id, cp) in &cluster.nodes {
            if cp.is_leader().await {
                leader_count += 1;
            }
        }
        if leader_count == 1 {
            // Found exactly one leader, verify it's stable
            sleep(Duration::from_millis(500)).await;
            leader_count = 0;
            for (node_id, cp) in &cluster.nodes {
                if cp.is_leader().await {
                    leader_count += 1;
                    tracing::info!("Node {} is leader", node_id);
                }
            }
            break;
        }
    }
    
    assert_eq!(leader_count, 1, "Expected exactly one leader, found {}", leader_count);
    
    cluster.stop_all().await;
}

#[tokio::test]
async fn test_state_replication_across_nodes() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    // Wait for leader election - check multiple times
    let mut leader = None;
    for _ in 0..20 {
        sleep(Duration::from_millis(200)).await;
        leader = cluster.get_leader().await;
        if leader.is_some() {
            break;
        }
    }
    let leader = leader.expect("No leader elected");
    let (leader_id, leader_cp) = leader;
    
    // Register an agent through the leader
    let mut metadata = HashMap::new();
    metadata.insert("type".to_string(), "test".to_string());
    
    leader_cp.register_agent("test-agent-1".to_string(), metadata).await.unwrap();
    
    // Wait for replication - need time for log replication, commit, and state machine application
    // Retry checking multiple times since apply loop runs every 50ms
    let mut all_replicated = false;
    for _ in 0..30 {
        sleep(Duration::from_millis(200)).await;
        all_replicated = true;
        for (node_id, cp) in &cluster.nodes {
            let agents = cp.get_agents().await;
            if !agents.contains_key("test-agent-1") {
                all_replicated = false;
                break;
            }
        }
        if all_replicated {
            break;
        }
    }
    
    // Verify agent is registered on all nodes
    for (node_id, cp) in &cluster.nodes {
        let agents = cp.get_agents().await;
        assert!(
            agents.contains_key("test-agent-1"),
            "Agent should be registered on node {}",
            node_id
        );
    }
    
    cluster.stop_all().await;
}

#[tokio::test]
async fn test_leader_failover() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    // Wait for initial leader election - check multiple times
    let mut initial_leader = None;
    for _ in 0..20 {
        sleep(Duration::from_millis(200)).await;
        initial_leader = cluster.get_leader().await;
        if initial_leader.is_some() {
            break;
        }
    }
    let initial_leader = initial_leader.expect("No initial leader");
    let (leader_id, leader_cp) = initial_leader.clone();
    tracing::info!("Initial leader: {}", leader_id);
    
    // Stop the leader
    leader_cp.stop().await.unwrap();
    tracing::info!("Stopped leader {}", leader_id);
    
    // Wait for new leader election - check multiple times
    // Need to ensure the stopped node is not considered
    let mut new_leader_opt = None;
    for _ in 0..30 {
        sleep(Duration::from_millis(200)).await;
        // Get leader, but skip the stopped node
        for (node_id, cp) in &cluster.nodes {
            if *node_id == leader_id {
                continue; // Skip the stopped leader
            }
            if cp.is_leader().await {
                new_leader_opt = Some((node_id.clone(), Arc::clone(cp)));
                break;
            }
        }
        if new_leader_opt.is_some() {
            break;
        }
    }
    let new_leader = new_leader_opt.expect("Expected a new leader after failover");
    
    let (new_leader_id, _) = new_leader.clone();
    assert_ne!(
        new_leader_id,
        leader_id,
        "New leader should be different from old leader"
    );
    
    cluster.stop_all().await;
}

#[tokio::test]
async fn test_five_node_cluster() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let cluster = TestCluster::new(5).await;
    cluster.start_all().await;
    
    // Wait for leader election and stability - check multiple times
    let mut leader_count = 0;
    for _ in 0..20 {
        sleep(Duration::from_millis(200)).await;
        leader_count = 0;
        for (node_id, cp) in &cluster.nodes {
            if cp.is_leader().await {
                leader_count += 1;
            }
        }
        if leader_count == 1 {
            // Found exactly one leader, verify it's stable
            sleep(Duration::from_millis(500)).await;
            leader_count = 0;
            for (node_id, cp) in &cluster.nodes {
                if cp.is_leader().await {
                    leader_count += 1;
                    tracing::info!("Node {} is leader", node_id);
                }
            }
            break;
        }
    }
    
    assert_eq!(leader_count, 1, "Expected exactly one leader in 5-node cluster");
    
    cluster.stop_all().await;
}
