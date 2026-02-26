//! Kokoro TTS Engine Wrapper
//!
//! This module provides a complete wrapper around the kokoro-tts library
//! that implements the TTSEngine trait for integration with MoFA.
//!
//! # Overview
//!
//! The Kokoro TTS engine provides high-quality text-to-speech synthesis with
//! support for multiple voices and languages. It uses ONNX models for efficient
//! inference and supports both synchronous and streaming synthesis.
//!
//! # Supported Audio Format
//!
//! **Output Format**: PCM WAV (16-bit signed, mono)
//! - Sample Rate: 24000 Hz
//! - Channels: 1 (mono)
//! - Bit Depth: 16-bit signed integer
//! - Byte Order: Little-endian
//!
//! # Supported Voices
//!
//! ## Model Version Detection
//!
//! Kokoro TTS supports two model versions with different voice sets:
//!
//! ### Version 1.1 (v1.1) Voices
//! - `zf_088` - Female voice (default for v1.1)
//! - `zf_090` - Alternative female voice
//! - More voices: `zf_092`, `zf_094`, etc. (check your model)
//!
//! ### Version 1.0 (v1.0) Voices
//! - `zf_xiaoxiao` - Female voice (default for v1.0)
//! - `zf_xiaoyi` - Alternative female voice
//! - `zm_yunyang` - Male voice
//! - `zf_xiaobei` - Female voice variant
//! - `zf_xiaoni` - Female voice variant
//!
//! ### Special Voice Names
//! - `"default"` - Uses the model-appropriate default voice
//! - Case-insensitive matching (e.g., `"ZF_088"`, `"zf088"`)
//! - Underscores optional (e.g., `"zf088"` same as `"zf_088"`)
//!
//! # Required Model Files
//!
//! To use the Kokoro TTS engine, you need:
//! 1. ONNX model file (e.g., `kokoro-v1.1-zh.onnx`)
//! 2. Voice embeddings file (e.g., `voices-v1.1-zh.bin`)
//!
//! Both files must be in the same directory or you must provide correct paths.
//!
//! # Error Handling
//!
//! The wrapper provides clear error messages for:
//! - Missing model files (with path suggestions)
//! - Invalid voice names (falls back to default with warning)
//! - Text submission failures
//! - Stream submission failures
//! - Unknown voice selections
//!
//! # Examples
//!
//! ```rust,ignore
//! use mofa_plugins::tts::kokoro_wrapper::KokoroTTS;
//!
//! // Initialize with model paths
//! let kokoro = KokoroTTS::new(
//!     "kokoro-v1.1-zh.onnx",
//!     "voices-v1.1-zh.bin"
//! ).await?;
//!
//! // Synchronous synthesis
//! let audio_bytes = kokoro.synthesize("你好，世界", "zf_088").await?;
//! println!("Generated {} bytes of audio", audio_bytes.len());
//!
//! // Streaming synthesis with callback
//! kokoro.synthesize_stream(
//!     "这是一个长文本...",
//!     "default",
//!     Box::new(|chunk: Vec<u8>| {
//!         println!("Received {} bytes", chunk.len());
//!     })
//! ).await?;
//! ```

use super::{TTSEngine, VoiceInfo};
use mofa_kernel::plugin::{PluginError, PluginResult};use futures::StreamExt;
pub use kokoro_tts::{KokoroTts, SynthSink, SynthStream, Voice};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

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
            default_voice: self.default_voice,
            is_v11_model: self.is_v11_model,
        }
    }
}

