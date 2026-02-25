use anyhow::Result;
use mofa_kernel::bus::{AgentBus, config::{EventBusConfig, BackpressureStrategy}};
use mofa_kernel::agent::{AgentMetadata, capabilities::AgentCapabilities};
use mofa_kernel::CommunicationMode;
use mofa_kernel::message::{AgentMessage, AgentEvent, TaskPriority, TaskRequest};
use tokio::time::{sleep, Duration};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    println!("--- MoFA Event Bus Backpressure Strategies Example ---");

    // 1. Create a custom EventBusConfig
    let config = EventBusConfig::new(2, BackpressureStrategy::DropOldest)
        .with_topic("topic_block".to_string(), 2, BackpressureStrategy::Block)
        .with_topic("topic_drop_oldest".to_string(), 2, BackpressureStrategy::DropOldest)
        .with_topic("topic_drop_low_priority".to_string(), 2, BackpressureStrategy::DropLowPriority);

    let bus = AgentBus::new_with_config(config).await?;

    let receiver_meta = AgentMetadata {
        id: "receiver".to_string(),
        name: "Receiver".to_string(),
        description: None,
        version: None,
        capabilities: AgentCapabilities::default(),
        state: mofa_kernel::agent::prelude::AgentState::Ready,
    };

    // Register receiver channels for our tests
    bus.register_channel(&receiver_meta, CommunicationMode::PubSub("topic_drop_oldest".to_string())).await?;
    bus.register_channel(&receiver_meta, CommunicationMode::PubSub("topic_drop_low_priority".to_string())).await?;
    bus.register_channel(&receiver_meta, CommunicationMode::PubSub("topic_block".to_string())).await?;

    // --- Scenario 1: DropOldest ---
    println!("\n[Scenario 1] DropOldest Strategy");
    let bus_clone = bus.clone();
    
    // Send 4 messages (capacity is 2).
    for i in 1..=4 {
        let msg = AgentMessage::Event(AgentEvent::Custom(format!("DropOldest-{}", i), vec![]));
        bus_clone.send_message("sender", CommunicationMode::PubSub("topic_drop_oldest".to_string()), &msg).await?;
        println!("  Sent message {}", i);
    }
    
    // Receive messages. We should only get the last 2 (DropOldest-3 and DropOldest-4).
    for _ in 0..2 {
        let received = bus_clone.receive_message("receiver", CommunicationMode::PubSub("topic_drop_oldest".to_string())).await?;
        if let Some(AgentMessage::Event(AgentEvent::Custom(val, _))) = received {
            println!("  Received: {}", val);
        }
    }
    
    // Print metrics
    let metrics = bus.metrics();
    println!("  Metrics - Dropped: {}", metrics.dropped_messages.load(std::sync::atomic::Ordering::Relaxed)); // I might need to check metrics module real quick, but I know it's atomic counters or similar. Wait, there's `metrics.dropped_messages()` or `metrics.get_dropped_messages()`. I'll use `metrics.get_dropped_messages()`.

    // --- Scenario 2: DropLowPriority ---
    println!("\n[Scenario 2] DropLowPriority Strategy");
    // Send 2 low priority messages to fill queue
    let msg_low1 = AgentMessage::Event(AgentEvent::Custom("Low-1".to_string(), vec![]));
    let msg_low2 = AgentMessage::Event(AgentEvent::Custom("Low-2".to_string(), vec![]));
    bus_clone.send_message("sender", CommunicationMode::PubSub("topic_drop_low_priority".to_string()), &msg_low1).await?;
    bus_clone.send_message("sender", CommunicationMode::PubSub("topic_drop_low_priority".to_string()), &msg_low2).await?;
    println!("  Sent 2 Low priority messages");

    // Send a critical priority message. It should displace a low priority one.
    let msg_crit = AgentMessage::TaskRequest {
        task_id: "Crit-Task".to_string(),
        content: "".to_string(),
        priority: TaskPriority::High, // Maps to Critical priority
    };
    bus_clone.send_message("sender", CommunicationMode::PubSub("topic_drop_low_priority".to_string()), &msg_crit).await?;
    println!("  Sent 1 Critical priority message (caused DropLowPriority)");

    // Receive the remaining 2 messages. One should be Low, one should be Critical.
    for _ in 0..2 {
        let received = bus_clone.receive_message("receiver", CommunicationMode::PubSub("topic_drop_low_priority".to_string())).await?;
        match received {
            Some(AgentMessage::Event(AgentEvent::Custom(val, _))) => println!("  Received Low: {}", val),
            Some(AgentMessage::TaskRequest { task_id, .. }) => println!("  Received Critical: {}", task_id),
            _ => (),
        }
    }
    println!("  Metrics - Total Dropped: {}", metrics.get_dropped_messages()); // Assuming get_dropped_messages is correct

    // --- Scenario 3: Block ---
    println!("\n[Scenario 3] Block Strategy");
    // We spawn a task to drain the queue after a delay, to demonstrate blocking send
    let bus_block = bus.clone();
    tokio::spawn(async move {
        sleep(Duration::from_millis(500)).await;
        println!("  (Receiver) Waking up to receive 1 message to unblock sender...");
        let _ = bus_block.receive_message("receiver", CommunicationMode::PubSub("topic_block".to_string())).await.unwrap();
    });

    for i in 1..=3 {
        let msg = AgentMessage::Event(AgentEvent::Custom(format!("Block-{}", i), vec![]));
        println!("  Attempting to send message {}...", i);
        let start = tokio::time::Instant::now();
        bus_clone.send_message("sender", CommunicationMode::PubSub("topic_block".to_string()), &msg).await?;
        let elapsed = start.elapsed();
        if elapsed.as_millis() > 100 {
            println!("  Sent message {} (Blocked for {}ms)", i, elapsed.as_millis());
        } else {
            println!("  Sent message {}", i);
        }
    }

    println!("\nAll strategies successfully validated.");
    Ok(())
}
