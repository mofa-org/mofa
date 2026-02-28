//! Kokoro TTS Comprehensive Example
//!
//! This example demonstrates various ways to use the Kokoro TTS engine:
//! - Basic synchronous synthesis
//! - Streaming synthesis with callbacks
//! - Voice enumeration
//! - Error handling
//! - Voice fallback behavior
//! - Concurrent synthesis
//!
//! Prerequisites:
//! - Set KOKORO_MODEL_PATH environment variable (or use default)
//! - Set KOKORO_VOICE_PATH environment variable (or use default)
//! - Both files must exist at the specified paths
//!
//! Run with:
//! ```bash
//! KOKORO_MODEL_PATH=/path/to/kokoro-v1.1-zh.onnx \
//! KOKORO_VOICE_PATH=/path/to/voices-v1.1-zh.bin \
//! cargo run --example kokoro_tts_demo
//! ```

use std::env;
use std::sync::{Arc, Mutex};

// Note: This example uses mock TTS to work without model files
// In a real scenario, you would import KokoroTTS from mofa_plugins

/// Simulated KokoroTTS for demonstration
struct MockKokoroTTS {
    model_version: String,
    voices: Vec<(String, String)>,
}

impl MockKokoroTTS {
    fn new(model_version: &str) -> Self {
        let voices = match model_version {
            "v1.1" => vec![
                ("default".to_string(), "Default Female (v1.1)".to_string()),
                ("zf_088".to_string(), "Zf088 (v1.1)".to_string()),
                ("zf_090".to_string(), "Zf090 (v1.1)".to_string()),
            ],
            _ => vec![
                ("default".to_string(), "Default Female (v1.0)".to_string()),
                ("zf_xiaoxiao".to_string(), "Xiaoxiao (v1.0)".to_string()),
                ("zf_xiaoyi".to_string(), "Xiaoyi (v1.0)".to_string()),
                ("zm_yunyang".to_string(), "Yunyang (v1.0)".to_string()),
            ],
        };

        Self {
            model_version: model_version.to_string(),
            voices,
        }
    }

    async fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8, Box<dyn std::error::Error>>, String> {
        println!("🎙️  Synthesizing with voice '{}': {}", voice, text);

        if text.is_empty() {
            return Err("Text cannot be empty".to_string());
        }

        // Generate fake WAV audio
        let audio_size = text.len() * 1000 + 100;
        Ok(vec![0u8; audio_size])
    }

    async fn synthesize_stream<F>(
        &self,
        text: &str,
        voice: &str,
        callback: F,
    ) -> Result<(), String>
    where
        F: Fn(Vec<u8>),
    {
        println!("🎙️  Streaming synthesis with voice '{}': {}", voice, text);

        if text.is_empty() {
            return Err("Text cannot be empty".to_string());
        }

        // Simulate chunked streaming
        let total_size = text.len() * 1000;
        let chunk_size = 50000;
        let chunks = (total_size + chunk_size - 1) / chunk_size;

        for i in 0..chunks {
            let audio_chunk = vec![
                0u8;
                std::cmp::min(
                    chunk_size,
                    total_size - i * chunk_size
                )
            ];
            callback(audio_chunk);
            println!("  └─ Chunk {}/{} delivered", i + 1, chunks);
        }

        Ok(())
    }

