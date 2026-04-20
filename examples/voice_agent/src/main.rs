//! End-to-end voice agent example.
//!
//! Wires a full conversation loop using three cloud adapters in sequence:
//!
//! ```text
//!   Microphone  ──(WAV bytes)──▶  Deepgram ASR  ──(text)──▶  OpenAI LLM
//!                                                                   │
//!                                                              (reply text)
//!                                                                   │
//!   Speaker  ◀──(MP3 bytes)──  ElevenLabs TTS  ◀─────────────────────
//! ```
//!
//! # Required environment variables
//!
//! ```text
//! OPENAI_API_KEY      — OpenAI API key (LLM stage)
//! DEEPGRAM_API_KEY    — Deepgram API key (ASR stage)
//! ELEVENLABS_API_KEY  — ElevenLabs API key (TTS stage)
//! ```
//!
//! # Usage
//!
//! ```bash
//! cargo run -p voice_agent
//! ```
//!
//! Press **Enter** to begin a 5-second recording, then listen to the agent's
//! spoken reply.  Type `quit` or press **Ctrl-C** to exit.

mod audio_io;

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result};
use mofa_foundation::llm::openai::OpenAIProvider;
use mofa_foundation::llm::provider::LLMProvider as FoundationLLMProvider;
use mofa_foundation::llm::types::ChatCompletionRequest;
use mofa_integrations::speech::deepgram::{DeepgramAsrAdapter, DeepgramConfig};
use mofa_integrations::speech::elevenlabs::{ElevenLabsConfig, ElevenLabsTtsAdapter};
use mofa_kernel::speech::{AsrAdapter, AsrConfig, TtsAdapter, TtsConfig};
use tracing::{error, info};

const RECORDING_SECS: u64 = 5;
const LLM_MODEL: &str = "gpt-4o-mini";
const TTS_VOICE: &str = "Rachel";
const SYSTEM_PROMPT: &str =
    "You are a helpful voice assistant. Keep replies short and conversational — \
     no more than two or three sentences.";

#[tokio::main]
async fn main() -> Result<()> {
    // ── Logging ───────────────────────────────────────────────────────────────
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
            "Missing required environment variables: {}\n\
             Set them before running:\n  \
             export OPENAI_API_KEY=sk-...\n  \
             export DEEPGRAM_API_KEY=...\n  \
             export ELEVENLABS_API_KEY=...",
            missing.join(", ")
        );
    }

    // ── Build adapters ────────────────────────────────────────────────────────
    // ASR: Deepgram nova-2 — picks up DEEPGRAM_API_KEY from env
    let asr = DeepgramAsrAdapter::new(DeepgramConfig::new());

    // LLM: OpenAI — picks up OPENAI_API_KEY from env
    let llm = OpenAIProvider::from_env();

    // TTS: ElevenLabs — picks up ELEVENLABS_API_KEY from env
    let tts = ElevenLabsTtsAdapter::new(ElevenLabsConfig::new());

    // ── Conversation loop ─────────────────────────────────────────────────────
    println!();
    println!("Voice Agent ready.");
    println!("  Press Enter to record {RECORDING_SECS} seconds of speech.");
    println!("  Type 'quit' to exit.");
    println!();

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        print!("[ Press Enter to speak ] ");
        io::stdout().flush().ok();

        match lines.next() {
            None => break,
            Some(Err(e)) => {
                error!("stdin error: {e}");
                break;
            }
            Some(Ok(line)) => {
                let trimmed = line.trim().to_lowercase();
                if trimmed == "quit" || trimmed == "exit" {
                    println!("Goodbye!");
                    break;
                }
            }
        }

        // ── Stage 0: record from microphone ───────────────────────────────
        println!("Recording for {RECORDING_SECS} seconds…");
        let wav_bytes = match audio_io::record_wav(RECORDING_SECS) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Recording failed: {e}");
                continue;
            }
        };
        info!("Captured {} WAV bytes", wav_bytes.len());

        // ── Stage 1: ASR (Deepgram) ───────────────────────────────────────
        println!("Transcribing…");
        let transcription = match asr.transcribe(&wav_bytes, &AsrConfig::new()).await {
            Ok(t) => t,
            Err(e) => {
                eprintln!("ASR failed: {e}");
                continue;
            }
        };

        if transcription.text.trim().is_empty() {
            eprintln!("No speech detected — try again.");
            continue;
        }
        println!("  You: {}", transcription.text);

        // ── Stage 2: LLM (OpenAI) ─────────────────────────────────────────
        println!("Thinking…");
        let request = ChatCompletionRequest::new(LLM_MODEL)
            .system(SYSTEM_PROMPT)
            .user(&transcription.text);

        let response = match llm.chat(request).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("LLM failed: {e}");
                continue;
            }
        };

        let reply = response
            .content()
            .context("LLM returned no text content")?
            .to_string();
        println!("  Agent: {reply}");

        // ── Stage 3: TTS (ElevenLabs) ─────────────────────────────────────
        println!("Synthesising speech…");
        let audio = match tts.synthesize(&reply, TTS_VOICE, &TtsConfig::new()).await {
            Ok(a) => a,
            Err(e) => {
                eprintln!("TTS failed: {e}");
                continue;
            }
        };

        // ── Stage 4: play back via speaker ────────────────────────────────
        println!("Playing…");
        if let Err(e) = audio_io::play_mp3(audio.data) {
            eprintln!("Playback error: {e}");
        }

        println!();
    }

    Ok(())
}
