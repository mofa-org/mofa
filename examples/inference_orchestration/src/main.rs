//! This example demonstrates the advanced inference orchestration features
//! of the `InferenceOrchestrator`, including:
//! - Local vs Cloud routing policies
//! - Memory-aware admission control (rejecting large requests or deferring under pressure)
//! - Priority-based bypass mechanisms (handling Critical latency requests)
//!
//! Run with: `cargo run --example inference_orchestration`
//! (if testing inside the crate directly, simply `cargo run` inside `examples/inference_orchestration`)

use mofa_foundation::inference::{
    InferenceOrchestrator, InferenceRequest, OrchestratorConfig, RequestPriority,
    RoutingPolicy,
};
use std::time::Duration;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting inference orchestration demonstration...");
    
    // We create a mock configuration with an artificially low memory limit 
    // to easily trigger the defer/reject policies.
    let mut config = OrchestratorConfig::default();
    config.memory_capacity_mb = 16_384; // 16 GB capacity
    config.defer_threshold = 0.75;      // Defer things > 12 GB
    config.reject_threshold = 0.90;     // Reject things > 14.7 GB
    config.routing_policy = RoutingPolicy::LocalFirstWithCloudFallback;

    // Use a local block to enforce dropping of mutable borrows
    {
        // 1. Initialize the orchestrator.
        let mut orchestrator = InferenceOrchestrator::new(config.clone());
        info!("Initialized orchestrator with 16GB capacity and LocalFirst policy.\n");

        // Use Case 1: Simple Local Inference (fits well within memory)
        info!("--- Use Case 1: Simple Local Inference ---");
        // We simulate a 7B model requires 8GB (50% usage) -> Should be Local
        let req1 = InferenceRequest::new("llama-3-8b", "Tell me a joke.", 8_192);
        let res1 = orchestrator.infer(&req1);
        info!("Result: {:?}", res1.routed_to);
        info!("Current memory usage: {:.2}%\n", orchestrator.memory_utilization() * 100.0);

        // Use Case 2: Cloud Fallback under memory constraint
        info!("--- Use Case 2: Cloud Fallback ---");
        // We simulate another 7B model also requiring 8GB -> Total > 16GB. 
        // This will be rejected and fall back to cloud.
        let req2 = InferenceRequest::new("mistral-7b", "Translate this text.", 8_192);
        let res2 = orchestrator.infer(&req2);
        info!("Result: {:?}", res2.routed_to);
        info!("Current memory usage: {:.2}%\n", orchestrator.memory_utilization() * 100.0);
        
        // Use Case 3: Deferred Zone Fallback
        info!("--- Use Case 3: Defer Zone Fallback ---");
        // We unload the first model to free memory up.
        orchestrator.unload_model("llama-3-8b");
        // We load an 11GB model (using 11GB / 16GB = ~68%).
        let req3a = InferenceRequest::new("large-vision-11b", "Analyze this image.", 11_264);
        let res3a = orchestrator.infer(&req3a);
        info!("Loaded 11B model: {:?}", res3a.routed_to);
        
        // We now request a 2GB model (making usage 13.2GB / 16GB = 82%).
        // 82% is in the `[defer_threshold, reject_threshold)` band (75% to 90%).
        // A Normal priority request should be deferred (and under the current policy, fall back to cloud).
        let req3b = InferenceRequest::new("tiny-summarizer-1b", "Summarize short text.", 2_048)
            .with_priority(RequestPriority::Normal);
        let res3b = orchestrator.infer(&req3b);
        info!("Normal priority (in defer band): {:?}", res3b.routed_to);
    } // Drops the first orchestrator

    // We can pause slightly to cleanly separate sections.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // We initialize a new orchestrator to start clean.
    {
        let mut orchestrator = InferenceOrchestrator::new(config.clone());
        info!("Re-initialized orchestrator.\n");

        // Use Case 4: Priority Bypass
        info!("--- Use Case 4: Priority Bypass (Critical requests) ---");
        // Load an 11GB model to reach the defer zone (11GB / 16GB = 68%)
        orchestrator.infer(&InferenceRequest::new("base-11b", "Starting...", 11_264));
        
        // Now submit a request that pushes us to 82% again, but this time it's Critical priority.
        // Critical priority bypasses the defer hysteresis band and admits as long as we are < 90%.
        let req4 = InferenceRequest::new("realtime-voice-2b", "Transcribe this fast!", 2_048)
            .with_priority(RequestPriority::Critical);
        let res4 = orchestrator.infer(&req4);
        // It should be admitted locally, despite being in the defer band!
        info!("Critical priority (in defer band): {:?}", res4.routed_to);
        info!("Current memory usage: {:.2}%\n", orchestrator.memory_utilization() * 100.0);
    } // drops orchestrator

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Use Case 5: Different Routing Policies
    {
        info!("--- Use Case 5: Routing Policy Variations ---");
        let mut local_config = config.clone();
        local_config.routing_policy = RoutingPolicy::LocalOnly;
        let mut orchestrator_local = InferenceOrchestrator::new(local_config);

        // Under LocalOnly, if memory is exceeded, we don't fall back, we just get Rejected.
        orchestrator_local.infer(&InferenceRequest::new("fill", "warmup", 11_264));
        let res_local_only = orchestrator_local.infer(&InferenceRequest::new("huge", "break", 8_192));
        info!("LocalOnly under contention: {:?}", res_local_only.routed_to);

        let mut cost_config = config.clone();
        cost_config.routing_policy = RoutingPolicy::CostOptimized;
        let mut orchestrator_cost = InferenceOrchestrator::new(cost_config);

        // Under CostOptimized, if memory is in the defer zone, we wait (return Local) rather than using Expensive cloud!
        orchestrator_cost.infer(&InferenceRequest::new("fill", "warmup", 11_264));
        let res_cost = orchestrator_cost.infer(&InferenceRequest::new("small", "wait", 2_048));
        info!("CostOptimized under defer contention: {:?}", res_cost.routed_to);
    }

    info!("Demonstration complete.");

    Ok(())
}
