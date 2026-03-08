//! Tests for the Raft consensus engine.

#[cfg(test)]
mod tests {
    use crate::consensus::engine::{ConsensusEngine, RaftConfig};
    use crate::consensus::storage::RaftStorage;
    use crate::consensus::transport_impl::{ConsensusHandler, InMemoryTransport};
    use crate::types::{NodeId, StateMachineCommand};
    use std::collections::HashMap;
    use std::sync::Arc;

    struct MockHandler {
        engine: Arc<ConsensusEngine>,
    }

    #[async_trait::async_trait]
    impl ConsensusHandler for MockHandler {
        async fn handle_request_vote(
            &self,
            request: crate::consensus::transport::RequestVoteRequest,
        ) -> crate::error::ConsensusResult<crate::consensus::transport::RequestVoteResponse> {
            self.engine.handle_request_vote(request).await
        }

        async fn handle_append_entries(
            &self,
            request: crate::consensus::transport::AppendEntriesRequest,
        ) -> crate::error::ConsensusResult<crate::consensus::transport::AppendEntriesResponse> {
            self.engine.handle_append_entries(request).await
        }
    }

    #[tokio::test]
    async fn test_consensus_engine_creation() {
        let node_id = NodeId::new("node-1");
        let storage = Arc::new(RaftStorage::new());
        let transport = Arc::new(InMemoryTransport::new());
        let config = RaftConfig::default();

        let engine = ConsensusEngine::new(node_id.clone(), config, storage, transport);
        // Verify engine was created (node_id is private, so we can't directly assert it)
        // The fact that it doesn't panic is sufficient for this test
    }

    #[tokio::test]
    async fn test_consensus_engine_start_stop() {
        let node_id = NodeId::new("node-1");
        let storage = Arc::new(RaftStorage::new());
        let transport = Arc::new(InMemoryTransport::new());
        let config = RaftConfig {
            cluster_nodes: vec![node_id.clone()],
            ..Default::default()
        };

        let engine = ConsensusEngine::new(node_id, config, storage, transport);
        engine.start().await.unwrap();

        // Give it a moment to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_propose_only_works_as_leader() {
        let node_id = NodeId::new("node-1");
        let storage = Arc::new(RaftStorage::new());
        let transport = Arc::new(InMemoryTransport::new());
        let config = RaftConfig {
            cluster_nodes: vec![node_id.clone()],
            ..Default::default()
        };

        let engine = ConsensusEngine::new(node_id, config, storage, transport);
        
        // Try to propose as follower (should fail)
        let command = StateMachineCommand::RegisterAgent {
            agent_id: "agent-1".to_string(),
            metadata: HashMap::new(),
        };
        
        let result = engine.propose(command).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::ConsensusError::NotLeader(_)
        ));
    }
}
