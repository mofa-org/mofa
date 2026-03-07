//! Control plane cluster example.
//!
//! This example demonstrates how to set up a multi-node control plane
//! cluster with Raft consensus.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example control_plane_cluster --package mofa-gateway
//! ```

use mofa_gateway::control_plane::{ControlPlane, ControlPlaneConfig};
use mofa_gateway::consensus::storage::RaftStorage;
use mofa_gateway::consensus::transport_impl::InMemoryTransport;
use mofa_gateway::types::{NodeAddress, NodeId};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("Starting Control Plane Cluster example");

    // Create a 3-node cluster
    let node1_id = NodeId::new("node-1");
    let node2_id = NodeId::new("node-2");
    let node3_id = NodeId::new("node-3");

    let cluster_nodes = vec![node1_id.clone(), node2_id.clone(), node3_id.clone()];

    // Create storage for each node
    let storage1 = Arc::new(RaftStorage::open("./control_plane_data/node1")?);
    let storage2 = Arc::new(RaftStorage::open("./control_plane_data/node2")?);
    let storage3 = Arc::new(RaftStorage::open("./control_plane_data/node3")?);

    // Create transport
    let transport = Arc::new(InMemoryTransport::new());

    // Create control plane instances
    let config1 = ControlPlaneConfig {
        node_id: node1_id.clone(),
        cluster_nodes: cluster_nodes.clone(),
        storage_path: "./control_plane_data/node1".to_string(),
        election_timeout_ms: 150,
        heartbeat_interval_ms: 50,
    };

    let config2 = ControlPlaneConfig {
        node_id: node2_id.clone(),
        cluster_nodes: cluster_nodes.clone(),
        storage_path: "./control_plane_data/node2".to_string(),
        election_timeout_ms: 150,
        heartbeat_interval_ms: 50,
    };

    let config3 = ControlPlaneConfig {
        node_id: node3_id.clone(),
        cluster_nodes: cluster_nodes.clone(),
        storage_path: "./control_plane_data/node3".to_string(),
        election_timeout_ms: 150,
        heartbeat_interval_ms: 50,
    };

    let transport_dyn: Arc<dyn mofa_gateway::consensus::RaftTransport> = transport.clone();
    
    let cp1 = ControlPlane::new(config1, storage1, transport_dyn.clone()).await?;
    let cp2 = ControlPlane::new(config2, storage2, transport_dyn.clone()).await?;
    let cp3 = ControlPlane::new(config3, storage3, transport_dyn).await?;

    // Start all control planes
    tracing::info!("Starting control planes...");
    cp1.start().await?;
    cp2.start().await?;
    cp3.start().await?;

    // Wait for leader election
    tracing::info!("Waiting for leader election...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check which node is leader
    let is_leader1 = cp1.is_leader().await;
    let is_leader2 = cp2.is_leader().await;
    let is_leader3 = cp3.is_leader().await;

    tracing::info!("Node 1 is leader: {}", is_leader1);
    tracing::info!("Node 2 is leader: {}", is_leader2);
    tracing::info!("Node 3 is leader: {}", is_leader3);

    // Use the leader to register an agent
    let leader = if is_leader1 {
        &cp1
    } else if is_leader2 {
        &cp2
    } else {
        &cp3
    };

    if let Ok(_) = leader.register_agent("agent-1".to_string(), HashMap::new()).await {
        tracing::info!("Successfully registered agent via leader");
    }

    // Get cluster membership
    let membership = leader.get_membership().await;
    tracing::info!("Cluster membership: {} nodes", membership.nodes.len());

    // Stop all control planes
    tracing::info!("Stopping control planes...");
    cp1.stop().await?;
    cp2.stop().await?;
    cp3.stop().await?;

    tracing::info!("Example completed");
    Ok(())
}
