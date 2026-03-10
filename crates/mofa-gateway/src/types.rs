//! Core types for the control plane and gateway.
//!
//! This module defines the fundamental data structures used throughout the
//! control plane and gateway components.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};

/// Unique identifier for a node in the cluster.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    /// Create a new node ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a random node ID.
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for NodeId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for NodeId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

/// Network address of a node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeAddress {
    /// Control plane address (for consensus)
    pub control_plane: SocketAddr,
    /// Gateway address (for client requests)
    pub gateway: SocketAddr,
}

/// Information about a cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique node identifier.
    pub id: NodeId,
    /// Network addresses.
    pub address: NodeAddress,
    /// Node metadata.
    pub metadata: HashMap<String, String>,
    /// When the node joined the cluster.
    pub joined_at: SystemTime,
    /// Current node status.
    pub status: NodeStatus,
}

/// Status of a cluster node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum NodeStatus {
    /// Node is starting up.
    Starting,
    /// Node is healthy and operational.
    Healthy,
    /// Node is unhealthy (failing health checks).
    Unhealthy,
    /// Node is leaving the cluster gracefully.
    Leaving,
    /// Node has left the cluster.
    Left,
}

/// Raft term (monotonically increasing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Term(pub u64);

impl Term {
    /// Create a new term.
    pub fn new(term: u64) -> Self {
        Self(term)
    }

    /// Increment the term.
    pub fn increment(self) -> Self {
        Self(self.0 + 1)
    }
}

impl std::fmt::Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Raft log index (monotonically increasing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LogIndex(pub u64);

impl LogIndex {
    /// Create a new log index.
    pub fn new(index: u64) -> Self {
        Self(index)
    }

    /// Increment the index.
    pub fn increment(self) -> Self {
        Self(self.0 + 1)
    }
}

impl std::fmt::Display for LogIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Raft log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Term when entry was received by leader.
    pub term: Term,
    /// Index in the log.
    pub index: LogIndex,
    /// Entry data (serialized state machine command).
    pub data: Vec<u8>,
}

/// Raft state (Follower, Candidate, Leader).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RaftState {
    /// Follower state - receiving log entries from leader.
    Follower,
    /// Candidate state - participating in leader election.
    Candidate,
    /// Leader state - accepting requests and replicating log.
    Leader,
}

/// State machine command (what gets replicated).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum StateMachineCommand {
    /// Add a node to the cluster.
    AddNode {
        /// ID of the node to add.
        node_id: NodeId,
        /// Network addresses for the node.
        address: NodeAddress,
    },
    /// Remove a node from the cluster.
    RemoveNode {
        /// ID of the node to remove.
        node_id: NodeId,
    },
    /// Register a new agent.
    RegisterAgent {
        /// Unique identifier for the agent.
        agent_id: String,
        /// Additional metadata for the agent.
        metadata: HashMap<String, String>,
    },
    /// Unregister an agent.
    UnregisterAgent {
        /// ID of the agent to unregister.
        agent_id: String,
    },
    /// Update agent state.
    UpdateAgentState {
        /// ID of the agent to update.
        agent_id: String,
        /// New state for the agent.
        state: String,
    },
    /// Update cluster configuration.
    UpdateConfig {
        /// Configuration key to update.
        key: String,
        /// New value for the configuration key.
        value: String,
    },
}

/// Cluster membership information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMembership {
    /// All nodes in the cluster.
    pub nodes: HashMap<NodeId, NodeInfo>,
    /// Current leader (if any).
    pub leader: Option<NodeId>,
    /// Current term.
    pub current_term: Term,
}

/// Gateway request metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    /// Request ID (for tracing).
    pub request_id: String,
    /// Client IP address.
    pub client_ip: Option<SocketAddr>,
    /// User ID (if authenticated).
    pub user_id: Option<String>,
    /// Request timestamp.
    pub timestamp: SystemTime,
    /// Additional metadata.
    pub extra: HashMap<String, String>,
}

/// Load balancing algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LoadBalancingAlgorithm {
    /// Round-robin: cycle through nodes.
    RoundRobin,
    /// Least connections: route to node with fewest active connections.
    LeastConnections,
    /// Weighted round-robin: use node weights.
    WeightedRoundRobin,
    /// Random: randomly select a node.
    Random,
}

/// Rate limiting strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RateLimitStrategy {
    /// Token bucket algorithm.
    TokenBucket {
        /// Maximum number of tokens in the bucket.
        capacity: u64,
        /// Token refill rate (tokens per second).
        refill_rate: u64,
    },
    /// Sliding window algorithm.
    SlidingWindow {
        /// Size of the sliding time window.
        window_size: Duration,
        /// Maximum number of requests allowed in the window.
        max_requests: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id() {
        let id1 = NodeId::new("node-1");
        let id2 = NodeId::random();
        assert_ne!(id1, id2);
        assert_eq!(id1.to_string(), "node-1");
    }

    #[test]
    fn test_term_ordering() {
        let term1 = Term::new(1);
        let term2 = Term::new(2);
        assert!(term1 < term2);
        assert_eq!(term1.increment(), term2);
    }

    #[test]
    fn test_log_index_ordering() {
        let idx1 = LogIndex::new(1);
        let idx2 = LogIndex::new(2);
        assert!(idx1 < idx2);
        assert_eq!(idx1.increment(), idx2);
    }
}
