//! Cluster membership management.
//!
//! This module handles cluster membership, including:
//! - Adding/removing nodes
//! - Node health tracking
//! - Leader tracking
//!
//! # Implementation
//!
//! Membership changes are replicated via Raft consensus to ensure
//! all nodes have consistent view of the cluster.

use crate::types::{ClusterMembership as ClusterMembershipData, NodeId, NodeInfo, NodeStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Cluster membership manager.
pub struct ClusterMembershipManager {
    membership: ClusterMembershipData,
}

impl ClusterMembershipManager {
    /// Create a new membership manager.
    pub fn new() -> Self {
        Self {
            membership: ClusterMembershipData {
                nodes: HashMap::new(),
                leader: None,
                current_term: crate::types::Term::new(0),
            },
        }
    }

    /// Add a node to the cluster.
    pub fn add_node(&mut self, node: NodeInfo) {
        info!("Adding node {} to cluster", node.id);
        self.membership.nodes.insert(node.id.clone(), node);
    }

    /// Remove a node from the cluster.
    pub fn remove_node(&mut self, node_id: &NodeId) {
        info!("Removing node {} from cluster", node_id);
        self.membership.nodes.remove(node_id);
        if self.membership.leader.as_ref() == Some(node_id) {
            warn!("Removed node {} was the leader", node_id);
            self.membership.leader = None;
        }
    }

    /// Update node status.
    pub fn update_node_status(&mut self, node_id: &NodeId, status: NodeStatus) {
        if let Some(node) = self.membership.nodes.get_mut(node_id) {
            debug!("Updating node {} status to {:?}", node_id, status);
            node.status = status;
        }
    }

    /// Set the current leader.
    pub fn set_leader(&mut self, leader: Option<NodeId>) {
        self.membership.leader = leader.clone();
        if let Some(ref leader_id) = leader {
            info!("Leader set to {}", leader_id);
        } else {
            warn!("Leader cleared");
        }
    }

    /// Update the current term.
    pub fn update_term(&mut self, term: crate::types::Term) {
        if term > self.membership.current_term {
            debug!("Updating term from {} to {}", self.membership.current_term.0, term.0);
            self.membership.current_term = term;
        }
    }

    /// Get cluster membership data.
    pub fn get_membership(&self) -> ClusterMembershipData {
        self.membership.clone()
    }

    /// Get all node IDs.
    pub fn get_node_ids(&self) -> Vec<NodeId> {
        self.membership.nodes.keys().cloned().collect()
    }

    /// Get node info.
    pub fn get_node(&self, node_id: &NodeId) -> Option<&NodeInfo> {
        self.membership.nodes.get(node_id)
    }

    /// Check if a node exists.
    pub fn has_node(&self, node_id: &NodeId) -> bool {
        self.membership.nodes.contains_key(node_id)
    }

    /// Get healthy nodes.
    pub fn get_healthy_nodes(&self) -> Vec<&NodeInfo> {
        self.membership
            .nodes
            .values()
            .filter(|node| node.status == NodeStatus::Healthy)
            .collect()
    }

    /// Get the current leader.
    pub fn get_leader(&self) -> Option<&NodeId> {
        self.membership.leader.as_ref()
    }
}

impl Default for ClusterMembershipManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeAddress;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn create_test_node(id: &str) -> NodeInfo {
        NodeInfo {
            id: NodeId::new(id),
            address: NodeAddress {
                control_plane: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                gateway: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9090),
            },
            metadata: HashMap::new(),
            joined_at: std::time::SystemTime::now(),
            status: NodeStatus::Healthy,
        }
    }

    #[test]
    fn test_add_remove_node() {
        let mut manager = ClusterMembershipManager::new();
        let node = create_test_node("node-1");

        manager.add_node(node);
        assert!(manager.has_node(&NodeId::new("node-1")));

        manager.remove_node(&NodeId::new("node-1"));
        assert!(!manager.has_node(&NodeId::new("node-1")));
    }

    #[test]
    fn test_update_status() {
        let mut manager = ClusterMembershipManager::new();
        let node = create_test_node("node-1");
        manager.add_node(node);

        manager.update_node_status(&NodeId::new("node-1"), NodeStatus::Unhealthy);
        let node = manager.get_node(&NodeId::new("node-1")).unwrap();
        assert_eq!(node.status, NodeStatus::Unhealthy);
    }

    #[test]
    fn test_set_leader() {
        let mut manager = ClusterMembershipManager::new();
        let node_id = NodeId::new("node-1");

        manager.set_leader(Some(node_id.clone()));
        assert_eq!(manager.get_leader(), Some(&node_id));

        manager.set_leader(None);
        assert_eq!(manager.get_leader(), None);
    }

    #[test]
    fn test_get_healthy_nodes() {
        let mut manager = ClusterMembershipManager::new();
        let mut node1 = create_test_node("node-1");
        let mut node2 = create_test_node("node-2");
        node2.status = NodeStatus::Unhealthy;

        manager.add_node(node1);
        manager.add_node(node2);

        let healthy = manager.get_healthy_nodes();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].id, NodeId::new("node-1"));
    }
}
