#[cfg(all(target_os = "linux", feature = "linux-candle"))]
use mofa_foundation::orchestrator::{
    LinuxCandleProvider, ModelProvider, ModelProviderConfig, ModelType,
};
#[cfg(all(target_os = "linux", feature = "linux-candle"))]
use serde_json::Value;
#[cfg(all(target_os = "linux", feature = "linux-candle"))]
use std::collections::HashMap;
#[cfg(all(target_os = "linux", feature = "linux-candle"))]
use std::env;
#[cfg(all(target_os = "linux", feature = "linux-candle"))]
use std::path::PathBuf;

#[cfg(all(target_os = "linux", feature = "linux-candle"))]
fn default_models_dir() -> PathBuf {
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("models");
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("models")
}

#[cfg(all(target_os = "linux", feature = "linux-candle"))]
fn build_chat_prompt(user_prompt: &str) -> String {
    // Qwen's instruct style chat framing so generation stays in assistant mode
    format!(
        "<|im_start|>system\nYou are Qwen, created by Alibaba Cloud. You are a helpful assistant.<|im_end|>\n<|im_start|>user\n{user_prompt}<|im_end|>\n<|im_start|>assistant\n"
    )
}

#[cfg(all(target_os = "linux", feature = "linux-candle"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Prefer project-root ./models when invoked from the repo, but keep env overrides available.
    let models_dir = default_models_dir();
    let model_path = env::var("MOFA_MODEL_PATH").unwrap_or_else(|_| {
        models_dir
            .join("qwen2.5-3b-instruct-q4_k_m.gguf")
            .display()
            .to_string()
    });
    let tokenizer_path = env::var("MOFA_TOKENIZER_PATH")
        .unwrap_or_else(|_| models_dir.join("tokenizer.json").display().to_string());
    let prompt = env::args()
        .nth(1)
        .unwrap_or_else(|| "Write a two-sentence summary of MoFA.".to_string());
    let prompt = build_chat_prompt(&prompt);
    // Keep the default short for local CPU smoke runs, callers can raise it via env
    let max_new_tokens = env::var("MOFA_MAX_NEW_TOKENS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(16);

    let mut extra_config = HashMap::new();
    extra_config.insert("tokenizer_path".to_string(), Value::String(tokenizer_path));
    extra_config.insert(
        "max_new_tokens".to_string(),
        Value::Number(max_new_tokens.into()),
    );

    let config = ModelProviderConfig {
        model_name: "qwen2.5-3b-q4_k_m".to_string(),
        model_path,
        device: "cpu".to_string(),
        model_type: ModelType::Llm,
        max_context_length: Some(4096),
        quantization: Some("q4_k_m".to_string()),
        extra_config,
    };

    let mut provider = LinuxCandleProvider::new(config);
    provider.load().await?;

    let output = provider.infer(&prompt).await?;
    println!("{output}");

    provider.unload().await?;
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "linux-candle")))]
fn main() {
    eprintln!(
        "This example requires a Linux target with the `linux-candle` feature enabled."
    );
    std::process::exit(1);
}
