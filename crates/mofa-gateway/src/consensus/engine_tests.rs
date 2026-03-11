//! Tests for the Raft consensus engine.

#[cfg(test)]
mod tests {
    use crate::consensus::engine::{ConsensusEngine, RaftConfig};
    use crate::consensus::storage::RaftStorage;
    use crate::consensus::transport_impl::{ConsensusHandler, InMemoryTransport};
    use crate::consensus::transport::{
        AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
    };
    use crate::consensus::RaftTransport;
    use crate::consensus::state::LeaderState;
    use crate::types::{LogEntry, LogIndex, NodeId, RaftState, StateMachineCommand, Term};
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

    #[tokio::test]
    async fn test_raft_heartbeat_prev_log_index_regression() {
        let node_id = NodeId::new("leader");
        let follower_id = NodeId::new("follower");
        let transport = Arc::new(InMemoryTransport::new());
        let storage = Arc::new(RaftStorage::new());
        let config = RaftConfig {
            cluster_nodes: vec![node_id.clone(), follower_id.clone()],
            ..Default::default()
        };

        let leader_engine = Arc::new(ConsensusEngine::new(
            node_id.clone(),
            config.clone(),
            storage.clone(),
            transport.clone(),
        ));

        // 1. Manually setup leader state with some log entries
        {
            let mut s = leader_engine.state.write().await;
            s.state = RaftState::Leader;
            s.current_term = Term::new(1);
            // Add 10 entries to leader log
            for i in 1..=10 {
                s.log.push(LogEntry {
                    term: Term::new(1),
                    index: LogIndex::new(i),
                    data: vec![0],
                });
            }
        }

        // 2. Setup leader_state (next_index/match_index)
        // Follower is behind: next_index = 5 (it has up to index 4)
        {
            let mut ls = leader_engine.leader_state.write().await;
            let mut leader_state = LeaderState::new(&[follower_id.clone()], LogIndex::new(10));
            leader_state
                .next_index
                .insert(follower_id.clone(), LogIndex::new(5));
            *ls = Some(leader_state);
        }

        // 3. Setup a follower engine to receive the heartbeat
        let follower_storage = Arc::new(RaftStorage::new());
        let follower_engine = Arc::new(ConsensusEngine::new(
            follower_id.clone(),
            config,
            follower_storage,
            transport.clone(),
        ));

        // Follower has entries 1-4
        {
            let mut s = follower_engine.state.write().await;
            s.current_term = Term::new(1);
            for i in 1..=4 {
                s.log.push(LogEntry {
                    term: Term::new(1),
                    index: LogIndex::new(i),
                    data: vec![0],
                });
            }
        }

        // Register follower in transport so it can receive RPCs
        transport
            .register_handler(
                follower_id.clone(),
                Arc::new(MockHandler {
                    engine: Arc::clone(&follower_engine),
                }),
            )
            .await;

        // 4. Trigger heartbeat
        ConsensusEngine::send_heartbeats(
            &node_id,
            &leader_engine.state,
            &leader_engine.leader_state,
            &leader_engine.transport,
            &[node_id.clone(), follower_id.clone()],
        )
        .await;

        // Give it a moment to process the async RPC
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 5. Verification

        // Follower should have received a heartbeat and accepted it
        let last_hb = *follower_engine.last_heartbeat.read().await;
        assert!(
            last_hb.is_some(),
            "Follower should have received a heartbeat"
        );

        // Check leader side: Since follower accepted it, match_index should be updated
        // to the follower's last log index (which is 4)
        let ls = leader_engine.leader_state.read().await;
        let ls_ref = ls.as_ref().unwrap();
        let m_idx = ls_ref.match_index.get(&follower_id).unwrap();
        assert_eq!(
            m_idx.0, 4,
            "Follower match_index should be 4 after heartbeat"
        );
    }
}
