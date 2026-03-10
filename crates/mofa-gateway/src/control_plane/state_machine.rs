//! Replicated state machine.
//!
//! This module implements the replicated state machine that applies
//! commands replicated via Raft consensus.
//!
//! # Implementation
//!
//! The state machine applies commands in the order they appear in the
//! Raft log, ensuring all nodes see the same state transitions.

use crate::control_plane::membership::ClusterMembershipManager;
use crate::error::ControlPlaneResult;
use crate::types::{NodeAddress, NodeId, NodeInfo, NodeStatus, StateMachineCommand};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Replicated state machine.
pub struct ReplicatedStateMachine {
    /// Agent registry (replicated across cluster).
    agent_registry: Arc<RwLock<HashMap<String, AgentRegistryEntry>>>,
    /// Cluster membership (replicated).
    membership: Arc<RwLock<ClusterMembershipManager>>,
    /// Configuration (replicated).
    config: Arc<RwLock<HashMap<String, String>>>,
}

/// Entry in the replicated agent registry.
#[derive(Debug, Clone)]
pub struct AgentRegistryEntry {
    /// Unique identifier for the agent.
    pub agent_id: String,
    /// Type/category of the agent.
    pub agent_type: String,
    /// Additional metadata associated with the agent.
    pub metadata: HashMap<String, String>,
    /// Timestamp when the agent was registered (milliseconds since epoch).
    pub registered_at: u64,
}

impl ReplicatedStateMachine {
    /// Create a new state machine.
    pub fn new() -> Self {
        Self {
            agent_registry: Arc::new(RwLock::new(HashMap::new())),
            membership: Arc::new(RwLock::new(ClusterMembershipManager::new())),
            config: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Apply a command to the state machine.
    pub async fn apply(&self, command: StateMachineCommand) -> ControlPlaneResult<Vec<u8>> {
        debug!("Applying state machine command: {:?}", command);

        match command {
            StateMachineCommand::AddNode { node_id, address } => {
                self.apply_add_node(node_id, address).await?;
            }
            StateMachineCommand::RemoveNode { node_id } => {
                self.apply_remove_node(&node_id).await?;
            }
            StateMachineCommand::RegisterAgent { agent_id, metadata } => {
                self.apply_register_agent(agent_id, metadata).await?;
            }
            StateMachineCommand::UnregisterAgent { agent_id } => {
                self.apply_unregister_agent(&agent_id).await?;
            }
            StateMachineCommand::UpdateConfig { key, value } => {
                self.apply_update_config(&key, &value).await?;
            }
            StateMachineCommand::UpdateAgentState { agent_id, state } => {
                self.apply_update_agent_state(&agent_id, &state).await?;
            }
        }

        // Return empty result (could return serialized state if needed)
        Ok(Vec::new())
    }

    /// Apply add node command.
    async fn apply_add_node(&self, node_id: NodeId, address: NodeAddress) -> ControlPlaneResult<()> {
        let mut membership = self.membership.write().await;

        // Check if node already exists
        if membership.has_node(&node_id) {
            warn!("Node {} already exists, updating", node_id);
            // Update existing node
            if let Some(node) = membership.get_membership().nodes.get(&node_id) {
                let mut updated_node = node.clone();
                updated_node.address = address;
                updated_node.status = NodeStatus::Healthy;
                membership.add_node(updated_node);
            }
        } else {
            // Create new node info
            let node_info = NodeInfo {
                id: node_id.clone(),
                address,
                metadata: HashMap::new(),
                joined_at: std::time::SystemTime::now(),
                status: NodeStatus::Starting,
            };
            membership.add_node(node_info);
        }

        info!("Applied add_node command for {}", node_id);
        Ok(())
    }

    /// Apply remove node command.
    async fn apply_remove_node(&self, node_id: &NodeId) -> ControlPlaneResult<()> {
        let mut membership = self.membership.write().await;
        membership.remove_node(node_id);
        info!("Applied remove_node command for {}", node_id);
        Ok(())
    }

    /// Apply register agent command.
    async fn apply_register_agent(
        &self,
        agent_id: String,
        metadata: HashMap<String, String>,
    ) -> ControlPlaneResult<()> {
        let mut registry = self.agent_registry.write().await;

        let entry = AgentRegistryEntry {
            agent_id: agent_id.clone(),
            agent_type: metadata
                .get("type")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            metadata: metadata.clone(),
            registered_at: {
                let millis = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                u64::try_from(millis).unwrap_or(u64::MAX)
            },
        };

        registry.insert(agent_id.clone(), entry);
        info!("Applied register_agent command for {}", agent_id);
        Ok(())
    }

    /// Apply unregister agent command.
    async fn apply_unregister_agent(&self, agent_id: &str) -> ControlPlaneResult<()> {
        let mut registry = self.agent_registry.write().await;
        registry.remove(agent_id);
        info!("Applied unregister_agent command for {}", agent_id);
        Ok(())
    }

    /// Apply update config command.
    async fn apply_update_config(&self, key: &str, value: &str) -> ControlPlaneResult<()> {
        let mut config = self.config.write().await;
        config.insert(key.to_string(), value.to_string());
        debug!("Applied update_config command: {} = {}", key, value);
        Ok(())
    }

    /// Apply update agent state command.
    async fn apply_update_agent_state(&self, agent_id: &str, state: &str) -> ControlPlaneResult<()> {
        let mut registry = self.agent_registry.write().await;
        if let Some(entry) = registry.get_mut(agent_id) {
            entry.metadata.insert("state".to_string(), state.to_string());
            debug!("Applied update_agent_state command: {} = {}", agent_id, state);
        } else {
            warn!("Agent {} not found for state update", agent_id);
        }
        Ok(())
    }

    /// Get agent registry.
    pub async fn get_agent_registry(&self) -> HashMap<String, AgentRegistryEntry> {
        self.agent_registry.read().await.clone()
    }

    /// Get membership manager reference.
    pub fn membership(&self) -> Arc<RwLock<ClusterMembershipManager>> {
        Arc::clone(&self.membership)
    }

    /// Get config.
    pub async fn get_config(&self) -> HashMap<String, String> {
        self.config.read().await.clone()
    }

    /// Get all agents.
    pub async fn get_agents(&self) -> std::collections::HashMap<String, AgentRegistryEntry> {
        self.agent_registry.read().await.clone()
    }

    /// Get a specific agent.
    pub async fn get_agent(&self, agent_id: &str) -> Option<AgentRegistryEntry> {
        self.agent_registry.read().await.get(agent_id).cloned()
    }
}

impl Default for ReplicatedStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeAddress;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[tokio::test]
    async fn test_register_agent() {
        let sm = ReplicatedStateMachine::new();
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "test-agent".to_string());

        let command = StateMachineCommand::RegisterAgent {
            agent_id: "agent-1".to_string(),
            metadata: metadata.clone(),
        };

        sm.apply(command).await.unwrap();

        let registry = sm.get_agent_registry().await;
        assert!(registry.contains_key("agent-1"));
        assert_eq!(registry["agent-1"].agent_type, "test-agent");
    }

