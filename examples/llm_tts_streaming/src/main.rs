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
use uuid::Uuid;

#[cfg(feature = "audio-playback")]
use rodio::{OutputStream, Sink, buffer::SamplesBuffer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    #[cfg(feature = "audio-playback")]
    let (_output_stream, stream_handle) = OutputStream::try_default()?;
    #[cfg(feature = "audio-playback")]
    let audio_sink = Arc::new(Sink::try_new(&stream_handle)?);

    #[cfg(not(feature = "audio-playback"))]
    println!("Audio playback disabled. Rebuild with --features audio-playback to hear output.");

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

        // 中断现有播放并清空音频队列
        // Interrupt current playback and clear the audio queue
        agent.interrupt_tts().await?;
        #[cfg(feature = "audio-playback")]
        audio_sink.stop();  // 停止当前播放并清空队列
        // Stop the current playback and clear the queue

        #[cfg(feature = "audio-playback")]
        {
            let sink_clone = audio_sink.clone();
            agent.chat_with_tts_callback(
                &session_id,
                input,
                move |audio_f32| {
                    sink_clone.append(SamplesBuffer::new(1, 24000, audio_f32));
                }
            ).await?;

            // 启动播放（非阻塞）
            // Start playback (non-blocking)
            audio_sink.play();
            // 不再使用 sleep_until_end()，让音频在后台播放
            // No longer using sleep_until_end(), let audio play in the background
        }

        #[cfg(not(feature = "audio-playback"))]
        {
            agent.chat_with_tts_callback(&session_id, input, |_audio_f32| {}).await?;
        }
    }
    agent.remove_session(&session_id).await?;
    Ok(())
}
