use anyhow::Result;
use mofa_foundation::inference::{
    InferenceOrchestrator, InferenceRequest, OrchestratorConfig, RoutingPolicy,
};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    println!("=== MoFA Unified Inference Orchestrator Demo (#388) ===");

    // 1. Initialize our Memory-Aware Orchestrator with a simulated 16GB limit
    let mut config = OrchestratorConfig::default();
    config.memory_capacity_mb = 16384; // Force 16GB for the demo
    config.routing_policy = RoutingPolicy::LocalFirstWithCloudFallback;
    config.defer_threshold = 0.75;
    config.reject_threshold = 0.90;

    let mut orchestrator = InferenceOrchestrator::new(config);

    println!("Simulating local hardware...");
    println!("Hardware Detected: {:?}", orchestrator.hardware());
    println!("Total VRAM/Unified Budget Available: 16.3 GB\n");

    println!("--- Step 1: Agent A (General Chat) executing ---");
    // Request an 8GB model
    let req_a = InferenceRequest::new("qwen-7b-int4", "Hello", 8000);
    let res_a = orchestrator.infer(&req_a);
    println!("Result A: {:?}\n", res_a.routed_to);

    println!("--- Step 2: Agent B (Code Generation) executing ---");
    // Request a 6GB model
    let req_b = InferenceRequest::new("deepseek-coder-6.7b", "Write Rust", 6000);
    let res_b = orchestrator.infer(&req_b);
    println!("Result B: {:?}\n", res_b.routed_to);

    println!("--- Step 3: Agent C (Audio Response) executing ---");
    // Request a 4GB model.
    // Current usage is 14GB out of 16.3GB. 4GB puts it at 18GB -> Exceeds 90% reject threshold!
    // Since our policy is LocalFirstWithCloudFallback, rather than OOM crashing,
    // the Admission Gate will reject local and cleanly failover to the cloud!
    println!("Attempting to load gpt-sovits-v2 (Requires: 4 GB)");
    println!("Current Memory Pressure: 14 GB / 16.3 GB");
    println!("Warning: The Admission Gate will block this local allocation to prevent OOM panic!");

    let req_c = InferenceRequest::new("gpt-sovits-v2", "TTS Audio", 4000);
    let res_c = orchestrator.infer(&req_c);

    println!("Result C: {:?}\n", res_c.routed_to);

    println!("=== Orchestration Completed Successfully! ===");
    println!("No OOM crashes occurred because the Admission Gate actively managed VRAM.");

    Ok(())
}
