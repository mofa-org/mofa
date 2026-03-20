//! Local Compute Mesh Demo
//!
//! Demonstrates a working inference pipeline with streaming from local provider.
//!
//! Run with: cargo run --example local_compute_mesh_demo

use futures::StreamExt;
use mofa_foundation::orchestrator::traits::ModelProvider;
use mofa_local_llm::config::LinuxInferenceConfig;
use mofa_local_llm::provider::LinuxLocalProvider;
use std::time::Instant;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Clone)]
struct PerformanceMetrics {
    latency_ms: u64,
    time_to_first_token_ms: u64,
    tokens_streamed: usize,
    tokens_per_second: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("========================================");
    info!("  MoFA Compute Mesh Demo");
    info!("  with Streaming Inference");
    info!("========================================");
    info!("");

    // Create a simple prompt
    let prompt = "Explain photosynthesis";

    info!("User prompt: {}", prompt);
    info!("");

    // Create local provider with demo config
    let config = LinuxInferenceConfig::new("demo-model", "C:\\temp\\demo-model");

    // Create the provider
    let mut provider = LinuxLocalProvider::new(config)
        .map_err(|e| format!("failed to create provider: {}", e))?;

    // Load the model
    provider.load().await.map_err(|e| format!("failed to load model: {}", e))?;

    info!("Model loaded successfully");

    // Run streaming inference
    let start_time = Instant::now();
    let stream = provider
        .infer_stream(prompt)
        .await
        .map_err(|e| format!("inference error: {}", e))?;

    let first_token_time = start_time.elapsed();
    let mut tokens_streamed = 0;
    let mut collected_output = String::new();

    info!("[stream] starting stream...");

    // Process the stream
    let mut stream = Box::pin(stream);
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                let text = chunk.delta.clone();
                tokens_streamed += 1;
                info!("[stream] {}", text.trim());
                collected_output.push_str(&text);
                if chunk.is_done() {
                    info!("[stream] done");
                }
            }
            Err(e) => {
                tracing::error!("Stream error: {:?}", e);
                break;
            }
        }
    }

    let total_time = start_time.elapsed();
    let latency_ms = total_time.as_millis() as u64;
    let time_to_first_token_ms = first_token_time.as_millis() as u64;
    let tokens_per_second = if latency_ms > 0 {
        (tokens_streamed as f64) / (latency_ms as f64 / 1000.0)
    } else {
        0.0
    };

    let metrics = PerformanceMetrics {
        latency_ms,
        time_to_first_token_ms,
        tokens_streamed,
        tokens_per_second,
    };

    info!("");
    info!("[metrics] latency_ms = {}", metrics.latency_ms);
    info!("[metrics] time_to_first_token_ms = {}", metrics.time_to_first_token_ms);
    info!("[metrics] tokens_streamed = {}", metrics.tokens_streamed);
    info!("[metrics] tokens_per_second = {:.1}", metrics.tokens_per_second);

    // Cleanup
    provider.unload().await?;

    info!("");
    info!("========================================");
    info!("  Demo Complete");
    info!("========================================");
    info!("");
    info!(
        "Result: Processed '{}' with {} tokens (latency: {}ms)",
        prompt, tokens_streamed, latency_ms
    );

    Ok(())
}
