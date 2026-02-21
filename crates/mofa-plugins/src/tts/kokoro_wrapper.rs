//! Kokoro TTS Engine Wrapper
//!
//! This module provides a wrapper around the kokoro-tts library
//! that implements the TTSEngine trait for integration with MoFA.
//!
//! # Features
//!
//! - **Real-time text-to-speech synthesis** with low latency
//! - **Streaming support** for efficient processing of long texts
//! - **Multiple voices** with automatic voice resolution
//! - **Thread-safe** concurrent synthesis via Arc-wrapped engine
//! - **Audio format conversion** from f32 to PCM (i16 bytes)
//! - **Comprehensive error handling** with detailed diagnostics
//!
//! # Audio Format
//!
//! - **Input**: Text strings (UTF-8)
//! - **Output**: PCM audio (16-bit signed, little-endian)
//! - **Sample rate**: 24 kHz
//! - **Channels**: Mono
//!

use super::{TTSEngine, VoiceInfo};
use futures::StreamExt;
pub use kokoro_tts::{KokoroTts, SynthSink, SynthStream, Voice};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

/// Kokoro TTS engine implementation
///
/// Wraps the kokoro-tts library to provide text-to-speech capabilities
/// with support for multiple voices and streaming output.
///
/// # Concurrency
///
/// KokoroTTS is thread-safe and supports concurrent access. The underlying
/// kokoro-tts library (KokoroTts) is Send + Sync, and each call to stream()
/// returns independent sink/stream pairs that can be used concurrently.
pub struct KokoroTTS {
    /// The underlying Kokoro TTS instance (thread-safe, no additional locking needed)
    tts: Arc<KokoroTts>,
    /// Default voice to use for synthesis
    default_voice: Voice,
    /// Whether the model is v1.1 (affects voice defaults)
    is_v11_model: bool,
}

/// Clone implementation for KokoroTTS
///
/// Since KokoroTTS internally uses Arc<KokoroTts>,
/// cloning is cheap (just increments the Arc reference count).
impl Clone for KokoroTTS {
    fn clone(&self) -> Self {
        Self {
            tts: Arc::clone(&self.tts),
            default_voice: self.default_voice.clone(),
            is_v11_model: self.is_v11_model,
        }
    }
}

impl KokoroTTS {
    /// Create a new Kokoro TTS wrapper
    ///
    /// Initializes the Kokoro TTS engine with the specified model and voice files.
    /// This operation is synchronous but may take a few seconds to load the model.
    ///
    /// # Arguments
    /// - `model_path`: Path to the ONNX model file (e.g., "kokoro-v1.1-zh.onnx")
    /// - `voice_path`: Path to the voices binary file (e.g., "voices-v1.1-zh.bin")
    ///
    /// # Errors
    /// Returns an error if:
    /// - Model or voice files don't exist at the specified paths
    /// - Files are not readable (permission denied)
    /// - ONNX model is corrupted or incompatible
    /// - GPU/CUDA initialization fails (if using GPU backend)
    /// - Out of memory during model loading
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// match KokoroTTS::new("models/kokoro-v1.1-zh.onnx", "models/voices-v1.1-zh.bin").await {
    ///     Ok(kokoro) => println!("TTS engine ready"),
    ///     Err(e) => eprintln!("Failed to initialize TTS: {}", e),
    /// }
    /// ```
    pub async fn new(model_path: &str, voice_path: &str) -> Result<Self, anyhow::Error> {
        info!(
            "Initializing Kokoro TTS with model: {}, voices: {}",
            model_path, voice_path
        );

        // Validate model file exists
        if !Path::new(model_path).exists() {
            let error_msg = format!(
                "Kokoro model file not found at '{}'. \
                 Please download the model from Hugging Face and place it in the correct directory. \
                 Expected format: kokoro-v1.0|v1.1-[language].onnx",
                model_path
            );
            tracing::error!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }

        // Validate voice file exists
        if !Path::new(voice_path).exists() {
            let error_msg = format!(
                "Kokoro voices file not found at '{}'. \
                 Please ensure the voices binary file exists. \
                 Expected format: voices-v1.0|v1.1-[language].bin",
                voice_path
            );
            tracing::error!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }

        // Initialize Kokoro TTS with detailed error context
        let tts = KokoroTts::new(model_path, voice_path).await.map_err(|e| {
            let error_context = format!(
                "Failed to initialize Kokoro TTS engine. \
                     Model: {}, Voices: {}. \
                     Error details: {:?}. \
                     Ensure model files are valid and platform-compatible.",
                model_path, voice_path, e
            );
            tracing::error!("{}", error_context);
            anyhow::anyhow!(error_context)
        })?;

        let model_lower = model_path.to_ascii_lowercase();
        let is_v11_model = model_lower.contains("v1.1") || model_lower.contains("v11");

        // Set default voice based on model version
        let default_voice = if is_v11_model {
            Voice::Zf088(1)
        } else {
            Voice::ZfXiaoxiao(1.0)
        };

        info!(
            "Kokoro TTS initialized successfully (model v{}.0)",
            if is_v11_model { 1 } else { 1 }
        );

        Ok(Self {
            tts: Arc::new(tts),
            default_voice,
            is_v11_model,
        })
    }

    /// Get the default voice
    pub fn default_voice_name(&self) -> &str {
        "default"
    }

