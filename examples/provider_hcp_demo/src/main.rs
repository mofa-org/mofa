//! This example demonstrates the Health Check Protocol (HCP) introduced 
//! in the `mofa-kernel`'s `llm` module. It shows how:
//! - You can monitor an `LLMProvider` dynamically with a `PeriodicHealthChecker`.
//! - Events are emitted only on transition boundaries (e.g. Unknown -> Healthy -> Unhealthy).
//! - Custom implementations can dictate their health status mapping.
//!
//! Run with: `cargo run --example provider_hcp_demo`
//! (if testing inside the crate directly, simply `cargo run` inside `examples/provider_hcp_demo`)

use async_trait::async_trait;
use mofa_kernel::agent::AgentResult;
use mofa_kernel::llm::hcp::{HealthStatus, PeriodicHealthChecker};
use mofa_kernel::llm::provider::LLMProvider;
use mofa_kernel::llm::types::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// A mock LLM provider that simulates real-world flaky backend connections.
/// For the first few checks, it is healthy.
/// Then it simulates a rate-limit or network failure (returning `false` to `health_check`).
/// Finally, it recovers.
#[derive(Clone)]
struct FlakyProvider {
    checks_performed: Arc<AtomicUsize>,
}

impl FlakyProvider {
    fn new() -> Self {
        Self {
            checks_performed: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl LLMProvider for FlakyProvider {
    fn name(&self) -> &str {
        "FlakyMockAI"
    }

    async fn chat(&self, _r: ChatCompletionRequest) -> AgentResult<ChatCompletionResponse> {
        unimplemented!("Not used in this demo")
    }

    async fn embedding(&self, _r: EmbeddingRequest) -> AgentResult<EmbeddingResponse> {
        unimplemented!("Not used in this demo")
    }

    /// We simulate our health status evolving over time.
    /// Check 1-2: True (Healthy)
    /// Check 3-4: False (Degraded / Unhealthy simulation)
    /// Check 5+: True (Recovered)
    async fn health_check(&self) -> AgentResult<bool> {
        let count = self.checks_performed.fetch_add(1, Ordering::SeqCst);
        
        info!("--- Backend Provider received health check ping #{} ---", count + 1);

        if count < 2 {
            Ok(true)
        } else if count < 5 {
            // Returning Ok(false) maps into `Degraded` in the blanket implementation,
            // or we could return an Err to simulate `Unhealthy`. 
            // In the kernel provider.rs, `Ok(false)` -> `Degraded`, `Err(_)` -> `Unhealthy`.
            if count == 3 {
                Err(mofa_kernel::agent::AgentError::IoError("Network connection reset".into()))
            } else {
                Ok(false)
            }
        } else {
            Ok(true)
        }
    }

    async fn get_model_info(&self, _model: &str) -> AgentResult<mofa_kernel::llm::provider::ModelInfo> {
        unimplemented!("Not used in this demo")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Health Check Protocol (HCP) demonstration...");

    let provider = FlakyProvider::new();

    // The kernel provides a blanket implementation of `HealthProbe` 
    // for anything implementing `LLMProvider`. We box the provider as a `HealthProbe` directly.
    let probe = Box::new(provider) as Box<dyn mofa_kernel::llm::hcp::HealthProbe>;

    let interval = Duration::from_millis(500);
    let timeout = Duration::from_millis(100);

    // Initialize the checker.
    let checker = PeriodicHealthChecker::new(probe, interval, timeout);
    
    // Starting the checker spawns an indefinite background task and returns a Receiver
    // to listen for state transition events.
    let mut event_receiver = checker.start();

    // We loop to catch exactly 4 transitions for our demonstration:
    // 1. Unknown -> Healthy
    // 2. Healthy -> Degraded  (Count == 2)
    // 3. Degraded -> Unhealthy (Count == 3 via Error injection)
    // 4. Unhealthy -> Healthy (Count == 5 via Recovery)
    let mut transitions_caught = 0;
    while let Some(event) = event_receiver.recv().await {
        
        let msg = format!(
            "Health State Transitioned: {:?} -> {:?}",
            event.previous, event.current
        );

        match event.current {
            HealthStatus::Healthy => info!("✅ {}", msg),
            HealthStatus::Degraded => warn!("⚠️ {}", msg),
            HealthStatus::Unhealthy => warn!("❌ {}", msg),
            HealthStatus::Unknown => info!("❓ {}", msg),
            _ => warn!("Other health status: {}", msg),
        }

        transitions_caught += 1;
        if transitions_caught == 4 {
            info!("Caught all expected health transition events. Shutting down.");
            break;
        }
    }

    // By dropping the receiver, the background `PeriodicHealthChecker` task will cleanly abort 
    // the next time it tries to send on the channel.
    drop(event_receiver);
    
    tokio::time::sleep(Duration::from_millis(800)).await;
    info!("Demonstration complete.");

    Ok(())
}
