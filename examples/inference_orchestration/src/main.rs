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
    RoutedBackend, RoutingPolicy,
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

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Use Case 6: Cloud Fallback Retry
    {
        info!("--- Use Case 6: Cloud Fallback Retry ---");
        // The orchestrator is configured with cloud_retry_attempts = 2 by default,
        // meaning up to 3 total cloud calls before giving up.
        //
        // In Phase 1 the cloud backend is simulated, so we demonstrate the config.
        // In Phase 2, when a real LLM client is wired in, transient HTTP 503s or
        // rate-limit errors will automatically be retried up to this many times.
        let mut retry_config = config.clone();
        retry_config.cloud_retry_attempts = 2;   // up to 2 retries (3 total attempts)
        retry_config.cloud_retry_delay_ms = 100; // 100 ms between retries in production
        let mut orch = InferenceOrchestrator::new(retry_config.clone());

        // Fill memory so the request falls back to cloud.
        orch.infer(&InferenceRequest::new("fill-model", "warm up", 12_000));

        let req = InferenceRequest::new("gpt-4o", "What is 2+2?", 4_000);
        let result = orch.infer(&req);
        info!(
            "Cloud result (attempt #{}): {:?}  [config: max_retries={}]",
            result.cloud_attempt_count,
            result.routed_to,
            retry_config.cloud_retry_attempts,
        );
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Use Case 7: Global Policy Downgrade Switch
    {
        info!("--- Use Case 7: Global Policy Downgrade Switch (force_cloud) ---");
        // `set_force_cloud(true)` is the \"break-glass\" mechanism for local outages.
        // It bypasses routing policy AND admission control — every request goes to
        // cloud until the switch is cleared, regardless of available memory.
        let mut orch = InferenceOrchestrator::new(config.clone());
        info!("Before force_cloud: routing_policy = {}", orch.routing_policy());

        // Without the switch: a small request fits locally.
        let req = InferenceRequest::new("llama-3-8b", "Hello!", 4_096);
        let normal = orch.infer(&req);
        info!("Normal routing: {:?}", normal.routed_to);

        // Activate the global downgrade switch.
        orch.set_force_cloud(true);
        warn!(
            "⚠  force_cloud activated — all traffic redirected to cloud (is_force_cloud={})",
            orch.is_force_cloud()
        );

        let forced = orch.infer(&InferenceRequest::new("llama-3-8b", "Hello again!", 4_096));
        info!("Forced routing: {:?}  (local model count={})", forced.routed_to, orch.loaded_model_count());
        assert!(
            matches!(forced.routed_to, RoutedBackend::Cloud { .. }),
            "force_cloud must override LocalFirst policy"
        );

        // Restore normal routing.
        orch.set_force_cloud(false);
        info!("force_cloud cleared — normal routing restored (is_force_cloud={})", orch.is_force_cloud());
    }

    info!("Demonstration complete.");

    Ok(())
}
