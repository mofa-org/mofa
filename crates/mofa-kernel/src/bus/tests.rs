#[cfg(test)]
mod tests {
    use crate::bus::config::{BackpressureStrategy, EventBusConfig};
    use crate::bus::{AgentBus, CommunicationMode};
    use crate::message::{AgentEvent, AgentMessage, MessagePriority, StreamControlCommand, TaskPriority, TaskStatus};
    use crate::agent::AgentMetadata;
    use crate::agent::types::AgentState;
    use std::time::Duration;
    use tokio::time::timeout;

    // Helper functions
    fn dummy_metadata(id: &str) -> AgentMetadata {
        AgentMetadata {
            id: id.to_string(),
            name: format!("{} name", id),
            description: Some("test".to_string()),
            version: None,
            capabilities: Default::default(),
            state: AgentState::Ready,
        }
    }

    fn critical_msg() -> AgentMessage {
        AgentMessage::Event(AgentEvent::Shutdown)
    }

    fn high_msg() -> AgentMessage {
        AgentMessage::StreamControl {
            stream_id: "test".into(),
            command: StreamControlCommand::Pause,
            metadata: Default::default(),
        }
    }

    fn low_msg() -> AgentMessage {
        AgentMessage::StreamMessage {
            stream_id: "test".into(),
            message: vec![1, 2, 3],
            sequence: 0,
        }
    }

    #[tokio::test]
    async fn test_metrics_increment_decrement() {
        let config = EventBusConfig::new(2, BackpressureStrategy::DropOldest)
            .with_topic("test_topic".to_string(), 2, BackpressureStrategy::DropOldest);
        let bus = AgentBus::new_with_config(config).await.unwrap();

        let receiver_id = "agent_rx";
        let sender_id = "agent_tx";
        let rx_mode = CommunicationMode::PointToPoint(sender_id.to_string());
        let tx_mode = CommunicationMode::PointToPoint(receiver_id.to_string());
        
        bus.register_channel(&dummy_metadata(receiver_id), rx_mode.clone()).await.unwrap();
        
        let m = bus.metrics();
        assert_eq!(m.get_buffer_utilization(), 0);
        
        // Send a message
        bus.send_message(sender_id, tx_mode.clone(), &high_msg()).await.unwrap();
        assert_eq!(m.get_buffer_utilization(), 1);
        
        // Receive the message
        let rx_msg = bus.receive_message(receiver_id, rx_mode.clone()).await.unwrap();
        assert!(rx_msg.is_some());
        assert_eq!(m.get_buffer_utilization(), 0);
    }

    #[tokio::test]
    async fn test_drop_oldest_strategy() {
        let config = EventBusConfig::new(2, BackpressureStrategy::DropOldest)
            .with_topic("test_topic".to_string(), 2, BackpressureStrategy::DropOldest);
        let bus = AgentBus::new_with_config(config).await.unwrap();

        let receiver_id = "agent_rx";
        let sender_id = "agent_tx";
        let rx_mode = CommunicationMode::PointToPoint(sender_id.to_string());
        let tx_mode = CommunicationMode::PointToPoint(receiver_id.to_string());
        
        bus.register_channel(&dummy_metadata(receiver_id), rx_mode.clone()).await.unwrap();
        
        // Send 3 messages to a queue of capacity 2
        bus.send_message(sender_id, tx_mode.clone(), &low_msg()).await.unwrap(); // M1
        bus.send_message(sender_id, tx_mode.clone(), &high_msg()).await.unwrap(); // M2
        bus.send_message(sender_id, tx_mode.clone(), &critical_msg()).await.unwrap(); // M3

        // DropOldest should drop M1. Remainder: M2, M3
        let m = bus.metrics();
        assert_eq!(m.get_dropped_messages(), 1);
        assert_eq!(m.get_buffer_utilization(), 2);
        
        let m1_recv = bus.receive_message(receiver_id, rx_mode.clone()).await.unwrap().unwrap();
        let m2_recv = bus.receive_message(receiver_id, rx_mode.clone()).await.unwrap().unwrap();

        assert_eq!(m1_recv.priority(), MessagePriority::High);
        assert_eq!(m2_recv.priority(), MessagePriority::Critical);
    }

