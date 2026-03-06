//! Mock transport implementation for testing.
//!
//! This module provides a simple in-memory transport implementation
//! for testing the Raft consensus engine without network dependencies.

use crate::consensus::transport::{
    AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
    RaftTransport,
};
use crate::error::ConsensusResult;
use crate::types::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory transport for testing.
pub struct InMemoryTransport {
    /// Map of node ID to their consensus engine handler
    handlers: Arc<RwLock<HashMap<NodeId, Arc<dyn ConsensusHandler + Send + Sync>>>>,
}

/// Trait for handling consensus RPCs.
#[async_trait::async_trait]
pub trait ConsensusHandler: Send + Sync {
    /// Handle a RequestVote RPC.
    async fn handle_request_vote(
        &self,
        request: RequestVoteRequest,
    ) -> ConsensusResult<RequestVoteResponse>;

    /// Handle an AppendEntries RPC.
    async fn handle_append_entries(
        &self,
        request: AppendEntriesRequest,
    ) -> ConsensusResult<AppendEntriesResponse>;
}

impl InMemoryTransport {
    /// Create a new in-memory transport.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a handler for a node.
    pub async fn register_handler(&self, node_id: NodeId, handler: Arc<dyn ConsensusHandler + Send + Sync>) {
        self.handlers.write().await.insert(node_id, handler);
    }

    /// Unregister a handler for a node.
    pub async fn unregister_handler(&self, node_id: &NodeId) {
        self.handlers.write().await.remove(node_id);
    }
}

impl Default for InMemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RaftTransport for InMemoryTransport {
    async fn request_vote(
        &self,
        node_id: &NodeId,
        request: RequestVoteRequest,
    ) -> ConsensusResult<RequestVoteResponse> {
        let handlers = self.handlers.read().await;
        if let Some(handler) = handlers.get(node_id) {
            handler.handle_request_vote(request).await
        } else {
            Err(crate::error::ConsensusError::Internal(format!(
                "Node {} not found",
                node_id
            )))
        }
    }

    async fn append_entries(
        &self,
        node_id: &NodeId,
        request: AppendEntriesRequest,
    ) -> ConsensusResult<AppendEntriesResponse> {
        let handlers = self.handlers.read().await;
        if let Some(handler) = handlers.get(node_id) {
            handler.handle_append_entries(request).await
        } else {
            Err(crate::error::ConsensusError::Internal(format!(
                "Node {} not found",
                node_id
            )))
        }
    }
}
