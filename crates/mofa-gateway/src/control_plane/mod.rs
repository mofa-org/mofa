//! Control plane core implementation.
//!
//! This module provides the main control plane functionality:
//! - Cluster membership management
//! - State machine replication
//! - Agent registry synchronization
//! - Configuration management
//!
//! # Architecture
//!
//! The control plane coordinates cluster-wide state using Raft consensus:
//! - All state changes go through Raft (leader proposes, followers replicate)
//! - State machine applies commands in order
//! - Agent registry is replicated across all nodes
//! - Cluster membership changes are also replicated

pub mod membership;
pub mod state_machine;

pub use membership::ClusterMembershipManager;
pub use state_machine::*;

use crate::consensus::engine::{ConsensusEngine, RaftConfig};
use crate::consensus::state::RaftNodeState;
use crate::consensus::storage::RaftStorage;
use crate::consensus::transport::RaftTransport;
use crate::error::{ControlPlaneError, ControlPlaneResult};
use crate::types::{ClusterMembership as ClusterMembershipData, NodeId, StateMachineCommand};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for the control plane.
#[derive(Debug, Clone)]
pub struct ControlPlaneConfig {
    /// Local node ID.
    pub node_id: NodeId,
    /// Storage path for persistent state.
    pub storage_path: String,
    /// Election timeout (milliseconds).
    pub election_timeout_ms: u64,
    /// Heartbeat interval (milliseconds).
    pub heartbeat_interval_ms: u64,
    /// List of all node IDs in the cluster.
    pub cluster_nodes: Vec<NodeId>,
}

impl Default for ControlPlaneConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::random(),
            storage_path: "./control_plane_data".to_string(),
            election_timeout_ms: 150,
            heartbeat_interval_ms: 50,
            cluster_nodes: Vec::new(),
        }
    }
}

/// Control plane instance.
pub struct ControlPlane {
    config: ControlPlaneConfig,
    consensus: Arc<ConsensusEngine>,
    membership: Arc<RwLock<ClusterMembershipManager>>,
    state_machine: Arc<RwLock<ReplicatedStateMachine>>,
}

impl ControlPlane {
    /// Create a new control plane instance.
    pub async fn new(
        config: ControlPlaneConfig,
        storage: Arc<RaftStorage>,
        transport: Arc<dyn RaftTransport>,
    ) -> ControlPlaneResult<Self> {
        info!("Creating control plane for node {}", config.node_id);

        // Create Raft config
        let raft_config = RaftConfig {
            election_timeout_ms: (config.election_timeout_ms, config.election_timeout_ms * 2),
            heartbeat_interval_ms: config.heartbeat_interval_ms,
            cluster_nodes: config.cluster_nodes.clone(),
        };

        // Create consensus engine
        let consensus = Arc::new(ConsensusEngine::new(
            config.node_id.clone(),
            raft_config,
            storage,
            transport,
        ));

        // Create membership manager
        let membership = Arc::new(RwLock::new(ClusterMembershipManager::new()));

        // Create state machine
        let state_machine = Arc::new(RwLock::new(ReplicatedStateMachine::new()));

        Ok(Self {
            config,
            consensus,
            membership,
            state_machine,
        })
    }

    /// Start the control plane.
    pub async fn start(&self) -> ControlPlaneResult<()> {
        info!("Starting control plane for node {}", self.config.node_id);

        // Start consensus engine
        self.consensus.start().await?;

        // Start state machine apply loop
        self.start_state_machine_loop().await;

        info!("Control plane started successfully");
        Ok(())
    }

    /// Stop the control plane.
    pub async fn stop(&self) -> ControlPlaneResult<()> {
        info!("Stopping control plane for node {}", self.config.node_id);

        // Stop consensus engine
        self.consensus.stop().await?;

        info!("Control plane stopped");
        Ok(())
    }

    /// Add a node to the cluster (leader only).
    pub async fn add_node(&self, node_id: NodeId, address: crate::types::NodeAddress) -> ControlPlaneResult<()> {
        // Check if we're the leader
        if !self.consensus.is_leader().await {
            return Err(ControlPlaneError::NotLeader);
        }

        // Propose command via consensus
        let command = StateMachineCommand::AddNode {
            node_id: node_id.clone(),
            address: address.clone(),
        };

        // Propose the command and rely on the state machine apply loop to apply
        // committed entries once quorum is reached.
        self.consensus.propose(command).await?;
        
        info!("Added node {} to cluster via consensus", node_id);

        Ok(())
    }

