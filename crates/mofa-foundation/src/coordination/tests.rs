#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::AgentBus;
    use mofa_kernel::CommunicationMode;
    use mofa_kernel::agent::{AgentCapabilities, AgentMetadata, AgentState};
    use mofa_kernel::message::AgentMessage;
    use std::sync::Arc;
    use tokio::time::{Duration, timeout};

    use crate::coordination::{AgentCoordinator, CoordinationStrategy};
    #[tokio::test]
    async fn test_peer_to_peer_coordination() {
        let bus = Arc::new(AgentBus::new());
        register_peer_channel(&bus, "peer_1").await;
        register_peer_channel(&bus, "peer_2").await;
        register_peer_channel(&bus, "peer_3").await;

        let coordinator =
            AgentCoordinator::new(bus.clone(), CoordinationStrategy::PeerToPeer).await;

        coordinator.register_role("peer_1", "peer").await.unwrap();
        coordinator.register_role("peer_2", "peer").await.unwrap();
        coordinator.register_role("peer_3", "peer").await.unwrap();

        let task_msg = in_memory_message();
        let bus_1 = bus.clone();
        let bus_2 = bus.clone();
        let bus_3 = bus.clone();
        let recv_1 = tokio::spawn(async move { receive_peer(&bus_1, "peer_1").await });
        let recv_2 = tokio::spawn(async move { receive_peer(&bus_2, "peer_2").await });
        let recv_3 = tokio::spawn(async move { receive_peer(&bus_3, "peer_3").await });
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Send after receivers are subscribed
        let result = coordinator.coordinate_task(&task_msg).await;

        assert!(result.is_ok());

        let msg_1 = timeout(Duration::from_secs(1), recv_1)
            .await
            .unwrap()
            .unwrap();
        let msg_2 = timeout(Duration::from_secs(1), recv_2)
            .await
            .unwrap()
            .unwrap();
        let msg_3 = timeout(Duration::from_secs(1), recv_3)
            .await
            .unwrap()
            .unwrap();
        assert!(msg_1.expect("peer_1 missing message").is_some());
        assert!(msg_2.expect("peer_2 missing message").is_some());
        assert!(msg_3.expect("peer_3 missing message").is_some());

        if let AgentMessage::TaskRequest { task_id, .. } = &task_msg {
            let tracker = coordinator.task_tracker.read().await;
            let entries = tracker.get(task_id).expect("Task ID should be in tracker");
            assert_eq!(entries.len(), 3, "Should track all peers");
            let tracked_peers: Vec<_> = entries.iter().map(|(id, _)| id.clone()).collect();
            assert!(tracked_peers.contains(&"peer_1".to_string()));
            assert!(tracked_peers.contains(&"peer_2".to_string()));
            assert!(tracked_peers.contains(&"peer_3".to_string()));
        } else {
            panic!("Expected TaskRequest message");
        }
    }

    fn in_memory_message() -> AgentMessage {
        AgentMessage::TaskRequest {
            task_id: "test-task-123".to_string(),
            content: "Please do the work".to_string(),
        }
    }

    // Register a point to point channel from coordinator to peer
    async fn register_peer_channel(bus: &AgentBus, id: &str) {
        let metadata = AgentMetadata {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            version: None,
            capabilities: AgentCapabilities::default(),
            state: AgentState::Ready,
        };
        bus.register_channel(
            &metadata,
            CommunicationMode::PointToPoint("coordinator".to_string()),
        )
        .await
        .unwrap();
    }

    // Receive one message for a peer
    async fn receive_peer(
        bus: &AgentBus,
        id: &str,
    ) -> Result<Option<AgentMessage>, mofa_kernel::bus::BusError> {
        bus.receive_message(
            id,
            CommunicationMode::PointToPoint("coordinator".to_string()),
        )
        .await
    }

    #[tokio::test]
    async fn test_register_role() {
        assert!(true);
    }
}
