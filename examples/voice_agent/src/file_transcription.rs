//! File-based transcription example — no microphone or speakers required.
//!
//! Reads a WAV file from disk, transcribes it with Deepgram ASR, sends the
//! transcript to OpenAI LLM, synthesises the reply with ElevenLabs TTS, and
//! writes the resulting MP3 to `reply.mp3` in the current directory.
//!
//! This example is CI-friendly: it needs only API keys and a WAV file.
//!
//! # Required environment variables
//!
//! ```text
//! OPENAI_API_KEY     — OpenAI API key (LLM stage)
//! DEEPGRAM_API_KEY   — Deepgram API key (ASR stage)
//! ELEVENLABS_API_KEY — ElevenLabs API key (TTS stage)
//! ```
//!
//! # Usage
//!
//! ```bash
//! # Transcribe a specific WAV file:
//! cargo run --bin file_transcription -- path/to/audio.wav
//!
//! # No argument: generates a silent test WAV (useful for CI smoke tests):
//! cargo run --bin file_transcription
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use mofa_foundation::llm::openai::OpenAIProvider;
use mofa_foundation::llm::provider::LLMProvider as FoundationLLMProvider;
use mofa_foundation::llm::types::ChatCompletionRequest;
use mofa_integrations::speech::deepgram::{DeepgramAsrAdapter, DeepgramConfig};
use mofa_integrations::speech::elevenlabs::{ElevenLabsConfig, ElevenLabsTtsAdapter};
use mofa_kernel::speech::{AsrAdapter, AsrConfig, TtsAdapter, TtsConfig};
use tracing::info;

const LLM_MODEL: &str = "gpt-4o-mini";
const TTS_VOICE: &str = "Rachel";
const OUTPUT_PATH: &str = "reply.mp3";
const SYSTEM_PROMPT: &str =
    "You are a helpful assistant. Respond concisely to the transcribed speech.";

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

    // ── Resolve input WAV ─────────────────────────────────────────────────────
    let wav_bytes: Vec<u8> = match std::env::args().nth(1).map(PathBuf::from) {
        Some(path) => {
            println!("Reading WAV file: {}", path.display());
            std::fs::read(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?
        }
        None => {
            println!("No input file specified — generating 1-second silent WAV for smoke test.");
            generate_silent_wav(16_000, 1)
        }
    };

    info!("Input WAV: {} bytes", wav_bytes.len());

    // ── Build adapters ────────────────────────────────────────────────────────
    let asr = DeepgramAsrAdapter::new(DeepgramConfig::new());
    let llm = OpenAIProvider::from_env();
    let tts = ElevenLabsTtsAdapter::new(ElevenLabsConfig::new());

    // ── Stage 1: ASR ─────────────────────────────────────────────────────────
    println!("Transcribing with Deepgram ASR…");
    let transcription = asr
        .transcribe(&wav_bytes, &AsrConfig::new())
        .await
        .context("ASR transcription failed")?;

    if transcription.text.trim().is_empty() {
        println!("No speech detected in the audio file.");
        return Ok(());
    }

    println!("Transcript: {}", transcription.text);
    info!(
        confidence = transcription.confidence,
        language = transcription.language.as_deref().unwrap_or("unknown"),
        segments = transcription.segments.as_ref().map_or(0, |s| s.len()),
        "ASR complete"
    );

    // ── Stage 2: LLM ─────────────────────────────────────────────────────────
    println!("Sending transcript to OpenAI LLM…");
    let request = ChatCompletionRequest::new(LLM_MODEL)
        .system(SYSTEM_PROMPT)
        .user(&transcription.text);

    let response = llm
        .chat(request)
        .await
        .context("LLM request failed")?;

    let reply = response
        .content()
        .context("LLM returned no text content")?
        .to_string();

    println!("LLM reply: {reply}");

    // ── Stage 3: TTS ─────────────────────────────────────────────────────────
    println!("Synthesising speech with ElevenLabs TTS…");
    let audio = tts
        .synthesize(&reply, TTS_VOICE, &TtsConfig::new())
        .await
        .context("TTS synthesis failed")?;

    info!(
        bytes = audio.data.len(),
        format = ?audio.format,
        "TTS complete"
    );

    // ── Write output MP3 ─────────────────────────────────────────────────────
    std::fs::write(OUTPUT_PATH, &audio.data)
        .with_context(|| format!("Failed to write {OUTPUT_PATH}"))?;

    println!(
        "Done! Saved {} bytes of audio to '{OUTPUT_PATH}'.",
        audio.data.len()
    );

    Ok(())
}

/// Generate a minimal valid WAV with silence — no microphone or audio hardware required.
///
/// Useful for CI smoke tests where real speech is not available.
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
