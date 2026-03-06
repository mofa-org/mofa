//! Raft consensus engine example.
//!
//! This example demonstrates how to set up and use the Raft consensus
//! engine for distributed coordination.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example raft_consensus --package mofa-gateway
//! ```

use mofa_gateway::consensus::engine::{ConsensusEngine, RaftConfig};
use mofa_gateway::consensus::storage::RaftStorage;
use mofa_gateway::consensus::transport_impl::InMemoryTransport;
use mofa_gateway::types::{NodeId, StateMachineCommand};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("Starting Raft Consensus Engine example");

    // Create a 3-node cluster
    let node1_id = NodeId::new("node-1");
    let node2_id = NodeId::new("node-2");
    let node3_id = NodeId::new("node-3");

    let cluster_nodes = vec![node1_id.clone(), node2_id.clone(), node3_id.clone()];

    // Create storage for each node
    let storage1 = Arc::new(RaftStorage::open("./raft_data/node1")?);
    let storage2 = Arc::new(RaftStorage::open("./raft_data/node2")?);
    let storage3 = Arc::new(RaftStorage::open("./raft_data/node3")?);

    // Create transport
    let transport = Arc::new(InMemoryTransport::new());

    // Create consensus engines
    let config = RaftConfig {
        cluster_nodes: cluster_nodes.clone(),
        election_timeout_ms: (150, 300),
        heartbeat_interval_ms: 50,
    };

    let transport_dyn: Arc<dyn mofa_gateway::consensus::RaftTransport> = transport.clone();
    
    let engine1 = Arc::new(ConsensusEngine::new(
        node1_id.clone(),
        config.clone(),
        storage1,
        transport_dyn.clone(),
    ));

    let engine2 = Arc::new(ConsensusEngine::new(
        node2_id.clone(),
        config.clone(),
        storage2,
        transport_dyn.clone(),
    ));

    let engine3 = Arc::new(ConsensusEngine::new(
        node3_id.clone(),
        config,
        storage3,
        transport_dyn,
    ));

    // Register handlers (in a real implementation, this would be done by the transport)
    // For this example, we'll skip the actual RPC handling

    // Start all engines
    tracing::info!("Starting consensus engines...");
    engine1.start().await?;
    engine2.start().await?;
    engine3.start().await?;

    // Wait for leader election
    tracing::info!("Waiting for leader election...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check which node is leader
    let is_leader1 = engine1.is_leader().await;
    let is_leader2 = engine2.is_leader().await;
    let is_leader3 = engine3.is_leader().await;

    tracing::info!("Node 1 is leader: {}", is_leader1);
    tracing::info!("Node 2 is leader: {}", is_leader2);
    tracing::info!("Node 3 is leader: {}", is_leader3);

    // Propose a command (only leader can do this)
    if is_leader1 {
        let command = StateMachineCommand::RegisterAgent {
            agent_id: "agent-1".to_string(),
            metadata: HashMap::new(),
        };
        match engine1.propose(command).await {
            Ok(index) => tracing::info!("Proposed command, log index: {}", index.0),
            Err(e) => tracing::error!("Failed to propose: {}", e),
        }
    }

    // Stop all engines
    tracing::info!("Stopping consensus engines...");
    engine1.stop().await?;
    engine2.stop().await?;
    engine3.stop().await?;

    tracing::info!("Example completed");
    Ok(())
}
