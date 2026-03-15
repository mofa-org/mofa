//! Streaming TTS example — lower time-to-first-audio via sentence-boundary flushing.
//!
//! Instead of waiting for the full LLM response before synthesising speech, this
//! example streams LLM tokens and sends each completed sentence to ElevenLabs TTS
//! as soon as a sentence boundary (`.`, `!`, `?`) is detected.
//!
//! This pattern reduces perceived latency in real-time voice assistants because
//! the first sentence starts playing while the LLM is still generating the rest.
//!
//! # Required environment variables
//!
//! ```text
//! OPENAI_API_KEY     — OpenAI API key (streaming LLM)
//! ELEVENLABS_API_KEY — ElevenLabs API key (TTS)
//! ```
//!
//! # Optional environment variables
//!
//! ```text
//! VOICE_PROMPT — question to ask the LLM (default: "Explain how voice AI works in three sentences.")
//! ```
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin streaming_tts
//!
//! # Custom prompt:
//! VOICE_PROMPT="Tell me about Rust programming." cargo run --bin streaming_tts
//! ```

use anyhow::{Context, Result};
use futures::StreamExt;
use mofa_foundation::llm::openai::OpenAIProvider;
use mofa_foundation::llm::provider::LLMProvider as FoundationLLMProvider;
use mofa_foundation::llm::types::ChatCompletionRequest;
use mofa_integrations::speech::elevenlabs::{ElevenLabsConfig, ElevenLabsTtsAdapter};
use mofa_kernel::speech::{TtsAdapter, TtsConfig};
use tracing::info;

const LLM_MODEL: &str = "gpt-4o-mini";
const TTS_VOICE: &str = "Rachel";
const DEFAULT_PROMPT: &str = "Explain how voice AI works in three sentences.";
const SENTENCE_ENDINGS: &[char] = &['.', '!', '?'];

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    // ── Validate environment variables ────────────────────────────────────────
    let missing: Vec<&str> = ["OPENAI_API_KEY", "ELEVENLABS_API_KEY"]
        .into_iter()
        .filter(|k| std::env::var(k).is_err())
        .collect();

    if !missing.is_empty() {
        anyhow::bail!(
            "Missing required environment variables: {}",
            missing.join(", ")
        );
    }

    let prompt = std::env::var("VOICE_PROMPT").unwrap_or_else(|_| DEFAULT_PROMPT.to_string());
    println!("Prompt: {prompt}");
    println!("Streaming LLM tokens → sentence-boundary TTS flush…");
    println!();

    // ── Build providers ───────────────────────────────────────────────────────
    let llm = OpenAIProvider::from_env();
    let tts = ElevenLabsTtsAdapter::new(ElevenLabsConfig::new());

    // ── Stream LLM tokens ─────────────────────────────────────────────────────
    let request = ChatCompletionRequest::new(LLM_MODEL)
        .system("You are a concise voice assistant. Use short, clear sentences.")
        .user(&prompt);

    let mut stream = llm
        .chat_stream(request)
        .await
        .context("Failed to start LLM stream")?;

    let mut sentence_buf = String::new();
    let mut sentence_index: usize = 0;
    let mut all_mp3: Vec<Vec<u8>> = Vec::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("Stream chunk error")?;

        // Accumulate delta text from the first choice
        if let Some(delta) = chunk.choices.first().and_then(|c| c.delta.content.as_deref()) {
            print!("{delta}");
            sentence_buf.push_str(delta);

            // Flush on sentence boundary
            if sentence_buf.trim_end().ends_with(SENTENCE_ENDINGS) {
                let sentence = sentence_buf.trim().to_string();
                sentence_buf.clear();

                if sentence.is_empty() {
                    continue;
                }

                sentence_index += 1;
                info!(
                    sentence = sentence_index,
                    text = sentence,
                    "Flushing sentence to TTS"
                );

                // Synthesise this sentence immediately
                match tts.synthesize(&sentence, TTS_VOICE, &TtsConfig::new()).await {
                    Ok(audio) => {
                        info!(
                            sentence = sentence_index,
                            bytes = audio.data.len(),
                            "TTS chunk ready"
                        );
                        all_mp3.push(audio.data);
                    }
                    Err(e) => {
                        eprintln!("\nTTS failed for sentence {sentence_index}: {e}");
                    }
                }
            }
        }
    }
    println!(); // newline after streamed tokens

    // Flush any remaining partial sentence
    let remainder = sentence_buf.trim().to_string();
    if !remainder.is_empty() {
        info!(text = remainder, "Flushing final sentence fragment to TTS");
        match tts.synthesize(&remainder, TTS_VOICE, &TtsConfig::new()).await {
            Ok(audio) => all_mp3.push(audio.data),
            Err(e) => eprintln!("TTS failed for remainder: {e}"),
        }
    }

    // ── Write concatenated MP3 chunks to disk ─────────────────────────────────
    if all_mp3.is_empty() {
        println!("No audio synthesised.");
        return Ok(());
    }

    let combined: Vec<u8> = all_mp3.into_iter().flatten().collect();
    std::fs::write("streaming_reply.mp3", &combined)
        .context("Failed to write streaming_reply.mp3")?;

    println!(
        "Done! {} sentence(s) synthesised, {} bytes total → 'streaming_reply.mp3'.",
        sentence_index,
        combined.len()
    );
    println!("Play it with: afplay streaming_reply.mp3  (macOS) or mpv streaming_reply.mp3");

    Ok(())
}