    fn list_voices(&self) -> Vec<(String, String)> {
        self.voices.clone()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════╗");
    println!("║   Kokoro TTS Comprehensive Example    ║");
    println!("╚═══════════════════════════════════════╝\n");

    // Detect model version
    let model_path = env::var("KOKORO_MODEL_PATH")
        .unwrap_or_else(|_| "kokoro-v1.1-zh.onnx".to_string());
    let version = if model_path.contains("v1.1") || model_path.contains("v11") {
        "v1.1"
    } else {
        "v1.0"
    };

    println!("📦 Model version detected: {}\n", version);

    let kokoro = MockKokoroTTS::new(version);

    // Example 1: Basic Synthesis
    example_basic_synthesis(&kokoro).await?;

    // Example 2: Different Voices
    example_voice_selection(&kokoro).await?;

    // Example 3: Voice Enumeration
    example_list_voices(&kokoro)?;

    // Example 4: Streaming Synthesis
    example_streaming_synthesis(&kokoro).await?;

    // Example 5: Error Handling
    example_error_handling(&kokoro).await?;

    // Example 6: Concurrent Synthesis
    example_concurrent_synthesis(&kokoro).await?;

    println!("\n✅ All examples completed successfully!\n");

    Ok(())
}

async fn example_basic_synthesis(kokoro: &MockKokoroTTS) -> Result<(), Box<dyn std::error::Error>> {
    println!("📌 Example 1: Basic Synchronous Synthesis");
    println!("─────────────────────────────────────────\n");

    let texts = vec!["你好世界", "Hello world", "这是一个测试"];

    for text in texts {
        match kokoro.synthesize(text, "default").await {
            Ok(audio) => println!("   ✓ Generated {} bytes of audio\n", audio.len()),
            Err(e) => println!("   ✗ Error: {}\n", e),
        }
    }

    Ok(())
}

async fn example_voice_selection(kokoro: &MockKokoroTTS) -> Result<(), Box<dyn std::error::Error>> {
    println!("📌 Example 2: Voice Selection");
    println!("─────────────────────────────────────────\n");

    // Use first two available voices
    let voices: Vec<_> = kokoro
        .list_voices()
        .iter()
        .take(2)
        .map(|(id, _)| id.clone())
        .collect();

    let test_text = "这是一个语音合成测试";

    for voice in voices {
        match kokoro.synthesize(test_text, &voice).await {
            Ok(audio) => println!("   ✓ {} generated {} bytes\n", voice, audio.len()),
            Err(e) => println!("   ✗ {}: {}\n", voice, e),
        }
    }

    println!("   Note: Unknown voices fall back to default silently\n");

    Ok(())
}

fn example_list_voices(kokoro: &MockKokoroTTS) -> Result<(), Box<dyn std::error::Error>> {
    println!("📌 Example 3: List Available Voices");
    println!("─────────────────────────────────────────\n");

    let voices = kokoro.list_voices();
    println!("   Available voices:");
    for (voice_id, voice_name) in voices {
        println!("   • {} - {}", voice_id, voice_name);
    }
    println!();

    Ok(())
}

async fn example_streaming_synthesis(
    kokoro: &MockKokoroTTS,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📌 Example 4: Streaming Synthesis");
    println!("─────────────────────────────────────────\n");

    let text = "这是一个长文本，将分块流送以支持实时处理和播放音频。";
    let chunk_count = Arc::new(Mutex::new(0));
    let chunk_count_clone = chunk_count.clone();

    kokoro
        .synthesize_stream(text, "default", move |chunk: Vec<u8>| {
            let mut count = chunk_count_clone.lock().unwrap();
            *count += 1;
            println!("   Received chunk: {} bytes", chunk.len());
        })
        .await?;

    let total_chunks = chunk_count.lock().unwrap();
    println!("   ✓ Received {} chunks total\n", *total_chunks);

    Ok(())
}

async fn example_error_handling(kokoro: &MockKokoroTTS) -> Result<(), Box<dyn std::error::Error>> {
    println!("📌 Example 5: Error Handling");
    println!("─────────────────────────────────────────\n");

    // Empty text error
    println!("   Testing empty text:");
    match kokoro.synthesize("", "default").await {
        Ok(_) => println!("   ✗ Unexpectedly succeeded"),
        Err(e) => println!("   ✓ Caught error: {}\n", e),
    }

    // Empty text in streaming
    println!("   Testing empty text in streaming:");
    match kokoro
        .synthesize_stream("", "default", |_| {})
        .await
    {
        Ok(_) => println!("   ✗ Unexpectedly succeeded"),
        Err(e) => println!("   ✓ Caught error: {}\n", e),
    }

    Ok(())
}

async fn example_concurrent_synthesis(
    kokoro: &MockKokoroTTS,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📌 Example 6: Concurrent Synthesis");
    println!("─────────────────────────────────────────\n");

    let kokoro = Arc::new(kokoro.clone());
    let mut handles = vec![];

    println!("   Spawning 5 concurrent synthesis tasks...\n");

    for i in 0..5 {
        let kokoro_clone = kokoro.clone();
        let handle = tokio::spawn(async move {
            let text = format!("并发任务 {}", i);
            kokoro_clone.synthesize(&text, "default").await
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }

    // Check results
    let successful = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.iter().filter(|r| r.is_err()).count();

    println!("\n   Results:");
    println!("   ✓ Successful: {}", successful);
    println!("   ✗ Failed: {}\n", failed);

    Ok(())
}

// Make MockKokoroTTS cloneable for Arc
impl Clone for MockKokoroTTS {
    fn clone(&self) -> Self {
        Self {
            model_version: self.model_version.clone(),
            voices: self.voices.clone(),
        }
    }
}
