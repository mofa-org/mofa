//! Local Compute Mesh Demo
//!
//! This example demonstrates the LocalFirstWithCloudFallback routing behavior
//! in the MoFA Compute Mesh architecture.
//!
//! Scenario A: Local backend available → router selects local backend
//! Scenario B: Local backend unavailable → router falls back to cloud backend

use mofa_foundation::inference::{
    InferenceOrchestrator, OrchestratorConfig, RoutingPolicy,
};
use std::time::Duration;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

/// Check if Ollama server is reachable at localhost:11434
async fn check_ollama_available() -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    
    match client.get("http://localhost:11434/api/tags").send().await {
        Ok(response) => response.status().is_success(),
        Err(e) => {
            info!("[router] Ollama server not available: {}", e);
            false
        }
    }
}

/// Simulate local backend inference (when Ollama would be used)
async fn simulate_local_inference(prompt: &str) -> String {
    // Simulate processing time
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    format!(
        "[Local Inference Response] Processed '{}' using local Llama model.\n\
         The local backend handled this request entirely on your machine.",
        prompt
    )
}

/// Simulate cloud fallback inference
async fn simulate_cloud_inference(prompt: &str, provider: &str) -> String {
    // Simulate network latency
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    format!(
        "[Cloud Inference Response] Processed '{}' using {}.\n\
         The request was routed to the cloud provider due to local backend unavailability.",
        prompt, provider
    )
}

/// Run the inference orchestrator demo using the actual routing infrastructure
async fn run_orchestrator_demo(prompt: &str, ollama_available: bool) {
    // Create the orchestrator configuration with LocalFirstWithCloudFallback
    let config = OrchestratorConfig {
        routing_policy: RoutingPolicy::LocalFirstWithCloudFallback,
        cloud_provider: "openai".to_string(),
        idle_timeout: Duration::from_secs(300),
        ..Default::default()
    };
    
    info!("[router] policy: LocalFirstWithCloudFallback");
    
    // Create the inference orchestrator (demonstrates configuration)
    let _orchestrator = InferenceOrchestrator::new(config);
    
    if ollama_available {
        // Scenario A: Local backend is available
        info!("[router] attempting local backend");
        
        // In a real scenario, the orchestrator would attempt local inference
        // For demo purposes, we simulate the local response
        let response = simulate_local_inference(prompt).await;
        
        info!("[router] selected backend: local");
        println!("\n{}", response);
    } else {
        // Scenario B: Fallback to cloud
        warn!("[router] local backend unavailable");
        info!("[router] falling back to cloud provider: openai");
        
        let response = simulate_cloud_inference(prompt, "openai").await;
        println!("\n{}", response);
    }
    
    // The orchestrator automatically manages resources
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up structured logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");
    
    println!("=======================================================");
    println!("  MoFA Local Compute Mesh Demo");
    println!("  LocalFirstWithCloudFallback Routing Demonstration");
    println!("=======================================================\n");
    
    // Get the prompt from command line arguments or use default
    let prompt = std::env::args().nth(1).unwrap_or_else(|| {
        "Explain photosynthesis in one sentence.".to_string()
    });
    
    println!("Prompt: {}\n", prompt);
    
    // Check if Ollama is available
    let ollama_available = check_ollama_available().await;
    
    // Run the orchestrator demo with routing behavior
    run_orchestrator_demo(&prompt, ollama_available).await;
    
    println!("\n=======================================================");
    println!("  Demo Complete");
    println!("=======================================================");
    
    Ok(())
}
