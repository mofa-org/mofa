//! LLM + TTS Streaming Conversation Example (Simplified)
//!
//! 1. LLM 流式输出
//! 2. 遇到标点符号立即断句播放
//! 3. 逐句播放 TTS (using native f32 format for efficiency)

use futures::StreamExt;
use mofa_sdk::llm::{LLMAgentBuilder, openai_from_env};
use mofa_sdk::{KokoroTTS, TTSPlugin};
use rodio::{OutputStream, Sink, buffer::SamplesBuffer};
use std::env;
use std::io::Write;
use std::sync::Arc;
use uuid::Uuid;

/// 句子缓冲区：流式收集文本，遇到标点符号返回完整句子
struct SentenceBuffer {
    buffer: String,
}

impl SentenceBuffer {
    fn new() -> Self {
        Self { buffer: String::new() }
    }

    /// 推入文本块，返回完整句子（如果有）
    fn push(&mut self, text: &str) -> Option<String> {
        for ch in text.chars() {
            self.buffer.push(ch);
            // 句末标点：。！？!?
            if matches!(ch, '。' | '！' | '？' | '!' | '?') {
                let sentence = self.buffer.trim().to_string();
                if !sentence.is_empty() {
                    self.buffer.clear();
                    return Some(sentence);
                }
            }
        }
        None
    }

    /// 刷新剩余内容
    fn flush(&mut self) -> Option<String> {
        if self.buffer.trim().is_empty() {
            None
        } else {
            let remaining = self.buffer.trim().to_string();
            self.buffer.clear();
            Some(remaining)
        }
    }
}

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
            .with_temperature(0.7)
            .with_plugin(TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_088")))
            .build()
    );
    let (_output_stream, stream_handle) = OutputStream::try_default()?;
    let audio_sink = Arc::new(Sink::try_new(&stream_handle)?);

    let session_id = agent.current_session_id().await;
    println!("Using session: {}", session_id);

    println!("\n========================================");
    println!("  LLM + TTS Streaming Conversation");
    println!("========================================");
    println!("Type 'quit' to exit, 'clear' to clear history\n");

    loop {
        println!("\n请输入问题: ");
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
        let mut text_stream = agent.chat_stream_with_session(&session_id, input).await?;
        let mut buffer = SentenceBuffer::new();
        let audio_sink_clone = audio_sink.clone();

        // Collect all sentences first (流式收集所有句子)
        let mut sentences = Vec::new();

        while let Some(result) = text_stream.next().await {
            match result {
                Ok(text_chunk) => {
                    print!("{}", text_chunk);
                    std::io::stdout().flush()?;

                    // Check if we have a complete sentence
                    if let Some(sentence) = buffer.push(&text_chunk) {
                        println!("\n[TTS] {}", sentence);
                        sentences.push(sentence);
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("__stream_end__") {
                        break;
                    } else {
                        eprintln!("\nError: {}", e);
                        break;
                    }
                }
            }
        }

        // Add remaining content
        if let Some(remaining) = buffer.flush() {
            println!("\n[TTS] {}", remaining);
            sentences.push(remaining);
        }

        // Play all sentences through ONE stream using batch API
        if !sentences.is_empty() {
            println!("\n[Playing {} sentences through single stream...]", sentences.len());

            let sink_clone = audio_sink_clone.clone();
            agent.tts_speak_f32_stream_batch(
                sentences,
                Box::new(move |audio_f32| {
                    sink_clone.append(SamplesBuffer::new(1, 24000, audio_f32));
                }),
            ).await?;

            // Start playback and wait for completion
            audio_sink_clone.play();
            audio_sink_clone.sleep_until_end();
        }
    }
    agent.remove_session(&session_id).await?;
    Ok(())
}
