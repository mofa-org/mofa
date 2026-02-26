//! LLM + TTS Streaming Conversation Example (Simplified)
//!
//! Demonstrates the new `chat_with_tts_callback` API that simplifies
//! LLM streaming with automatic sentence segmentation and TTS playback.
//!
//! Before: ~40 lines of boilerplate code
//! After: 1 line with the new API!

use mofa_sdk::llm::{LLMAgentBuilder, openai_from_env};
use mofa_sdk::plugins::{KokoroTTS, TTSPlugin};
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let model_path = env::var("KOKORO_MODEL_PATH")
        .unwrap_or_else(|_| "/Users/lijing/Downloads/kokoro-v1.1-zh.onnx".to_string());
    let voice_path = env::var("KOKORO_VOICE_PATH")
        .unwrap_or_else(|_| "/Users/lijing/Downloads/voices-v1.1-zh.bin".to_string());

    println!("Initializing TTS engine...");
    let kokoro_engine = KokoroTTS::new(&model_path, &voice_path).await?;

    println!("Initializing LLM Agent with TTS...");
    let agent = Arc::new(
        LLMAgentBuilder::new()
            .with_id(Uuid::new_v4().to_string())
            .with_name("Chat TTS Agent")
            .with_session_id(Uuid::new_v4().to_string())
            .with_provider(Arc::new(openai_from_env()?))
            .with_system_prompt("你是一个友好的AI助手。")
            // You are a friendly AI assistant.
            .with_temperature(0.7)
            .with_plugin(TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_088")))
            .build()
    );
    let rendered_chunks = Arc::new(AtomicUsize::new(0));

    let session_id = agent.current_session_id().await;
    println!("Using session: {}", session_id);

    println!("\n========================================");
    println!("  LLM + TTS Streaming Conversation");
    println!("========================================");
    println!("Type 'quit' to exit, 'clear' to clear history\n");

    loop {
        println!("\n请输入问题: ");
        // Please enter your question: 
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        }

        if input.eq_ignore_ascii_case("clear") {
            agent.clear_session_history(&session_id).await?;
            println!("Conversation history cleared.");
            continue;
        }

        if input.is_empty() {
            continue;
        }

        print!("AI: ");

        // 中断现有 TTS 合成
        // Interrupt ongoing TTS synthesis
        agent.interrupt_tts().await?;

        let rendered_chunks_clone = rendered_chunks.clone();
        agent.chat_with_tts_callback(
            &session_id,
            input,
            move |_audio_f32| {
                rendered_chunks_clone.fetch_add(1, Ordering::Relaxed);
            }
        ).await?;

        println!(
            "[info] Generated {} audio chunks for this response",
            rendered_chunks.load(Ordering::Relaxed)
        );
        rendered_chunks.store(0, Ordering::Relaxed);
    }
    agent.remove_session(&session_id).await?;
    Ok(())
}
