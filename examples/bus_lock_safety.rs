// Example: Verifies message-bus lock safety (no lock held across await)
// Run with: cargo run --example bus_lock_safety

use mofa_runtime::message_bus::MessageBus;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let bus = Arc::new(MessageBus::new());
    let lock = Arc::new(RwLock::new(0));

    // Simulate concurrent read/write with lock and await
    let bus_clone = bus.clone();
    let lock_clone = lock.clone();
    let writer = tokio::spawn(async move {
        let mut guard = lock_clone.write().await;
        *guard += 1;
        println!("Writer acquired lock, incremented value: {}", *guard);
        drop(guard); // Drop lock before await
        bus_clone.send("test").await;
        println!("Writer sent message after releasing lock");
    });

    let bus_clone2 = bus.clone();
    let lock_clone2 = lock.clone();
    let reader = tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        let guard = lock_clone2.read().await;
        println!("Reader acquired lock, value: {}", *guard);
        drop(guard); // Drop lock before await
        let msg = bus_clone2.recv().await;
        println!("Reader received message after releasing lock: {}", msg);
    });

    let _ = tokio::join!(writer, reader);
    println!("Bus lock safety example completed.");
}
