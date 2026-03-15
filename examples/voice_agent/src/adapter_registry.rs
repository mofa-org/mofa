//! Adapter registry example — swap ASR providers at runtime via environment variable.
//!
//! Demonstrates `SpeechAdapterRegistry`: both Deepgram and OpenAI Whisper ASR
//! adapters are registered; the one selected by `ASR_PROVIDER` (default:
//! `"deepgram"`) is used to transcribe the input text provided via `VOICE_INPUT`.
//!
//! This pattern is useful for A/B testing providers, cost-based routing, or
//! per-tenant adapter selection without recompiling.
//!
//! # Required environment variables
//!
//! ```text
//! OPENAI_API_KEY     — used for both OpenAI Whisper ASR and the LLM
//! DEEPGRAM_API_KEY   — Deepgram ASR key
//! ELEVENLABS_API_KEY — ElevenLabs TTS key
//! ```
//!
//! # Optional environment variables
//!
//! ```text
//! ASR_PROVIDER  — "deepgram" (default) or "whisper"
//! VOICE_INPUT   — text to synthesise as a test WAV for transcription
//!                 (default: generates silent WAV — useful for CI)
//! ```
//!
//! # Usage
//!
//! ```bash
//! # Use Deepgram (default):
//! cargo run --bin adapter_registry
//!
//! # Switch to OpenAI Whisper at runtime — no code change:
//! ASR_PROVIDER=whisper cargo run --bin adapter_registry
//! ```

use std::sync::Arc;

use anyhow::{Context, Result};
use mofa_foundation::llm::openai::OpenAIProvider;
use mofa_foundation::llm::provider::LLMProvider as FoundationLLMProvider;
use mofa_foundation::llm::types::ChatCompletionRequest;
use mofa_foundation::speech_registry::SpeechAdapterRegistry;
use mofa_integrations::speech::deepgram::{DeepgramAsrAdapter, DeepgramConfig};
use mofa_integrations::speech::elevenlabs::{ElevenLabsConfig, ElevenLabsTtsAdapter};
use mofa_integrations::speech::openai::{OpenAiAsrAdapter, OpenAiSpeechConfig};
use mofa_kernel::speech::{AsrConfig, TtsAdapter, TtsConfig};
use tracing::info;

const LLM_MODEL: &str = "gpt-4o-mini";
const TTS_VOICE: &str = "Rachel";
const SYSTEM_PROMPT: &str = "You are a helpful assistant. Respond concisely.";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    // ── Validate environment variables ────────────────────────────────────────
    let missing: Vec<&str> = ["OPENAI_API_KEY", "DEEPGRAM_API_KEY", "ELEVENLABS_API_KEY"]
        .into_iter()
        .filter(|k| std::env::var(k).is_err())
        .collect();

    if !missing.is_empty() {
        anyhow::bail!(
            "Missing required environment variables: {}",
            missing.join(", ")
        );
    }

    // ── Build and populate the registry ──────────────────────────────────────
    let mut registry = SpeechAdapterRegistry::new();

    // Register Deepgram ASR (keyed as "deepgram" via adapter.name())
    registry.register_asr(Arc::new(DeepgramAsrAdapter::new(DeepgramConfig::new())));

    // Register OpenAI Whisper ASR (keyed as "openai-whisper" via adapter.name())
    // OpenAiSpeechConfig::new() reads OPENAI_API_KEY from env automatically.
    registry.register_asr(Arc::new(OpenAiAsrAdapter::new(OpenAiSpeechConfig::new())));

    // Set default based on ASR_PROVIDER env var.
    // Valid values: "deepgram" (default) or "openai-whisper"
    let provider_name = std::env::var("ASR_PROVIDER")
        .unwrap_or_else(|_| "deepgram".to_string());

    if !registry.set_default_asr(&provider_name) {
        anyhow::bail!(
            "Unknown ASR_PROVIDER '{provider_name}' — valid values: {}",
            registry.list_asr().join(", ")
        );
    }

    println!("ASR provider selected: '{provider_name}'");
    info!(
        registered = registry.list_asr().join(", "),
        active = provider_name,
        "SpeechAdapterRegistry configured"
    );

    // ── Resolve input WAV ─────────────────────────────────────────────────────
    // In a real app this would come from a mic or uploaded file.
    // Here we generate a silent WAV so the demo runs without hardware.
    let wav_bytes = generate_silent_wav(16_000, 1);
    info!("Input WAV: {} bytes (silent test audio)", wav_bytes.len());

    // ── Stage 1: ASR via registry ─────────────────────────────────────────────
    let asr = registry
        .default_asr()
        .context("No default ASR adapter registered")?;

    println!("Transcribing with '{}'…", asr.name());
    let transcription = asr
        .transcribe(&wav_bytes, &AsrConfig::new())
        .await
        .context("ASR failed")?;

    // Silent audio produces empty transcript — show that gracefully
    let transcript = if transcription.text.trim().is_empty() {
        println!("(silent input — no transcript to display)");
        "Hello! This is a registry demo with silent test audio.".to_string()
    } else {
        println!("Transcript: {}", transcription.text);
        transcription.text.clone()
    };

    // ── Stage 2: LLM ─────────────────────────────────────────────────────────
    let llm = OpenAIProvider::from_env();
    let request = ChatCompletionRequest::new(LLM_MODEL)
        .system(SYSTEM_PROMPT)
        .user(&transcript);

    println!("Sending to LLM…");
    let response = llm.chat(request).await.context("LLM request failed")?;
    let reply = response
        .content()
        .context("LLM returned no text content")?
        .to_string();
    println!("LLM reply: {reply}");

    // ── Stage 3: TTS ─────────────────────────────────────────────────────────
    let tts = ElevenLabsTtsAdapter::new(ElevenLabsConfig::new());
    let audio = tts
        .synthesize(&reply, TTS_VOICE, &TtsConfig::new())
        .await
        .context("TTS failed")?;

    std::fs::write("registry_reply.mp3", &audio.data)
        .context("Failed to write registry_reply.mp3")?;

    println!(
        "Done! Saved {} bytes to 'registry_reply.mp3'. ASR provider used: '{provider_name}'.",
        audio.data.len()
    );

    Ok(())
}

fn generate_silent_wav(sample_rate: u32, duration_secs: u32) -> Vec<u8> {
    let num_samples = sample_rate * duration_secs;
    let mut buf = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(&mut buf, spec).expect("WavWriter init");
    for _ in 0..num_samples {
        writer.write_sample(0i16).expect("write sample");
    }
    writer.finalize().expect("finalize WAV");
    buf.into_inner()
}