    /// Remove a node from the cluster (leader only).
    pub async fn remove_node(&self, node_id: &NodeId) -> ControlPlaneResult<()> {
        // Check if we're the leader
        if !self.consensus.is_leader().await {
            return Err(ControlPlaneError::NotLeader);
        }

        // Propose command via consensus
        let command = StateMachineCommand::RemoveNode {
            node_id: node_id.clone(),
        };

        // Propose the command; the state machine apply loop will apply it once
        // the corresponding log entry is committed.
        self.consensus.propose(command).await?;
        
        info!("Removed node {} from cluster via consensus", node_id);

        Ok(())
    }

    /// Register an agent (leader only, replicated via Raft).
    pub async fn register_agent(
        &self,
        agent_id: String,
        metadata: std::collections::HashMap<String, String>,
    ) -> ControlPlaneResult<()> {
        // Check if we're the leader
        if !self.consensus.is_leader().await {
            return Err(ControlPlaneError::NotLeader);
        }

        // Propose command via consensus
        let command = StateMachineCommand::RegisterAgent {
            agent_id: agent_id.clone(),
            metadata: metadata.clone(),
        };

        // Propose the command; state changes are applied by the state machine
        // apply loop once the entry is committed.
        self.consensus.propose(command).await?;
        
        info!("Registered agent {} via consensus", agent_id);

        Ok(())
    }

    /// Unregister an agent (leader only, replicated via Raft).
    pub async fn unregister_agent(&self, agent_id: &str) -> ControlPlaneResult<()> {
        // Check if we're the leader
        if !self.consensus.is_leader().await {
            return Err(ControlPlaneError::NotLeader);
        }

        // Propose command via consensus
        let command = StateMachineCommand::UnregisterAgent {
            agent_id: agent_id.to_string(),
        };

        // Propose the command and rely on the state machine apply loop to apply
        // it once committed, keeping all nodes consistent.
        self.consensus.propose(command).await?;
        
        info!("Unregistered agent {} via consensus", agent_id);

        Ok(())
    }

    /// Get cluster membership.
    pub async fn get_membership(&self) -> ClusterMembershipData {
        // Derive cluster membership from the replicated state machine so that
        // all nodes observe the same, Raft-committed view of the cluster.
        let sm = self.state_machine.read().await;
        let membership_manager = sm.membership();
        let membership = membership_manager.read().await;
        membership.get_membership()
    }

    /// Check if this node is the leader.
    pub async fn is_leader(&self) -> bool {
        self.consensus.is_leader().await
    }

    /// Get current term.
    pub async fn current_term(&self) -> crate::types::Term {
        self.consensus.current_term().await
    }

    /// Get consensus engine (for testing and handler registration).
    pub fn consensus(&self) -> &Arc<crate::consensus::engine::ConsensusEngine> {
        &self.consensus
    }

    /// Get state machine reference (for accessing agent registry).
    pub fn state_machine(&self) -> Arc<RwLock<ReplicatedStateMachine>> {
        Arc::clone(&self.state_machine)
    }

    /// Get all agents (async wrapper).
    pub async fn get_agents(&self) -> std::collections::HashMap<String, crate::control_plane::state_machine::AgentRegistryEntry> {
        let sm = self.state_machine.read().await;
        sm.get_agents().await
    }

    /// Get a specific agent (async wrapper).
    pub async fn get_agent(&self, agent_id: &str) -> Option<crate::control_plane::state_machine::AgentRegistryEntry> {
        let sm = self.state_machine.read().await;
        sm.get_agent(agent_id).await
    }

    /// Start the state machine apply loop.
    async fn start_state_machine_loop(&self) {
        let state_machine = Arc::clone(&self.state_machine);
        let consensus = Arc::clone(&self.consensus);

        tokio::spawn(async move {
            let mut last_applied = 0u64;

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                // Get committed log entries from consensus engine
                let (commit_index, entries) = consensus.get_committed_entries(last_applied).await;
                
                if !entries.is_empty() {
                    info!("State machine apply loop: found {} entries to apply (commit_index: {}, last_applied: {})", 
                        entries.len(), commit_index, last_applied);
                }

                // Apply any new committed entries
                for entry in entries {
                    // Deserialize command from log entry
                    match bincode::deserialize::<StateMachineCommand>(&entry.data) {
                        Ok(command) => {
                            info!("Node applying committed command: {:?}", command);
                            if let Err(e) = state_machine.write().await.apply(command).await {
                                warn!("Failed to apply command: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to deserialize command from log entry: {}", e);
                        }
                    }
                }

                if commit_index > last_applied {
                    last_applied = commit_index;
                    debug!("Applied entries up to index {}", last_applied);
                }
            }
        });

        info!("State machine apply loop started");
    }
}
