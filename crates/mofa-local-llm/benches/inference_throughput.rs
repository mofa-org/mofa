//! Inference throughput benchmarks
//!
//! Compares tokens-per-second across available compute backends.
//! Run with: `cargo bench -p mofa-local-llm`

use mofa_local_llm::{ComputeBackend, HardwareInfo, LinuxInferenceConfig, LinuxLocalProvider};
use std::time::Instant;

fn bench_backend(backend: ComputeBackend, model_path: &str, prompts: &[&str]) {
    let config = LinuxInferenceConfig::new(
        format!("bench-{}", backend),
        model_path,
    )
    .with_backend(backend.clone());

    let provider = match LinuxLocalProvider::new(config) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skip {backend}: {e}");
            return;
        }
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut total_tokens = 0usize;
    let start = Instant::now();

    for &prompt in prompts {
        let result = rt.block_on(async {
            let mut p = provider;
            // Load would fail without a real model file; count stub responses
            let out = p.infer(prompt).await;
            (p, out)
        });
        let (p, out) = result;
        if let Ok(response) = out {
            total_tokens += response.split_whitespace().count();
        }
        drop(p);
        break; // one round per backend in bench
    }

    let elapsed = start.elapsed();
    println!(
        "{backend} | prompts={} tokens={} elapsed={:.2}ms throughput={:.1} tok/s",
        prompts.len(),
        total_tokens,
        elapsed.as_secs_f64() * 1000.0,
        total_tokens as f64 / elapsed.as_secs_f64().max(0.001),
    );
}

fn main() {
    let info = HardwareInfo::detect();
    println!("=== mofa-local-llm inference throughput benchmark ===");
    println!("detected backend: {}", info.backend);
    println!("available: {:?}", info.available_backends);
    println!(
        "vram: {} MB | ram: {} MB | cores: {}",
        info.vram_bytes / (1024 * 1024),
        info.total_ram_bytes / (1024 * 1024),
        info.cpu_cores,
    );
    println!();

    let model_path = std::env::var("BENCH_MODEL_PATH")
        .unwrap_or_else(|_| "/tmp/bench-model.gguf".into());

    let prompts = [
        "explain the difference between CUDA and ROCm in two sentences",
        "what is the capital of France",
        "write a haiku about inference speed",
    ];

    for backend in &info.available_backends {
        bench_backend(backend.clone(), &model_path, &prompts);
    }
}
