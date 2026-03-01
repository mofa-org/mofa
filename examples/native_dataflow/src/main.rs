//! Native dataflow example — no Dora-rs dependency.
//!
//! This example builds a two-node pipeline:
//!
//!   producer --[out]--> consumer
//!
//! The producer node injects 5 events into the dataflow, the consumer node
//! processes them via its event loop, and both nodes are stopped cleanly.

use mofa_runtime::native_dataflow::{
    DataflowBuilder, NativeRuntime, NodeConfig,
};
use mofa_runtime::AgentEvent;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // --- Build a two-node dataflow --------------------------------------------

    let df = DataflowBuilder::new("demo-pipeline")
        .add_node_config(NodeConfig {
            node_id: "producer".to_string(),
            name: "Producer".to_string(),
            outputs: vec!["events".to_string()],
            ..Default::default()
        })
        .add_node_config(NodeConfig {
            node_id: "consumer".to_string(),
            name: "Consumer".to_string(),
            inputs: vec!["events".to_string()],
            ..Default::default()
        })
        .connect("producer", "events", "consumer", "events")
        .build()
        .await?;

    // --- Register and start via NativeRuntime --------------------------------

    let runtime = NativeRuntime::new();
    let df_id = runtime.register_dataflow(df).await?;
    runtime.start().await?;

    info!("Runtime started, dataflow id = {}", df_id);

    // Retrieve node handles for the producer and consumer.
    let df_handle = runtime.get_dataflow(&df_id).await.unwrap();
    let producer = df_handle.get_node("producer").await.unwrap();
    let consumer = df_handle.get_node("consumer").await.unwrap();

    // --- Spawn consumer event loop -------------------------------------------

    let received_count = Arc::new(AtomicU32::new(0));
    let received_clone = received_count.clone();
    let consumer_el = consumer.create_event_loop();

    let consumer_task = tokio::spawn(async move {
        info!("[consumer] event loop started");
        loop {
            match consumer_el.next_event().await {
                Some(AgentEvent::Shutdown) => {
                    info!("[consumer] shutdown received");
                    break;
                }
                Some(AgentEvent::Custom(port, data)) => {
                    let n = received_clone.fetch_add(1, Ordering::Relaxed) + 1;
                    info!(
                        "[consumer] received event #{} on port '{}' ({} bytes)",
                        n,
                        port,
                        data.len()
                    );
                }
                Some(other) => {
                    info!("[consumer] received event: {:?}", other);
                    received_clone.fetch_add(1, Ordering::Relaxed);
                }
                None => break,
            }
        }
        info!("[consumer] event loop exited");
    });

    // --- Produce 5 messages --------------------------------------------------

    for i in 1u32..=5 {
        let payload = format!("message-{}", i).into_bytes();
        if let Err(e) = producer.send_output("events", payload).await {
            warn!("[producer] send_output failed: {}", e);
        }
        info!("[producer] sent message #{}", i);
        sleep(Duration::from_millis(50)).await;
    }

    // Give the consumer a moment to drain the queue.
    sleep(Duration::from_millis(200)).await;

    // --- Stop the runtime ----------------------------------------------------

    runtime.stop().await?;
    consumer_task.await?;

    let total = received_count.load(Ordering::Relaxed);
    info!("Done — consumer received {} / 5 messages", total);
    assert_eq!(total, 5, "expected 5 messages, got {}", total);

    Ok(())
}
