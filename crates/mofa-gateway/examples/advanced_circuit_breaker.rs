//! Advanced circuit breaker configuration example.
//!
//! This example demonstrates how to configure circuit breakers for
//! fault tolerance and preventing cascading failures.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example advanced_circuit_breaker --package mofa-gateway
//! ```

use mofa_gateway::gateway::CircuitBreakerRegistry;
use mofa_gateway::NodeId;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("Advanced Circuit Breaker Configuration Example");

    // Example 1: Conservative Circuit Breaker (Few Failures Trigger)
    tracing::info!("\n=== Example 1: Conservative Circuit Breaker ===");
    let circuit_breaker_conservative = CircuitBreakerRegistry::new(
        3,                                      // Failure threshold: 3 failures
        2,                                      // Success threshold: 2 successes
        Duration::from_secs(30),                // Timeout: 30 seconds
    );
    
    tracing::info!("Configuration: failure_threshold=3, success_threshold=2, timeout=30s");
    tracing::info!("Best for: Critical services, low tolerance for failures");
    tracing::info!("Opens quickly, requires 2 successes to close");

    // Example 2: Aggressive Circuit Breaker (Many Failures Trigger)
    tracing::info!("\n=== Example 2: Aggressive Circuit Breaker ===");
    let circuit_breaker_aggressive = CircuitBreakerRegistry::new(
        10,                                     // Failure threshold: 10 failures
        5,                                      // Success threshold: 5 successes
        Duration::from_secs(60),               // Timeout: 60 seconds
    );
    
    tracing::info!("Configuration: failure_threshold=10, success_threshold=5, timeout=60s");
    tracing::info!("Best for: Resilient services, temporary failures expected");
    tracing::info!("Opens slowly, requires 5 successes to close");

    // Example 3: Fast Recovery Circuit Breaker
    tracing::info!("\n=== Example 3: Fast Recovery Circuit Breaker ===");
    let circuit_breaker_fast = CircuitBreakerRegistry::new(
        5,                                      // Failure threshold: 5 failures
        1,                                      // Success threshold: 1 success
        Duration::from_secs(10),               // Timeout: 10 seconds
    );
    
    tracing::info!("Configuration: failure_threshold=5, success_threshold=1, timeout=10s");
    tracing::info!("Best for: Services that recover quickly");
    tracing::info!("Opens after 5 failures, closes after 1 success");

    // Demonstrate circuit breaker behavior
    tracing::info!("\n=== Circuit Breaker Behavior Simulation ===");
    let node_id = NodeId::new("test-node");
    let breaker = circuit_breaker_conservative.get_or_create(&node_id).await;
    
    tracing::info!("Initial state: {:?}", breaker.state().await);
    
    // Simulate failures
    tracing::info!("\nSimulating failures...");
    for i in 0..5 {
        let _ = breaker.record_failure().await;
        tracing::info!("Failure {}: State = {:?}", i + 1, breaker.state().await);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Simulate recovery attempts
    tracing::info!("\nSimulating recovery attempts...");
    for i in 0..3 {
        let _ = breaker.record_success().await;
        tracing::info!("Success {}: State = {:?}", i + 1, breaker.state().await);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tracing::info!("\n=== Circuit Breaker Best Practices ===");
    tracing::info!("1. Set failure threshold based on acceptable error rate");
    tracing::info!("2. Use shorter timeouts for fast-recovering services");
    tracing::info!("3. Monitor circuit breaker state transitions");
    tracing::info!("4. Alert on frequent circuit breaker opens");
    tracing::info!("5. Consider different thresholds for different endpoints");
    tracing::info!("6. Use half-open state to test recovery");

    tracing::info!("\nExample completed successfully!");
    Ok(())
}