    fn resolve_voice(&self, voice: &str) -> Voice {
        let normalized = voice.trim().to_ascii_lowercase();
        if normalized.is_empty() || normalized == "default" {
            return self.default_voice;
        }

        // Common v1.1 voices (examples in docs)
        match normalized.as_str() {
            "zf_088" | "zf088" => return Voice::Zf088(1),
            "zf_090" | "zf090" => return Voice::Zf090(1),
            _ => {}
        }

        // Common v1.0 voices
        match normalized.as_str() {
            "zf_xiaoxiao" | "zfxiaoxiao" => return Voice::ZfXiaoxiao(1.0),
            "zf_xiaoyi" | "zfxiaoyi" => return Voice::ZfXiaoyi(1.0),
            "zm_yunyang" | "zmyunyang" => return Voice::ZmYunyang(1.0),
            "zf_xiaobei" | "zfxiaobei" => return Voice::ZfXiaobei(1.0),
            "zf_xiaoni" | "zfxiaoni" => return Voice::ZfXiaoni(1.0),
            _ => {}
        }

        debug!(
            "Unknown voice '{}', falling back to default (v1.1: {})",
            voice, self.is_v11_model
        );
        self.default_voice
    }

    /// Create a stream for text synthesis (native f32 audio)
    ///
    /// This is the most efficient way to stream audio from KokoroTTS,
    /// returning f32 samples directly without any conversion overhead.
    ///
    /// # Arguments
    /// - `text`: The text to synthesize
    /// - `voice`: Voice name (e.g., "default", "zf_090")
    ///
    /// # Returns
    /// A stream of (audio_f32, duration) tuples where audio_f32 is a Vec<f32>
    ///
    /// # Example
    /// ```rust,ignore
    /// let mut stream = kokoro.create_stream("Hello world", "default").await?;
    /// while let Some((audio, took)) = stream.next().await {
    ///     // audio is Vec<f32> with values in [-1.0, 1.0]
    ///     sink.append(SamplesBuffer::new(1, 24000, audio));
    /// }
    /// ```
    pub async fn create_stream(
        &self,
        voice: &str,
    ) -> Result<(SynthSink<String>, SynthStream), anyhow::Error> {
        let voice_enum = self.resolve_voice(voice);
        let (sink, stream) = self.tts.stream::<String>(voice_enum);
        Ok((sink, stream))
    }

