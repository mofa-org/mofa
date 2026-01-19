//! Kokoro TTS Engine Wrapper
//!
//! This module provides a wrapper around the kokoro-tts library
//! that implements the TTSEngine trait for integration with MoFA.

use super::{TTSEngine, VoiceInfo};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use kokoro_tts::{KokoroTts, Voice, SynthStream, SynthSink};
use std::collections::HashMap;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Kokoro TTS engine implementation
///
/// Wraps the kokoro-tts library to provide text-to-speech capabilities
/// with support for multiple voices and streaming output.
pub struct KokoroTTS {
    /// The underlying Kokoro TTS instance
    tts: Arc<Mutex<KokoroTts>>,
    /// Default voice to use for synthesis
    default_voice: Voice,
}

/// Clone implementation for KokoroTTS
///
/// Since KokoroTTS internally uses Arc<Mutex<KokoroTts>>,
/// cloning is cheap (just increments the Arc reference count).
impl Clone for KokoroTTS {
    fn clone(&self) -> Self {
        Self {
            tts: Arc::clone(&self.tts),
            default_voice: self.default_voice.clone(),
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
    pub async fn new(model_path: &str, voice_path: &str) -> Result<Self, anyhow::Error> {
        info!(
            "Initializing Kokoro TTS with model: {}, voices: {}",
            model_path, voice_path
        );

        // Validate file paths exist
        if !Path::new(model_path).exists() {
            return Err(anyhow::anyhow!(
                "Kokoro model file not found: {}",
                model_path
            ));
        }
        if !Path::new(voice_path).exists() {
            return Err(anyhow::anyhow!(
                "Kokoro voices file not found: {}",
                voice_path
            ));
        }

        // Initialize Kokoro TTS
        let tts = KokoroTts::new(model_path, voice_path).await.map_err(|e| {
            anyhow::anyhow!("Failed to initialize Kokoro TTS: {:?}", e)
        })?;

        // Set default voice
        let default_voice = Voice::AfMaple(1);

        Ok(Self {
            tts: Arc::new(Mutex::new(tts)),
            default_voice,
        })
    }

    /// Get the default voice
    pub fn default_voice_name(&self) -> &str {
        "default"
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
    ) -> Result<(SynthSink<String>,SynthStream), anyhow::Error> {
        let voice_enum = Voice::from_name(voice);
        let tts = self.tts.lock().await;
        let (sink, stream) = tts.stream::<String>(voice_enum);
        Ok((sink,stream))
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
            voice,
            text
        );

        let voice_enum = Voice::from_name(voice);

        // Create a new stream for this synthesis
        let tts = self.tts.lock().await;
        let (mut sink, mut stream) = tts.stream::<String>(voice_enum);

        // Submit text for synthesis
        sink.synth(text.to_string()).await.map_err(|e| {
            anyhow::anyhow!("Failed to submit text for f32 streaming: {:?}", e)
        })?;

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
    async fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, anyhow::Error> {
        debug!(
            "[KokoroTTS] Synthesizing text with voice '{}': {}",
            voice,
            text
        );

        let voice = Voice::from_name(voice);

        // Collect all audio chunks - we need to keep the tts lock alive while iterating
        let all_audio_f32 = {
            let tts = self.tts.lock().await;
            let (mut sink, mut stream) = tts.stream::<String>(voice);

            // Submit text for synthesis using the synth method
            sink.synth(text.to_string()).await.map_err(|e| {
                anyhow::anyhow!("Failed to submit text for synthesis: {:?}", e)
            })?;

            // Collect all audio chunks while tts is still locked
            let mut audio = Vec::new();
            while let Some((chunk, _took)) = stream.next().await {
                audio.extend_from_slice(&chunk);
            }
            audio
        };

        // Convert f32 samples (-1.0 to 1.0) to i16 samples
        let audio_i16: Vec<i16> = all_audio_f32
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
    async fn synthesize_stream(
        &self,
        text: &str,
        voice: &str,
        callback: Box<dyn Fn(Vec<u8>) + Send + Sync>,
    ) -> Result<(), anyhow::Error> {
        debug!(
            "[KokoroTTS] Stream synthesizing text with voice '{}': {}",
            voice,
            text
        );

        let voice_enum = Voice::from_name(voice);

        // Create a new stream for this synthesis
        let tts = self.tts.lock().await;
        let (mut sink, mut stream) = tts.stream::<String>(voice_enum);

        // Submit text for synthesis
        sink.synth(text.to_string()).await.map_err(|e| {
            anyhow::anyhow!("Failed to submit text for streaming synthesis: {:?}", e)
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
