// Example: Verifies message-bus lock safety (no lock held across await)
// Run with: cargo run --example bus_lock_safety

use mofa_runtime::{SimpleMessageBus, AgentEvent};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let bus = Arc::new(SimpleMessageBus::new());
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    bus.register("receiver", tx.clone()).await;

    let lock = Arc::new(RwLock::new(0));

    // Writer task: acquires lock, increments value, releases lock, then sends message
    let bus_writer = bus.clone();
    let lock_writer = lock.clone();
    let writer = tokio::spawn(async move {
        let mut guard = lock_writer.write().await;
        *guard += 1;
        println!("Writer acquired lock, incremented value: {}", *guard);
        drop(guard); // Drop lock before await
        bus_writer.send_to("receiver", AgentEvent::Custom("test message".to_string(), Vec::new())).await.unwrap();
        println!("Writer sent message after releasing lock");
    });

    // Reader task: waits, acquires lock, reads value, releases lock, then receives message
    let lock_reader = lock.clone();
    let reader = tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        let guard = lock_reader.read().await;
        println!("Reader acquired lock, value: {}", *guard);
        drop(guard); // Drop lock before await
        if let Some(event) = rx.recv().await {
            match event {
                AgentEvent::Custom(msg, _) => println!("Reader received message after releasing lock: {}", msg),
                _ => println!("Reader received unexpected event"),
            }
        }
    });

    let _ = tokio::join!(writer, reader);
    println!("Bus lock safety example completed.");
}
