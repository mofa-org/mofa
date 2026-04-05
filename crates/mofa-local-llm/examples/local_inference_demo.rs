//! Local Inference Demo - Phase 1
//!
//! Demonstrates real GGUF model inference using the Candle backend.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p mofa-local-llm --example local_inference_demo --features candle -- \
//!   --model ./models/tinyllama.gguf \
//!   --tokenizer ./models/tinyllama.tokenizer.json \
//!   --prompt "Hello"
//! ```
//!
//! ## Requirements
//!
//! - A GGUF model file (.gguf)
//! - A tokenizer.json file that matches the model
//!
//! ## What Phase 1 Implements
//!
//! - Real GGUF tensor loading (embedding + LM head)
//! - Real tokenization using tokenizers crate
//! - Candle-based inference (embedding lookup + matmul)
//! - Basic argmax sampling

use std::path::PathBuf;
use structopt::StructOpt;

use mofa_foundation::orchestrator::traits::ModelProvider;
use mofa_local_llm::{ComputeBackend, LinuxInferenceConfig, LinuxLocalProvider};

#[derive(Debug, StructOpt)]
#[structopt(name = "local_inference_demo")]
#[allow(dead_code)]
struct Opt {
    /// Path to GGUF model file
    #[structopt(short = "M", long, default_value = "./models/llama-7b.gguf")]
    model: PathBuf,

    /// Path to tokenizer.json
    #[structopt(short = "T", long, default_value = "./models/tokenizer.json")]
    tokenizer: PathBuf,

    /// Prompt for inference
    #[structopt(short, long, default_value = "Hello, how are you?")]
    prompt: String,

    /// Maximum tokens to generate
    #[structopt(long, default_value = "32")]
    max_tokens: usize,

    /// Temperature (not used in Phase 1, but stored for API compatibility)
    #[structopt(long, default_value = "0.8")]
    temperature: f32,

    /// Top-p sampling (not used in Phase 1, but stored for API compatibility)
    #[structopt(long, default_value = "0.9")]
    top_p: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Parse command line arguments
    let opt = Opt::from_args();

    println!("=== Local Inference Demo (Phase 1) ===");
    println!();
    println!("Model:    {:?}", opt.model);
    println!("Tokenizer: {:?}", opt.tokenizer);
    println!("Prompt:   {}", opt.prompt);
    println!("Max tokens: {}", opt.max_tokens);
    println!();

    // Check if model file exists
    if !opt.model.exists() {
        eprintln!("ERROR: Model file not found: {:?}", opt.model);
        eprintln!();
        eprintln!("Please provide a valid GGUF model path.");
        eprintln!("You can download TinyLlama from:");
        eprintln!("  https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF");
        std::process::exit(1);
    }

    // Check if tokenizer file exists
    if !opt.tokenizer.exists() {
        eprintln!("ERROR: Tokenizer file not found: {:?}", opt.tokenizer);
        eprintln!();
        eprintln!("Please provide a valid tokenizer.json path.");
        std::process::exit(1);
    }

    // Create provider configuration
    let config = LinuxInferenceConfig::new("tinyllama-demo", opt.model.to_str().unwrap())
        .with_tokenizer(opt.tokenizer.to_str().unwrap())
        .with_backend(ComputeBackend::Cpu)
        .with_num_threads(4)
        .unwrap();

    // Create the provider
    println!("[1] Creating LinuxLocalProvider...");
    let provider = LinuxLocalProvider::new(config)
        .map_err(|e| anyhow::anyhow!("Failed to create provider: {:?}", e))?;

    println!(
        "[2] Backend: {}",
        provider.get_metadata().get("backend").unwrap()
    );

    // Load the model
    println!("[3] Loading model (this may take a moment)...");
    let mut provider = provider;
    provider
        .load()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load model: {:?}", e))?;
    println!("[4] Model loaded successfully!");

    // Run inference
    println!("[5] Running inference...");
    println!();
    println!(">>> {}", opt.prompt);

    let result = provider
        .infer(&opt.prompt)
        .await
        .map_err(|e| anyhow::anyhow!("Inference failed: {:?}", e))?;

    println!("<<< {}", result);
    println!();

    // Show metadata
    println!("=== Inference Complete ===");
    println!("Memory usage: {} bytes", provider.memory_usage_bytes());

    Ok(())
}