    #[tokio::test]
    async fn test_unregister_agent() {
        let sm = ReplicatedStateMachine::new();
        let metadata = HashMap::new();

        let register_cmd = StateMachineCommand::RegisterAgent {
            agent_id: "agent-1".to_string(),
            metadata,
        };
        sm.apply(register_cmd).await.unwrap();

        let unregister_cmd = StateMachineCommand::UnregisterAgent {
            agent_id: "agent-1".to_string(),
        };
        sm.apply(unregister_cmd).await.unwrap();

        let registry = sm.get_agent_registry().await;
        assert!(!registry.contains_key("agent-1"));
    }

    #[tokio::test]
    async fn test_add_remove_node() {
        let sm = ReplicatedStateMachine::new();
        let node_id = NodeId::new("node-1");
        let address = NodeAddress {
            control_plane: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            gateway: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9090),
        };

        let add_cmd = StateMachineCommand::AddNode {
            node_id: node_id.clone(),
            address: address.clone(),
        };
        sm.apply(add_cmd).await.unwrap();

        let membership_arc = sm.membership();
        let membership = membership_arc.read().await;
        assert!(membership.has_node(&node_id));
        drop(membership);

        let remove_cmd = StateMachineCommand::RemoveNode { node_id };
        sm.apply(remove_cmd).await.unwrap();

        let membership_arc = sm.membership();
        let membership = membership_arc.read().await;
        assert!(!membership.has_node(&NodeId::new("node-1")));
    }
}