    #[tokio::test]
    async fn test_drop_low_priority_strategy() {
        let config = EventBusConfig::new(2, BackpressureStrategy::DropLowPriority);
        let bus = AgentBus::new_with_config(config).await.unwrap();

        let receiver_id = "agent_rx";
        let sender_id = "agent_tx";
        let rx_mode = CommunicationMode::PointToPoint(sender_id.to_string());
        let tx_mode = CommunicationMode::PointToPoint(receiver_id.to_string());
        
        bus.register_channel(&dummy_metadata(receiver_id), rx_mode.clone()).await.unwrap();
        
        // Fill queue with High priority (Critical=0, High=1, Low=2)
        bus.send_message(sender_id, tx_mode.clone(), &high_msg()).await.unwrap(); // High
        bus.send_message(sender_id, tx_mode.clone(), &high_msg()).await.unwrap(); // High

        // Queue is full. Send a Critical message. It should drop one of the High priority messages to make room.
        bus.send_message(sender_id, tx_mode.clone(), &critical_msg()).await.unwrap();

        let m = bus.metrics();
        assert_eq!(m.get_dropped_messages(), 1);
        assert_eq!(m.get_buffer_utilization(), 2);

        // Send a Low priority message. Since queue is full of High, Critical, it should drop the new message.
        bus.send_message(sender_id, tx_mode.clone(), &low_msg()).await.unwrap();

        assert_eq!(m.get_dropped_messages(), 2); // Dropped the new Low message

        let mut queue_priorities = vec![];
        for _ in 0..2 {
            let msg = bus.receive_message(receiver_id, rx_mode.clone()).await.unwrap().unwrap();
            queue_priorities.push(msg.priority());
        }

        queue_priorities.sort(); // Should be Critical, High
        assert_eq!(queue_priorities, vec![MessagePriority::Critical, MessagePriority::High]);
    }

    #[tokio::test]
    async fn test_block_strategy() {
        let config = EventBusConfig::new(2, BackpressureStrategy::Block);
        let bus = AgentBus::new_with_config(config).await.unwrap();

        let receiver_id = "agent_rx";
        let sender_id = "agent_tx";
        let rx_mode = CommunicationMode::PointToPoint(sender_id.to_string());
        let tx_mode = CommunicationMode::PointToPoint(receiver_id.to_string());
        
        bus.register_channel(&dummy_metadata(receiver_id), rx_mode.clone()).await.unwrap();
        
        // Fill queue
        bus.send_message(sender_id, tx_mode.clone(), &low_msg()).await.unwrap();
        bus.send_message(sender_id, tx_mode.clone(), &low_msg()).await.unwrap();

        let bus_clone = bus.clone();
        let receiver_id_clone = receiver_id.to_string();
        let tx_mode_clone = tx_mode.clone();

        // The 3rd send should block. We use timeout to verify it blocks, and then a background task consumes it.
        let send_fut = async move {
            let msg = high_msg();
            bus_clone.send_message(sender_id, tx_mode_clone, &msg).await
        };

        // It should timeout because queue is full
        let res = timeout(Duration::from_millis(50), send_fut).await;
        assert!(res.is_err(), "Send should have blocked");

        // Now spawn a task to consume one item
        let _ = tokio::spawn({
            let bus = bus.clone();
            let rx_mode = rx_mode.clone();
            let receiver_id = receiver_id.to_string();
            async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                bus.receive_message(&receiver_id, rx_mode).await.unwrap(); // freed 1 permit
            }
        });

        // Now send should succeed
        let msg = high_msg();
        let send_fut = bus.send_message(sender_id, tx_mode.clone(), &msg);
        let res = timeout(Duration::from_millis(200), send_fut).await;
        assert!(res.is_ok(), "Send should unblock once queue has space");
        assert!(res.unwrap().is_ok());

        assert_eq!(bus.metrics().get_dropped_messages(), 0);
    }
}