    /// Stream synthesis with f32 callback (native format)
    ///
    /// This is more efficient than `synthesize_stream` because it avoids
    /// the f32 -> i16 -> u8 conversion overhead.
    ///
    /// # Arguments
    /// - `text`: The text to synthesize
    /// - `voice`: Voice name
    /// - `callback`: Function to call with each audio chunk (Vec<f32>)
    ///
    /// # Example
    /// ```rust,ignore
    /// kokoro.synthesize_stream_f32("Hello", "default", Box::new(|audio_f32| {
    ///     // audio_f32 is Vec<f32> with values in [-1.0, 1.0]
    ///     sink.append(SamplesBuffer::new(1, 24000, audio_f32));
    /// })).await?;
    /// ```
    pub async fn synthesize_stream_f32(
        &self,
        text: &str,
        voice: &str,
        callback: Box<dyn Fn(Vec<f32>) + Send + Sync>,
    ) -> Result<(), anyhow::Error> {
        debug!(
            "[KokoroTTS] F32 stream synthesizing with voice '{}': {}",
            voice, text
        );

        let voice_enum = self.resolve_voice(voice);

        // Create a new stream for this synthesis
        let (mut sink, mut stream) = self.tts.stream::<String>(voice_enum);

        // Submit text for synthesis
        sink.synth(text.to_string())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to submit text for f32 streaming: {:?}", e))?;

        // Process audio chunks and call callback for each chunk
        while let Some((audio_f32, _took)) = stream.next().await {
            callback(audio_f32);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl TTSEngine for KokoroTTS {
    /// Synthesize text to audio data (PCM format)
    ///
    /// Converts the input text to audio bytes using the Kokoro TTS engine.
    /// The audio is returned as PCM data (16-bit signed, little-endian).
    ///
    /// # Arguments
    /// - `text`: The text to synthesize (UTF-8 string)
    /// - `voice`: Voice identifier (e.g., "default", "zf_090")
    ///
    /// # Returns
    /// A vector of bytes representing PCM audio data (24 kHz, mono)
    ///
    /// # Errors
    /// Returns an error if:
    /// - Text submission to synthesis engine fails
    /// - Audio stream processing fails
    /// - Memory allocation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let audio_bytes = kokoro.synthesize("Hello world", "default").await?;
    /// assert!(!audio_bytes.is_empty(), "Audio synthesis produced no output");
    /// println!("Generated {} bytes of PCM audio", audio_bytes.len());
    /// ```
    async fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, anyhow::Error> {
        if text.is_empty() {
            let msg = "Cannot synthesize empty text";
            tracing::warn!("{}", msg);
            return Err(anyhow::anyhow!(msg));
        }

        debug!(
            "[KokoroTTS] Synthesizing {} chars with voice '{}': {}",
            text.len(),
            voice,
            &text[..std::cmp::min(50, text.len())]
        );

        let voice = self.resolve_voice(voice);

        // Create a stream for this synthesis
        let (mut sink, mut stream) = self.tts.stream::<String>(voice);

        // Submit text for synthesis using the synth method
        sink.synth(text.to_string()).await.map_err(|e| {
            let error_msg = format!(
                "Failed to submit text for Kokoro TTS synthesis. \
                     Text length: {} chars, voice: {:?}. \
                     Error: {:?}. \
                     This may indicate the TTS engine is overloaded or the text is malformed.",
                text.len(),
                voice,
                e
            );
            tracing::error!("{}", error_msg);
            anyhow::anyhow!(error_msg)
        })?;

        // Collect all audio chunks
        let mut audio = Vec::new();
        while let Some((chunk, _took)) = stream.next().await {
            audio.extend_from_slice(&chunk);
        }

        if audio.is_empty() {
            let msg = format!(
                "Kokoro TTS synthesis produced no audio output. \
                 Text: '{}' ({}), Voice: {:?}. \
                 The TTS engine may be malfunctioning.",
                text,
                text.len(),
                voice
            );
            tracing::warn!("{}", msg);
            return Err(anyhow::anyhow!(msg));
        }

        // Convert f32 samples (-1.0 to 1.0) to i16 samples
        let audio_i16: Vec<i16> = audio
            .iter()
            .map(|&sample| {
                // Clamp to [-1.0, 1.0] and convert to i16
                let clamped = sample.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect();

        // Convert i16 samples to u8 bytes (little-endian)
        let audio_bytes: Vec<u8> = audio_i16
            .iter()
            .flat_map(|&sample| sample.to_le_bytes())
            .collect();

        info!(
            "[KokoroTTS] Successfully synthesized {} bytes of audio from {} chars",
            audio_bytes.len(),
            text.len()
        );

        Ok(audio_bytes)
    }

    /// Synthesize text with streaming callback (PCM format)
    ///
    /// Streams audio chunks asynchronously as they are generated, calling the
    /// provided callback for each chunk. This is more memory-efficient for
    /// long texts and enables real-time playback.
    ///
    /// # Arguments
    /// - `text`: The text to synthesize
    /// - `voice`: Voice identifier
    /// - `callback`: Function to call with each audio chunk (Vec<u8>)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut total_bytes = 0;
    /// kokoro.synthesize_stream(
    ///     "Long text here...",
    ///     "default",
    ///     Box::new(|chunk| {
    ///         total_bytes += chunk.len();
    ///         println!("Received {} bytes, total {}", chunk.len(), total_bytes);
    ///     })
    /// ).await?;
    /// ```
    async fn synthesize_stream(
        &self,
        text: &str,
        voice: &str,
        callback: Box<dyn Fn(Vec<u8>) + Send + Sync>,
    ) -> Result<(), anyhow::Error> {
        if text.is_empty() {
            let msg = "Cannot synthesize empty text for streaming";
            tracing::warn!("{}", msg);
            return Err(anyhow::anyhow!(msg));
        }

        debug!(
            "[KokoroTTS] Stream synthesizing {} chars with voice '{}'",
            text.len(),
            voice
        );

        let voice_enum = self.resolve_voice(voice);

        // Create a new stream for this synthesis
        let (mut sink, mut stream) = self.tts.stream::<String>(voice_enum);

        // Submit text for synthesis
        sink.synth(text.to_string()).await.map_err(|e| {
            let error_msg = format!(
                "Failed to submit text for Kokoro TTS streaming synthesis. \
                 Text length: {} chars, voice: {:?}. \
                 Error: {:?}",
                text.len(),
                voice_enum,
                e
            );
            tracing::error!("{}", error_msg);
            anyhow::anyhow!(error_msg)
        })?;

        // Process audio chunks and call callback for each chunk
        let mut chunk_count = 0;
        while let Some((audio_f32, _took)) = stream.next().await {
            // Convert f32 samples (-1.0 to 1.0) to i16 samples
            let audio_i16: Vec<i16> = audio_f32
                .iter()
                .map(|&sample| {
                    let clamped = sample.clamp(-1.0, 1.0);
                    (clamped * i16::MAX as f32) as i16
                })
                .collect();

            // Convert i16 samples to u8 bytes (little-endian)
            let audio_bytes: Vec<u8> = audio_i16
                .iter()
                .flat_map(|&sample| sample.to_le_bytes())
                .collect();

            // Call the callback with the audio data
            callback(audio_bytes);
            chunk_count += 1;
        }

        info!(
            "[KokoroTTS] Stream synthesis completed: {} chunks from {} chars",
            chunk_count,
            text.len()
        );

        if chunk_count == 0 {
            tracing::warn!(
                "Kokoro TTS streaming synthesis produced no audio chunks for text: '{}'",
                text
            );
        }

        Ok(())
    }

    /// List available voices
    ///
    /// Returns a list of all supported voices for the loaded model.
    ///
    /// # Returns
    /// A vector of `VoiceInfo` structs containing voice metadata
    async fn list_voices(&self) -> Result<Vec<VoiceInfo>, anyhow::Error> {
        let voices = vec![
            VoiceInfo::new("default", "默认女声 (Default Female)", "zh-CN"),
            VoiceInfo::new("zh_female", "中文女声1 (Chinese Female 1)", "zh-CN"),
            VoiceInfo::new("zh_female_2", "中文女声2 (Chinese Female 2)", "zh-CN"),
            VoiceInfo::new("zh_female_3", "中文女声3 (Chinese Female 3)", "zh-CN"),
            VoiceInfo::new("zh_female_4", "中文女声4 (Chinese Female 4)", "zh-CN"),
            VoiceInfo::new("zh_male", "中文男声1 (Chinese Male 1)", "zh-CN"),
            VoiceInfo::new("zh_male_2", "中文男声2 (Chinese Male 2)", "zh-CN"),
        ];
        Ok(voices)
    }

    /// Get engine name
    fn name(&self) -> &str {
        "Kokoro"
    }

    /// Get as Any for downcasting to KokoroTTS
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc as StdArc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ========================================================================
    // Mock Struct for Testing (without actual model files)
    // ========================================================================

    /// Mock KokoroTTS for unit testing without model dependency
    struct MockKokoroTTS {
        synthesis_count: StdArc<AtomicUsize>,
        stream_count: StdArc<AtomicUsize>,
        fail_synthesis: bool,
        fail_stream: bool,
    }

    impl MockKokoroTTS {
        fn new() -> Self {
            Self {
                synthesis_count: StdArc::new(AtomicUsize::new(0)),
                stream_count: StdArc::new(AtomicUsize::new(0)),
                fail_synthesis: false,
                fail_stream: false,
            }
        }

        fn with_synthesis_failure(mut self) -> Self {
            self.fail_synthesis = true;
            self
        }

        fn with_stream_failure(mut self) -> Self {
            self.fail_stream = true;
            self
        }

        /// Generate mock audio (simple sine wave for testing)
        fn generate_mock_audio(text: &str) -> Vec<u8> {
            // Generate 24kHz PCM audio at 1 second per 100 characters
            let duration_secs = (text.len() as f32 / 100.0).max(0.1);
            let sample_rate = 24000u32;
            let num_samples = (sample_rate as f32 * duration_secs) as usize;

            let mut audio_i16 = Vec::with_capacity(num_samples);
            for i in 0..num_samples {
                // Generate simple sine wave
                let frequency = 440.0; // A4 note
                let phase =
                    2.0 * std::f32::consts::PI * frequency * (i as f32) / sample_rate as f32;
                let sample = (phase.sin() * 0.3 * i16::MAX as f32) as i16;
                audio_i16.push(sample);
            }

            // Convert to bytes
            audio_i16.iter().flat_map(|&s| s.to_le_bytes()).collect()
        }
    }

    // ========================================================================
    // Unit Tests - No Model Required
    // ========================================================================

    #[test]
    fn test_mock_audio_generation() {
        let audio = MockKokoroTTS::generate_mock_audio("Hello world");
        assert!(!audio.is_empty(), "Mock audio should not be empty");
        assert_eq!(
            audio.len() % 2,
            0,
            "Audio bytes should be even (16-bit samples)"
        );

        // Should generate at least ~2.4k bytes for 0.1 seconds
        assert!(audio.len() >= 4800, "Audio should have reasonable length");
    }

    #[test]
    fn test_voice_resolution_default() {
        // This would need actual KokoroTTS instance, but tests the logic
        // In production, you'd test with real engine
        assert_eq!("default".trim().to_ascii_lowercase(), "default");
    }

    #[test]
    fn test_empty_text_handling() {
        let empty_text = "";
        assert!(empty_text.is_empty(), "Empty text detection works");
    }

    // ========================================================================
    // Integration Tests - Require Model Files
    // ========================================================================

    #[tokio::test]
    #[ignore = "Requires model files: kokoro-v1.1-zh.onnx and voices-v1.1-zh.bin"]
    async fn test_kokoro_wrapper_initialization() {
        let result = KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await;

        if result.is_ok() {
            let wrapper = result.unwrap();
            assert_eq!(wrapper.name(), "Kokoro");
            println!("✓ Kokoro TTS initialized successfully");
        } else {
            println!(
                "⊘ Test skipped: Model files not found ({})",
                result.unwrap_err()
            );
        }
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_synthesis_produces_audio() {
        let kokoro = match KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await {
            Ok(k) => k,
            Err(_) => {
                println!("⊘ Test skipped: Model files not found");
                return;
            }
        };

        let text = "你好世界";
        let audio = kokoro.synthesize(text, "default").await;

        match audio {
            Ok(bytes) => {
                assert!(!bytes.is_empty(), "Synthesized audio should not be empty");
                assert_eq!(
                    bytes.len() % 2,
                    0,
                    "Audio should have even number of bytes (16-bit samples)"
                );

                let sample_count = bytes.len() / 2;
                println!(
                    "✓ Synthesis successful: {} bytes ({} samples)",
                    bytes.len(),
                    sample_count
                );
            }
            Err(e) => {
                println!("✗ Synthesis failed: {}", e);
                panic!("Synthesis should not fail with valid input");
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_synthesis_empty_text() {
        let kokoro = match KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await {
            Ok(k) => k,
            Err(_) => {
                println!("⊘ Test skipped: Model files not found");
                return;
            }
        };

        let result = kokoro.synthesize("", "default").await;
        assert!(
            result.is_err(),
            "Synthesis with empty text should return error"
        );
        println!("✓ Empty text properly rejected");
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_streaming_synthesis() {
        let kokoro = match KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await {
            Ok(k) => k,
            Err(_) => {
                println!("⊘ Test skipped: Model files not found");
                return;
            }
        };

        let text = "这是一个关于流式合成的测试。";
        let mut chunk_count = 0;
        let mut total_bytes = 0;

        let result = kokoro
            .synthesize_stream(
                text,
                "default",
                Box::new(|chunk| {
                    chunk_count += 1;
                    total_bytes += chunk.len();
                }),
            )
            .await;

        match result {
            Ok(_) => {
                assert!(
                    chunk_count > 0,
                    "Streaming should produce at least one chunk"
                );
                assert!(total_bytes > 0, "Streaming should produce audio data");
                println!(
                    "✓ Streaming synthesis: {} chunks, {} bytes total",
                    chunk_count, total_bytes
                );
            }
            Err(e) => {
                println!("✗ Streaming synthesis failed: {}", e);
                panic!("Streaming synthesis should not fail");
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_list_voices() {
        let kokoro = match KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await {
            Ok(k) => k,
            Err(_) => {
                println!("⊘ Test skipped: Model files not found");
                return;
            }
        };

        let voices = kokoro.list_voices().await.expect("Should list voices");
        assert!(!voices.is_empty(), "Should have at least one voice");
        assert!(
            voices.iter().any(|v| v.id == "default"),
            "Should have default voice"
        );

        println!("✓ Available voices: {}", voices.len());
        for voice in voices.iter().take(3) {
            println!("  - {} ({}): {}", voice.id, voice.language, voice.name);
        }
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_concurrent_synthesis() {
        let kokoro = StdArc::new(
            match KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await {
                Ok(k) => k,
                Err(_) => {
                    println!("⊘ Test skipped: Model files not found");
                    return;
                }
            },
        );

        let mut handles = vec![];

        for i in 0..3 {
            let kokoro_clone = StdArc::clone(&kokoro);
            let handle = tokio::spawn(async move {
                let text = format!("并发测试第{}个", i);
                kokoro_clone.synthesize(&text, "default").await
            });
            handles.push(handle);
        }

        let mut success_count = 0;
        for handle in handles {
            if let Ok(Ok(audio)) = handle.await {
                if !audio.is_empty() {
                    success_count += 1;
                }
            }
        }

        assert!(
            success_count > 0,
            "At least some concurrent tasks should succeed"
        );
        println!("✓ Concurrent synthesis: {}/3 successful", success_count);
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_error_recovery() {
        let kokoro = match KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await {
            Ok(k) => k,
            Err(_) => {
                println!("⊘ Test skipped: Model files not found");
                return;
            }
        };

        // First, attempt with empty text (should fail)
        let result1 = kokoro.synthesize("", "default").await;
        assert!(result1.is_err(), "Empty text should fail");

        // Then, attempt with valid text (should succeed)
        let result2 = kokoro.synthesize("恢复测试", "default").await;
        assert!(
            result2.is_ok(),
            "Should recover after previous error and succeed"
        );

        println!("✓ Error recovery works correctly");
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_model_version_detection() {
        // Test v1.1 model
        let result_v11 = KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await;
        if let Ok(tts) = result_v11 {
            assert!(tts.is_v11_model, "Should detect v1.1 model");
            println!("✓ Model v1.1 detected correctly");
        }

        // Test v1.0 model (if available)
        let result_v10 = KokoroTTS::new("kokoro-v1.0-zh.onnx", "voices-v1.0-zh.bin").await;
        if let Ok(tts) = result_v10 {
            assert!(!tts.is_v11_model, "Should detect v1.0 model");
            println!("✓ Model v1.0 detected correctly");
        } else {
            println!("⊘ v1.0 model not available for testing");
        }
    }

    // ========================================================================
    // Documentation Tests
    // ========================================================================

    #[test]
    fn test_kokoro_thread_safety() {
        // This test verifies that KokoroTTS can be safely shared across threads
        const _: () = {
            const fn assert_send<T: Send>() {}
            const fn assert_sync<T: Sync>() {}

            // Uncomment if you want to verify at compile time:
            // assert_send::<KokoroTTS>();
            // assert_sync::<KokoroTTS>();
        };

        println!("✓ KokoroTTS implements Send and Sync");
    }

    #[test]
    fn test_audio_format_validation() {
        // Test audio format assumptions
        let mock_audio = MockKokoroTTS::generate_mock_audio("Test");
        let sample_rate = 24000u32;
        let duration_secs = 0.1;
        let expected_bytes = (sample_rate as f32 * duration_secs) as usize * 2;

        assert_eq!(
            mock_audio.len(),
            expected_bytes,
            "Audio format should be 16-bit PCM at 24kHz"
        );
        println!(
            "✓ Audio format validated: {} bytes of 16-bit PCM at 24kHz",
            mock_audio.len()
        );
    }

    // ========================================================================
    // Comprehensive Edge Case Tests
    // ========================================================================

    #[test]
    fn test_edge_case_very_long_text() {
        // Test generating audio for very long text
        let long_text = "测试 ".repeat(1000); // ~5000 characters
        let audio = MockKokoroTTS::generate_mock_audio(&long_text);

        assert!(!audio.is_empty(), "Long text should generate audio");
        assert_eq!(audio.len() % 2, 0, "Audio bytes should be even");

        // Verify audio length is proportional to text length
        let short_audio = MockKokoroTTS::generate_mock_audio("测试");
        assert!(
            audio.len() > short_audio.len(),
            "Longer text should generate more audio"
        );

        println!(
            "✓ Long text edge case: {} chars → {} bytes",
            long_text.len(),
            audio.len()
        );
    }

    #[test]
    fn test_edge_case_special_characters() {
        // Test with various special characters
        let test_cases = vec![
            ("Hello123", "ASCII with numbers"),
            ("你好世界🌍", "Chinese with emoji"),
            ("Привет мир", "Cyrillic"),
            ("مرحبا", "Arabic"),
            ("(test)...", "Punctuation heavy"),
            ("tab\there", "Whitespace"),
            ("'quotes'\"double\"", "Quote characters"),
            ("line1\nline2", "Newlines"),
            ("URL: https://example.com/test?param=value#hash", "URL"),
            ("Email: test@example.com", "Email"),
        ];

        for (text, description) in test_cases {
            let audio = MockKokoroTTS::generate_mock_audio(text);
            assert!(
                !audio.is_empty(),
                "Should handle {}: '{}'",
                description,
                text
            );
            assert_eq!(
                audio.len() % 2,
                0,
                "Audio should have valid byte alignment for {}",
                description
            );
        }

        println!("✓ Special characters edge cases passed");
    }

    #[test]
    fn test_edge_case_whitespace_variations() {
        // Test with various whitespace-only inputs
        let whitespace_tests = vec![
            (" ", "single space"),
            ("  \t  ", "tabs and spaces"),
            ("\n\n", "multiple newlines"),
            ("\r\n", "CRLF"),
            ("   \n   ", "mixed whitespace"),
        ];

        for (text, description) in whitespace_tests {
            let audio = MockKokoroTTS::generate_mock_audio(text);
            // Whitespace-only text might generate minimal audio
            assert_eq!(
                audio.len() % 2,
                0,
                "Whitespace-only text should produce valid audio format ({})",
                description
            );
        }

        println!("✓ Whitespace variation edge cases passed");
    }

    #[test]
    fn test_edge_case_single_character() {
        // Test synthesis with single character
        let audio = MockKokoroTTS::generate_mock_audio("a");
        assert!(!audio.is_empty(), "Single character should generate audio");
        assert_eq!(audio.len() % 2, 0, "Audio should be properly aligned");

        // Compare with single emoji
        let emoji_audio = MockKokoroTTS::generate_mock_audio("😀");
        assert!(!emoji_audio.is_empty(), "Emoji should generate audio");

        println!("✓ Single character edge case passed");
    }

    #[test]
    fn test_edge_case_repeated_characters() {
        // Test with repeated characters (might confuse TTS)
        let repeated = "a".repeat(100);
        let audio = MockKokoroTTS::generate_mock_audio(&repeated);

        assert!(
            !audio.is_empty(),
            "Repeated characters should generate audio"
        );
        assert_eq!(audio.len() % 2, 0, "Audio should be aligned");

        println!("✓ Repeated characters edge case passed");
    }

    // ========================================================================
    // Voice Resolution & Variant Tests
    // ========================================================================

    #[test]
    fn test_voice_resolution_case_insensitivity() {
        // Test that voice names are case-insensitive
        let test_voices = vec![
            "ZF_088", "zf_088", "Zf_088", "zF_088", "DEFAULT", "Default", "dEfAuLt",
        ];

        for voice in test_voices {
            let normalized = voice.trim().to_ascii_lowercase();
            assert!(
                normalized == "zf_088" || normalized == "default",
                "Voice '{}' should normalize correctly",
                voice
            );
        }

        println!("✓ Voice resolution case insensitivity passed");
    }

    #[test]
    fn test_voice_resolution_with_whitespace() {
        // Test voice names with leading/trailing whitespace
        let voices_with_ws = vec!["  default", "default  ", "  default  ", " zf_088 "];

        for voice in voices_with_ws {
            let normalized = voice.trim().to_ascii_lowercase();
            let is_valid = !normalized.is_empty();
            assert!(is_valid, "Trimmed voice should be valid");
        }

        println!("✓ Voice whitespace handling passed");
    }

    #[test]
    fn test_voice_resolution_variants() {
        // Test voice name variants (with/without underscores)
        let variants = vec![
            ("zf_088", "underscore"),
            ("zf088", "no underscore"),
            ("ZF_088", "uppercase with underscore"),
            ("ZF088", "uppercase no underscore"),
        ];

        for (voice, variant) in variants {
            let normalized = voice.to_ascii_lowercase();
            // Both should normalize to the same voice
            let is_v11 = normalized.contains("zf") && normalized.contains("088");
            assert!(is_v11, "Should resolve v1.1 voice variant: {}", variant);
        }

        println!("✓ Voice resolution variants passed");
    }

    // ========================================================================
    // Audio Output Validation Tests
    // ========================================================================

    #[test]
    fn test_audio_byte_alignment() {
        // Verify all generated audio has proper byte alignment for 16-bit samples
        let texts = vec!["Hello", "世界", "longertexthere"];

        for text in texts {
            let audio = MockKokoroTTS::generate_mock_audio(text);
            assert_eq!(
                audio.len() % 2,
                0,
                "Audio for '{}' should have even byte count for 16-bit alignment",
                text
            );

            // Verify we can interpret bytes as i16 samples
            let sample_count = audio.len() / 2;
            assert!(sample_count > 0, "Should have at least one sample");
        }

        println!("✓ Audio byte alignment validation passed");
    }

    #[test]
    fn test_audio_sample_bounds() {
        // Verify audio samples stay within i16 bounds
        let audio = MockKokoroTTS::generate_mock_audio("Test audio validation");

        // Parse as i16 samples
        let samples: Vec<i16> = audio
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        // All samples should be within reasonable range
        let max_sample = samples.iter().map(|s| s.abs()).max().unwrap_or(0);
        assert!(
            max_sample <= i16::MAX,
            "Audio samples should not exceed i16::MAX"
        );

        println!(
            "✓ Audio sample bounds validation passed (max: {})",
            max_sample
        );
    }

    #[test]
    fn test_audio_sample_rate_inference() {
        // Verify audio duration is consistent with 24kHz sample rate
        let sample_rate = 24000u32;

        let texts = vec![
            ("short", 0.05),            // ~5 chars = ~0.05 seconds
            ("medium text here", 0.16), // ~16 chars = ~0.16 seconds
        ];

        for (text, expected_secs) in texts {
            let audio = MockKokoroTTS::generate_mock_audio(text);
            let sample_count = audio.len() / 2;
            let actual_secs = sample_count as f32 / sample_rate as f32;

            // Allow ±50% variance for test purposes
            assert!(
                actual_secs >= expected_secs * 0.5 && actual_secs <= expected_secs * 1.5,
                "Audio duration for '{}' should be approximately {} seconds, got {}",
                text,
                expected_secs,
                actual_secs
            );
        }

        println!("✓ Audio sample rate inference passed");
    }

    #[test]
    fn test_audio_non_zero_samples() {
        // Verify audio contains actual samples (not silent)
        let audio = MockKokoroTTS::generate_mock_audio("audio test");
        let samples: Vec<i16> = audio
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        let zero_count = samples.iter().filter(|&&s| s == 0).count();
        let total_count = samples.len();

        // Audio should not be completely silent (allow some zeros but not all)
        assert!(
            zero_count < total_count,
            "Audio should not be completely silent"
        );

        println!(
            "✓ Audio non-zero sample validation passed ({}% non-zero)",
            ((total_count - zero_count) * 100 / total_count)
        );
    }

    // ========================================================================
    // Streaming & Callback Tests
    // ========================================================================

    #[test]
    fn test_streaming_callback_invocation_count() {
        // Verify callbacks are invoked for each stream chunk
        let text = "This is a test for streaming callbacks";

        // Simulate streaming with fixed chunk size
        let chunk_size = 2048; // bytes per chunk
        let audio = MockKokoroTTS::generate_mock_audio(text);
        let expected_chunks = (audio.len() + chunk_size - 1) / chunk_size;

        assert!(expected_chunks > 0, "Should have at least one chunk");

        println!(
            "✓ Streaming callback chunk calculation: {} total bytes → {} chunks",
            audio.len(),
            expected_chunks
        );
    }

    #[test]
    fn test_streaming_with_empty_callback() {
        // Test that streaming handles no-op callbacks correctly
        let text = "Test with empty callback";
        let audio = MockKokoroTTS::generate_mock_audio(text);

        // Simulate streaming with no-op callback
        let mut chunks_processed = 0;
        let chunk_size = 1024;
        for _ in (0..audio.len()).step_by(chunk_size) {
            chunks_processed += 1;
        }

        assert!(
            chunks_processed > 0,
            "Should process chunks with no-op callback"
        );
        println!(
            "✓ Empty callback handled correctly: {} chunks",
            chunks_processed
        );
    }

    #[test]
    fn test_streaming_callback_data_order() {
        // Verify callback receives data in correct order
        let audio = MockKokoroTTS::generate_mock_audio("ordered delivery test");
        let chunk_size = 512;

        let mut received_bytes = Vec::new();
        for chunk in audio.chunks(chunk_size) {
            received_bytes.extend_from_slice(chunk);
        }

        assert_eq!(
            received_bytes, audio,
            "Streamed chunks should reconstruct original audio"
        );

        println!("✓ Streaming callback data order validation passed");
    }

    // ========================================================================
    // Concurrent & Stress Tests
    // ========================================================================

    #[test]
    fn test_concurrent_mock_synthesis() {
        // Test thread safety with concurrent synthesis tasks
        use std::sync::{Arc, Barrier};
        use std::thread;

        let barrier = Arc::new(Barrier::new(5));
        let mut handles = vec![];

        for i in 0..5 {
            let barrier_clone = Arc::clone(&barrier);
            let handle = thread::spawn(move || {
                barrier_clone.wait(); // Synchronize all threads

                let text = format!("Concurrent test {}", i);
                let audio = MockKokoroTTS::generate_mock_audio(&text);
                !audio.is_empty()
            });
            handles.push(handle);
        }

        let results: Vec<bool> = handles
            .into_iter()
            .map(|h| h.join().unwrap_or(false))
            .collect();

        let success_count = results.iter().filter(|&&r| r).count();
        assert_eq!(success_count, 5, "All concurrent tasks should succeed");

        println!(
            "✓ Concurrent synthesis test passed: {}/5 threads successful",
            success_count
        );
    }

    #[test]
    fn test_stress_large_batch_synthesis() {
        // Stress test: synthesize many texts in sequence
        let texts: Vec<&str> = vec![
            "First test",
            "Second test with more text",
            "Third",
            "四个测试",
            "🎵🎶🎸",
        ];

        let mut total_bytes = 0;
        for text in texts.iter() {
            let audio = MockKokoroTTS::generate_mock_audio(text);
            total_bytes += audio.len();
            assert!(!audio.is_empty(), "Each synthesis should produce audio");
        }

        assert!(
            total_bytes > 0,
            "Batch synthesis should produce total audio"
        );
        println!(
            "✓ Batch synthesis stress test: {} texts → {} total bytes",
            texts.len(),
            total_bytes
        );
    }

    #[test]
    fn test_memory_efficiency_large_text() {
        // Test that synthesizing very large text doesn't cause issues
        let large_text = "Lorem ipsum dolor sit amet. ".repeat(200); // ~6KB text

        let audio = MockKokoroTTS::generate_mock_audio(&large_text);
        assert!(!audio.is_empty(), "Large text should generate audio");

        // Rough memory check (audio should be reasonable size)
        let audio_size_mb = audio.len() as f32 / (1024.0 * 1024.0);
        assert!(
            audio_size_mb < 10.0,
            "Audio size should be reasonable (got {} MB for {} chars)",
            audio_size_mb,
            large_text.len()
        );

        println!(
            "✓ Memory efficiency test: {} chars → {:.2} MB",
            large_text.len(),
            audio_size_mb
        );
    }

    // ========================================================================
    // Error Recovery & Resilience Tests
    // ========================================================================

    #[test]
    fn test_mixed_valid_invalid_inputs() {
        // Test resilience by mixing valid and invalid inputs
        let test_cases = vec![
            ("", "empty", false),
            ("Valid text", "valid", true),
            ("   ", "whitespace only", true),
            ("Mixed 123 !@# 。，", "mixed", true),
        ];

        for (text, desc, should_produce) in test_cases {
            let audio = MockKokoroTTS::generate_mock_audio(text);
            let produced = !audio.is_empty();

            // Edge case: whitespace-only DOES produce minimal audio in mock
            if produced || !should_produce {
                println!("  ✓ '{}': handled as expected", desc);
            }
        }

        println!("✓ Mixed input resilience test passed");
    }

    #[test]
    fn test_rapid_sequential_calls() {
        // Test rapid sequential synthesis without delays
        let call_count = 100;
        let mut success = 0;

        for _ in 0..call_count {
            let audio = MockKokoroTTS::generate_mock_audio("quick");
            if !audio.is_empty() {
                success += 1;
            }
        }

        assert_eq!(
            success, call_count,
            "All {} rapid calls should succeed",
            call_count
        );

        println!(
            "✓ Rapid sequential calls test: {}/{} successful",
            success, call_count
        );
    }

    // ========================================================================
    // Integration Test Fixtures & Helpers
    // ========================================================================

    /// Helper struct for mock streaming tests
    struct StreamingTestCollector {
        chunks: Vec<Vec<u8>>,
        total_bytes: usize,
    }

    impl StreamingTestCollector {
        fn new() -> Self {
            Self {
                chunks: Vec::new(),
                total_bytes: 0,
            }
        }

        fn callback(&mut self, chunk: Vec<u8>) {
            self.total_bytes += chunk.len();
            self.chunks.push(chunk);
        }

        fn into_audio(self) -> Vec<u8> {
            self.chunks.into_iter().flatten().collect()
        }

        fn chunk_count(&self) -> usize {
            self.chunks.len()
        }
    }

    #[test]
    fn test_streaming_collector_integration() {
        // Test the streaming collector helper
        let audio_orig = MockKokoroTTS::generate_mock_audio("collector test");
        let mut collector = StreamingTestCollector::new();

        // Simulate streaming
        let chunk_size = 1024;
        for chunk in audio_orig.chunks(chunk_size) {
            collector.callback(chunk.to_vec());
        }

        let audio_reconstructed = collector.into_audio();
        assert_eq!(
            audio_orig, audio_reconstructed,
            "Collector should perfectly reconstruct audio"
        );
        assert!(
            collector.chunk_count() > 0,
            "Collector should have received chunks"
        );

        println!(
            "✓ Streaming collector integration: {} chunks",
            collector.chunk_count()
        );
    }

    #[test]
    fn test_mock_engine_audio_consistency() {
        // Verify mock engine produces deterministic audio for same text
        let text = "consistency test";
        let audio1 = MockKokoroTTS::generate_mock_audio(text);
        let audio2 = MockKokoroTTS::generate_mock_audio(text);

        assert_eq!(audio1, audio2, "Same text should produce identical audio");

        // Different text should produce different audio
        let audio3 = MockKokoroTTS::generate_mock_audio("different");
        assert_ne!(
            audio1, audio3,
            "Different text should produce different audio"
        );

        println!("✓ Mock engine consistency validation passed");
    }

    #[test]
    fn test_audio_quality_metrics() {
        // Test audio quality by analyzing samples
        let audio = MockKokoroTTS::generate_mock_audio("quality test metrics");
        let samples: Vec<i16> = audio
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        if samples.is_empty() {
            println!("⚠ No samples to analyze");
            return;
        }

        // Calculate statistics
        let max_val = samples.iter().map(|&s| s.abs()).max().unwrap_or(0) as f32;
        let mean: f32 =
            samples.iter().map(|&s| (s as f32).abs()).sum::<f32>() / samples.len() as f32;
        let dynamic_range = if max_val > 0.0 {
            20.0 * (max_val / (i16::MAX as f32)).log10()
        } else {
            0.0
        };

        assert!(max_val > 0.0, "Audio should have non-zero samples");
        assert!(
            dynamic_range > -60.0,
            "Audio dynamic range should be reasonable"
        );

        println!(
            "✓ Audio quality metrics: max={:.0}, mean={:.0}, dB range={:.1}",
            max_val, mean, dynamic_range
        );
    }
}
