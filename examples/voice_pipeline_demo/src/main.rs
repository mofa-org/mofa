//! Voice Pipeline Demo
//!
//! Demonstrates the framework-level End-to-End Voice Integration Pipeline
//! (ASR -> LLM -> TTS) using mock backends that run without requiring any
//! model files or GPUs.
//!
//! To run:
//! `cargo run -p voice_pipeline_demo`

use mofa_foundation::agent::voice::VoicePipelineExecutor;
use mofa_foundation::agent::voice_mock::{MockAsrStage, MockLlmStage, MockTtsStage};
use mofa_kernel::agent::voice::{StageInput, VoicePipelineConfig, VoiceStage};
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing to see the per-stage spans
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Initializing Voice Pipeline Demo...");

    // Create the mock stages (delay in ms simulates latency)
    let asr = Box::new(MockAsrStage::new(500));
    let llm = Box::new(MockLlmStage::new(1200));
    let tts = Box::new(MockTtsStage::new(800));

    let stages: Vec<Box<dyn VoiceStage>> = vec![asr, llm, tts];

    // Configure the pipeline
    let config = VoicePipelineConfig {
        abort_on_error: true,
        timeout_ms: Some(5000), // 5 seconds max for the whole pipeline
    };

    // Build the executor
    let pipeline = VoicePipelineExecutor::new(stages, config);

    // Simulate an incoming audio chunk (e.g., from a microphone)
    let mock_audio_input = vec![0.1f32, 0.2f32, 0.3f32, 0.4f32];
    let input = StageInput::Audio(mock_audio_input);

    info!("Starting pipeline execution...");
    
    // Execute the pipeline
    let output = pipeline.execute(input).await?;

    info!("Pipeline execution completed successfully!");
    info!("Total Latency: {}ms", output.total_latency_ms);
    
    for (stage_name, latency) in output.stage_latencies {
        info!(" -> {} Latency: {}ms", stage_name, latency);
    }

    Ok(())
}
