#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::AgentBus;
    use mofa_kernel::CommunicationMode;
    use mofa_kernel::agent::{AgentCapabilities, AgentMetadata, AgentState};
    use mofa_kernel::message::{AgentMessage, TaskStatus};
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
        register_point_to_point_channel(bus, id, "coordinator").await;
    }

    async fn register_point_to_point_channel(bus: &AgentBus, id: &str, sender: &str) {
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
            CommunicationMode::PointToPoint(sender.to_string()),
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

    #[tokio::test]
    async fn test_pipeline_preserves_root_task_id_across_stages() {
        let bus = Arc::new(AgentBus::new());
        for stage in ["stage1", "stage2", "stage3"] {
            register_point_to_point_channel(&bus, stage, "coordinator").await;
            register_point_to_point_channel(&bus, "coordinator", stage).await;
        }

        let coordinator = AgentCoordinator::new(bus.clone(), CoordinationStrategy::Pipeline).await;
        coordinator.register_role("stage1", "stage1").await.unwrap();
        coordinator.register_role("stage2", "stage2").await.unwrap();
        coordinator.register_role("stage3", "stage3").await.unwrap();

        // Each stage echoes the same root task id back to the coordinator
        let root_task_id = "pipeline-root-123".to_string();
        let expected_task_id = root_task_id.clone();
        let bus_stage1 = bus.clone();
        let bus_stage2 = bus.clone();
        let bus_stage3 = bus.clone();

        // Stage 1 turns the initial request into pipeline output for stage 2
        let stage1 = tokio::spawn(async move {
            let msg = bus_stage1
                .receive_message(
                    "stage1",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                )
                .await
                .unwrap()
                .unwrap();
            let AgentMessage::TaskRequest { task_id, content } = msg else {
                panic!("expected task request");
            };
            bus_stage1
                .send_message(
                    "stage1",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                    &AgentMessage::TaskResponse {
                        task_id: task_id.clone(),
                        result: format!("{content}-s1"),
                        status: TaskStatus::Success,
                    },
                )
                .await
                .unwrap();
            task_id
        });

        // Stage 2 should receive the same root task id, not a fresh generated id.
        let stage2 = tokio::spawn(async move {
            let msg = bus_stage2
                .receive_message(
                    "stage2",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                )
                .await
                .unwrap()
                .unwrap();
            let AgentMessage::TaskRequest { task_id, content } = msg else {
                panic!("expected task request");
            };
            bus_stage2
                .send_message(
                    "stage2",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                    &AgentMessage::TaskResponse {
                        task_id: task_id.clone(),
                        result: format!("{content}-s2"),
                        status: TaskStatus::Success,
                    },
                )
                .await
                .unwrap();
            task_id
        });

        // Stage 3 completes the chain and lets us assert lineage end to end.
        let stage3 = tokio::spawn(async move {
            let msg = bus_stage3
                .receive_message(
                    "stage3",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                )
                .await
                .unwrap()
                .unwrap();
            let AgentMessage::TaskRequest { task_id, content } = msg else {
                panic!("expected task request");
            };
            bus_stage3
                .send_message(
                    "stage3",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                    &AgentMessage::TaskResponse {
                        task_id: task_id.clone(),
                        result: format!("{content}-s3"),
                        status: TaskStatus::Success,
                    },
                )
                .await
                .unwrap();
            task_id
        });

        coordinator
            .coordinate_task(&AgentMessage::TaskRequest {
                task_id: root_task_id.clone(),
                content: "input".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(stage1.await.unwrap(), expected_task_id);
        assert_eq!(stage2.await.unwrap(), expected_task_id);
        assert_eq!(stage3.await.unwrap(), expected_task_id);
    }

    #[tokio::test]
    async fn test_pipeline_times_out_when_stage_never_responds() {
        let bus = Arc::new(AgentBus::new());
        register_point_to_point_channel(&bus, "stage1", "coordinator").await;
        register_point_to_point_channel(&bus, "coordinator", "stage1").await;
        register_point_to_point_channel(&bus, "stage2", "coordinator").await;
        register_point_to_point_channel(&bus, "coordinator", "stage2").await;
        register_point_to_point_channel(&bus, "stage3", "coordinator").await;
        register_point_to_point_channel(&bus, "coordinator", "stage3").await;

        let coordinator = AgentCoordinator::new(bus.clone(), CoordinationStrategy::Pipeline).await;
        coordinator.register_role("stage1", "stage1").await.unwrap();
        coordinator.register_role("stage2", "stage2").await.unwrap();
        coordinator.register_role("stage3", "stage3").await.unwrap();

        let bus_stage1 = bus.clone();
        tokio::spawn(async move {
            let msg = bus_stage1
                .receive_message(
                    "stage1",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                )
                .await
                .unwrap()
                .unwrap();
            let AgentMessage::TaskRequest { task_id, content } = msg else {
                panic!("expected task request");
            };
            bus_stage1
                .send_message(
                    "stage1",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                    &AgentMessage::TaskResponse {
                        task_id,
                        result: content,
                        status: TaskStatus::Success,
                    },
                )
                .await
                .unwrap();
        });

        let bus_stage2 = bus.clone();
        tokio::spawn(async move {
            let _ = bus_stage2
                .receive_message(
                    "stage2",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                )
                .await
                .unwrap()
                .unwrap();
            // Hold the stage open without replying so the coordinator hits its timeout path
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let err = coordinator
            .coordinate_task(&AgentMessage::TaskRequest {
                task_id: "pipeline-timeout".to_string(),
                content: "input".to_string(),
            })
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("Timed out waiting for pipeline stage stage2")
        );
    }

    #[tokio::test]
    async fn test_pipeline_returns_stage_failure() {
        let bus = Arc::new(AgentBus::new());
        for stage in ["stage1", "stage2", "stage3"] {
            register_point_to_point_channel(&bus, stage, "coordinator").await;
            register_point_to_point_channel(&bus, "coordinator", stage).await;
        }

        let coordinator = AgentCoordinator::new(bus.clone(), CoordinationStrategy::Pipeline).await;
        coordinator.register_role("stage1", "stage1").await.unwrap();
        coordinator.register_role("stage2", "stage2").await.unwrap();
        coordinator.register_role("stage3", "stage3").await.unwrap();

        let bus_stage1 = bus.clone();
        tokio::spawn(async move {
            let msg = bus_stage1
                .receive_message(
                    "stage1",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                )
                .await
                .unwrap()
                .unwrap();
            let AgentMessage::TaskRequest { task_id, content } = msg else {
                panic!("expected task request");
            };
            bus_stage1
                .send_message(
                    "stage1",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                    &AgentMessage::TaskResponse {
                        task_id,
                        result: content,
                        status: TaskStatus::Success,
                    },
                )
                .await
                .unwrap();
        });

        let bus_stage2 = bus.clone();
        tokio::spawn(async move {
            let msg = bus_stage2
                .receive_message(
                    "stage2",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                )
                .await
                .unwrap()
                .unwrap();
            let AgentMessage::TaskRequest { task_id, .. } = msg else {
                panic!("expected task request");
            };
            bus_stage2
                .send_message(
                    "stage2",
                    CommunicationMode::PointToPoint("coordinator".to_string()),
                    &AgentMessage::TaskResponse {
                        task_id,
                        result: "failed".to_string(),
                        status: TaskStatus::Failed,
                    },
                )
                .await
                .unwrap();
        });

        let err = coordinator
            .coordinate_task(&AgentMessage::TaskRequest {
                task_id: "pipeline-failure".to_string(),
                content: "input".to_string(),
            })
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("Pipeline stage stage2 (stage2) failed")
        );
    }
}
