use mofa_sdk::kernel::AgentEvent;
use mofa_sdk::runtime::{AgentBuilder, SimpleRuntime};
use tokio::time::{Duration, sleep, timeout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = SimpleRuntime::new();

    let slow_builder = AgentBuilder::new("slow-agent", "SlowAgent");
    let slow_meta = slow_builder.build_metadata();
    let slow_cfg = slow_builder.build_config();
    let mut slow_rx = runtime
        .register_agent(slow_meta, slow_cfg, "worker")
        .await?;
    runtime.subscribe_topic("slow-agent", "topic-a").await?;

    let bus = runtime.message_bus().clone();

    println!("Scenario 1: register remains responsive while send_to is backpressured");

    // Fill the slow receiver queue (capacity = 1 in SimpleRuntime::register_agent).
    runtime
        .send_to_agent(
            "slow-agent",
            AgentEvent::Custom("warmup".to_string(), vec![]),
        )
        .await?;

    let send_task = tokio::spawn({
        let bus = bus.clone();
        async move {
            bus.send_to(
                "slow-agent",
                AgentEvent::Custom("blocked-send".to_string(), vec![]),
            )
            .await
        }
    });

    sleep(Duration::from_millis(50)).await;

    let observer_builder = AgentBuilder::new("observer-agent", "ObserverAgent");
    let observer_meta = observer_builder.build_metadata();
    let observer_cfg = observer_builder.build_config();
    timeout(
        Duration::from_millis(300),
        runtime.register_agent(observer_meta, observer_cfg, "observer"),
    )
    .await??;
    println!("  register_agent completed quickly while send_to was pending");

    // Drain one event to unblock the pending send task.
    let _ = slow_rx.recv().await;
    send_task.await??;
    let _ = slow_rx.recv().await;

    println!("Scenario 2: subscribe remains responsive while publish is backpressured");

    runtime
        .send_to_agent(
            "slow-agent",
            AgentEvent::Custom("warmup-2".to_string(), vec![]),
        )
        .await?;

    let publish_task = tokio::spawn({
        let bus = bus.clone();
        async move {
            bus.publish(
                "topic-a",
                AgentEvent::Custom("blocked-publish".to_string(), vec![]),
            )
            .await
        }
    });

    sleep(Duration::from_millis(50)).await;

    timeout(
        Duration::from_millis(300),
        runtime.subscribe_topic("observer-agent", "topic-b"),
    )
    .await??;
    println!("  subscribe_topic completed quickly while publish was pending");

    // Drain one event to unblock publish and consume the publish message.
    let _ = slow_rx.recv().await;
    publish_task.await??;
    let _ = slow_rx.recv().await;

    println!("Done: message bus remained responsive under backpressure.");
    Ok(())
}