impl KokoroTTS {
    /// Create a new Kokoro TTS wrapper
    ///
    /// # Arguments
    /// - `model_path`: Path to the ONNX model file (e.g., "kokoro-v1.1-zh.onnx")
    /// - `voice_path`: Path to the voices binary file (e.g., "voices-v1.1-zh.bin")
    ///
    /// # Errors
    /// Returns an error if model initialization fails
    pub async fn new(model_path: &str, voice_path: &str) -> PluginResult<Self> {
        info!(
            "Initializing Kokoro TTS with model: {}, voices: {}",
            model_path, voice_path
        );

        // Validate file paths exist
        if !Path::new(model_path).exists() {
            return Err(PluginError::Other(format!(
                "Kokoro model file not found: {}",
                model_path
            )));
        }
        if !Path::new(voice_path).exists() {
            return Err(PluginError::Other(format!(
                "Kokoro voices file not found: {}",
                voice_path
            )));
        }

        // Initialize Kokoro TTS
        let tts = KokoroTts::new(model_path, voice_path)
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Failed to initialize Kokoro TTS: {:?}", e)))?;

        let model_lower = model_path.to_ascii_lowercase();
        let is_v11_model = model_lower.contains("v1.1") || model_lower.contains("v11");

        // Set default voice based on model version
        let default_voice = if is_v11_model {
            Voice::Zf088(1)
        } else {
            Voice::ZfXiaoxiao(1.0)
        };

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
    ) -> PluginResult<(SynthSink<String>, SynthStream)> {
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
    ) -> PluginResult<()> {
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
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Failed to submit text for f32 streaming: {:?}", e)))?;

        // Process audio chunks and call callback for each chunk
        while let Some((audio_f32, _took)) = stream.next().await {
            callback(audio_f32);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl TTSEngine for KokoroTTS {
    /// Synthesize text to audio data (synchronous version)
    ///
    /// This method synthesizes the entire input text and returns all audio data at once.
    /// It's suitable for shorter texts or when you need the complete audio immediately.
    ///
    /// # Arguments
    /// - `text`: The text to synthesize. Cannot be empty. It will be sent to the Kokoro
    ///   inference engine for synthesis.
    /// - `voice`: Voice identifier (e.g., "default", "zf_088", "zf_xiaoxiao").
    ///   Case-insensitive and underscore-tolerant. Unrecognized voices fallback to default
    ///   with a debug log warning.
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)`: Raw PCM audio data in WAV format (16-bit, mono, 24000 Hz)
    /// - `Err`: If text submission fails, synthesis fails, or audio conversion fails
    ///
    /// # Error Cases
    /// - Empty text: Will return error from synthesis
    /// - Invalid voice: Falls back to default voice (logged with debug level)
    /// - Network/system errors: Propagated with context
    /// - Invalid UTF-8 in text: Handled by underlying library
    ///
    /// # Performance Notes
    /// - Entire text is synthesized at once, blocking until complete
    /// - Large texts will create proportionally larger audio data
    /// - For streaming/chunked responses, use `synthesize_stream()` instead
    ///
    /// # Example
    /// ```rust,ignore
    /// let audio = kokoro.synthesize("Hello world", "default").await?;
    /// println!("Audio size: {} bytes", audio.len());
    /// ```
    async fn synthesize(&self, text: &str, voice: &str) -> PluginResult<Vec<u8>> {
        debug!(
            "[KokoroTTS] Synthesizing text with voice '{}': {}",
            voice, text
        );

        let voice = self.resolve_voice(voice);

        // Create a stream for this synthesis
        let (mut sink, mut stream) = self.tts.stream::<String>(voice);

        // Submit text for synthesis using the synth method
        sink.synth(text.to_string())
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Failed to submit text for synthesis: {:?}", e)))?;

        // Collect all audio chunks
        let mut audio = Vec::new();
        while let Some((chunk, _took)) = stream.next().await {
            audio.extend_from_slice(&chunk);
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

        // Convert i16 samples to u8 bytes
        let audio_bytes: Vec<u8> = audio_i16
            .iter()
            .flat_map(|&sample| sample.to_le_bytes())
            .collect();

        Ok(audio_bytes)
    }

    /// Synthesize text with streaming callback
    ///
    /// This method is ideal for long texts where you want to process audio as it's
    /// generated rather than waiting for full synthesis. The callback is called multiple
    /// times as audio chunks become available.
    ///
    /// # Arguments
    /// - `text`: The text to synthesize. Cannot be empty.
    /// - `voice`: Voice identifier (same format as `synthesize()`)
    /// - `callback`: Function called for each audio chunk. Called with Vec<u8> containing
    ///   16-bit PCM WAV data. This may be called multiple times.
    ///
    /// # Returns
    /// - `Ok(())`: Streaming synthesis completed successfully
    /// - `Err`: If text submission fails, synthesis fails, or streaming fails
    ///
    /// # Error Cases
    /// - Empty text: Returns error, callback not called
    /// - Invalid voice: Falls back to default voice
    /// - Stream failures: Propagated as error
    /// - Callback panics: Propagated as error
    ///
    /// # Performance Notes
    /// - Callback is invoked multiple times as chunks complete
    /// - Good for long texts (books, articles, etc.)
    /// - Allows real-time audio playback
    /// - Lower memory footprint than `synthesize()`
    ///
    /// # Example
    /// ```rust,ignore
    /// # use std::sync::Arc;
    /// # let kokoro = unimplemented!();
    /// let chunk_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    /// let chunk_count_clone = chunk_count.clone();
    ///
    /// kokoro.synthesize_stream(
    ///     "This is a long text...",
    ///     "default",
    ///     Box::new(move |chunk: Vec<u8>| {
    ///         println!("Received chunk: {} bytes", chunk.len());
    ///         chunk_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    ///     })
    /// ).await?;
    ///
    /// println!("Total chunks: {}", chunk_count.load(std::sync::atomic::Ordering::SeqCst));
    /// ```
    async fn synthesize_stream(
        &self,
        text: &str,
        voice: &str,
        callback: Box<dyn Fn(Vec<u8>) + Send + Sync>,
    ) -> PluginResult<()> {
        debug!(
            "[KokoroTTS] Stream synthesizing text with voice '{}': {}",
            voice, text
        );

        let voice_enum = self.resolve_voice(voice);

        // Create a new stream for this synthesis
        let (mut sink, mut stream) = self.tts.stream::<String>(voice_enum);

        // Submit text for synthesis
        sink.synth(text.to_string()).await.map_err(|e| {
            mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Failed to submit text for streaming synthesis: {:?}", e))
        })?;

        // Process audio chunks and call callback for each chunk
        while let Some((audio_f32, _took)) = stream.next().await {
            // Convert f32 samples (-1.0 to 1.0) to i16 samples
            let audio_i16: Vec<i16> = audio_f32
                .iter()
                .map(|&sample| {
                    let clamped = sample.clamp(-1.0, 1.0);
                    (clamped * i16::MAX as f32) as i16
                })
                .collect();

            // Convert i16 samples to u8 bytes
            let audio_bytes: Vec<u8> = audio_i16
                .iter()
                .flat_map(|&sample| sample.to_le_bytes())
                .collect();

            // Call the callback with the audio data
            callback(audio_bytes);
        }

        Ok(())
    }

    /// List available voices
    async fn list_voices(&self) -> PluginResult<Vec<VoiceInfo>> {
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

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_wrapper_creation() {
        let result = KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin").await;
        assert!(result.is_ok());

        let wrapper = result.unwrap();
        assert_eq!(wrapper.name(), "Kokoro");
    }

    #[tokio::test]
    #[ignore = "Requires model files"]
    async fn test_kokoro_list_voices() {
        let wrapper = KokoroTTS::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin")
            .await
            .unwrap();

        let voices = wrapper.list_voices().await.unwrap();
        assert!(!voices.is_empty());
        assert!(voices.iter().any(|v| v.id == "default"));
    }
}
