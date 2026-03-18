//! Voice Pipeline — end-to-end Audio → ASR → LLM → TTS → Audio
//!
//! Chains three adapters into a unified pipeline for voice-to-voice
//! agent interaction.
//!
//! ```text
//!   AudioInput ─→ ASR ─→ text ─→ LLM ─→ reply ─→ TTS ─→ AudioOutput
//! ```

use mofa_kernel::agent::{AgentError, AgentResult};
use mofa_kernel::llm::provider::LLMProvider;
use mofa_kernel::llm::types::{ChatCompletionRequest, ChatMessage};
use mofa_kernel::speech::{
    AsrAdapter, AsrConfig, AudioFormat, AudioOutput, TranscriptionResult, TtsAdapter, TtsConfig,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

// ============================================================================
// Pipeline Configuration
// ============================================================================

/// Configuration for the voice pipeline.
#[derive(Debug, Clone)]
pub struct VoicePipelineConfig {
    /// Default voice to use for TTS output.
    pub voice: String,
    /// TTS-specific config (format, speed, language).
    pub tts_config: TtsConfig,
    /// ASR-specific config (language, timestamps).
    pub asr_config: AsrConfig,
    /// System prompt injected before the user transcript.
    pub system_prompt: Option<String>,
    /// Optional maximum LLM reply length hint, forwarded to the LLM as `max_tokens`.
    /// This is a token-based limit; the pipeline itself does not perform additional truncation.
    pub max_reply_tokens: Option<usize>,
}

impl Default for VoicePipelineConfig {
    fn default() -> Self {
        Self {
            voice: "alloy".to_string(),
            tts_config: TtsConfig::new(),
            asr_config: AsrConfig::new(),
            system_prompt: None,
            max_reply_tokens: None,
        }
    }
}

impl VoicePipelineConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        self.voice = voice.into();
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_tts_config(mut self, config: TtsConfig) -> Self {
        self.tts_config = config;
        self
    }

    pub fn with_asr_config(mut self, config: AsrConfig) -> Self {
        self.asr_config = config;
        self
    }
}

// ============================================================================
// Pipeline Result
// ============================================================================

/// Full result from a voice pipeline invocation.
#[derive(Debug)]
pub struct VoicePipelineResult {
    /// The ASR transcription of the input audio.
    pub transcription: TranscriptionResult,
    /// The LLM's text reply.
    pub llm_reply: String,
    /// The TTS audio output of the LLM reply.
    pub audio_output: AudioOutput,
}

// ============================================================================
// Voice Pipeline
// ============================================================================

/// End-to-end voice pipeline: Audio → ASR → LLM → TTS → Audio.
pub struct VoicePipeline {
    asr: Arc<dyn AsrAdapter>,
    llm: Arc<dyn LLMProvider>,
    tts: Arc<dyn TtsAdapter>,
    config: VoicePipelineConfig,
}

impl VoicePipeline {
    /// Create a new voice pipeline with the given adapters.
    pub fn new(
        asr: Arc<dyn AsrAdapter>,
        llm: Arc<dyn LLMProvider>,
        tts: Arc<dyn TtsAdapter>,
        config: VoicePipelineConfig,
    ) -> Self {
        Self {
            asr,
            llm,
            tts,
            config,
        }
    }

    /// Run the full pipeline: Audio → ASR → LLM → TTS → Audio.
    pub async fn process(&self, audio_input: &[u8]) -> AgentResult<VoicePipelineResult> {
        // ── Stage 1: ASR ─────────────────────────────────────────────────
        info!(
            "[voice-pipeline] stage 1/3: ASR ({}) — {} bytes",
            self.asr.name(),
            audio_input.len()
        );
        let transcription = self
            .asr
            .transcribe(audio_input, &self.config.asr_config)
            .await
            .map_err(|e| {
                error!("[voice-pipeline] ASR failed: {}", e);
                AgentError::Other(format!("Voice pipeline ASR stage failed: {}", e))
            })?;

        debug!("[voice-pipeline] ASR result: \"{}\"", transcription.text);

        if transcription.text.trim().is_empty() {
            return Err(AgentError::Other(
                "Voice pipeline: ASR produced empty transcription".to_string(),
            ));
        }

        // ── Stage 2: LLM ─────────────────────────────────────────────────
        info!(
            "[voice-pipeline] stage 2/3: LLM ({}) — {} chars input",
            self.llm.name(),
            transcription.text.len()
        );

        let mut request = ChatCompletionRequest::new(self.llm.default_model());
        if let Some(sys) = &self.config.system_prompt {
            request = request.system(sys.clone());
        }
        request = request.user(transcription.text.clone());
        if let Some(max) = self.config.max_reply_tokens {
            request = request.max_tokens(max as u32);
        }
        request = request.temperature(0.7);

        let response = self.llm.chat(request).await.map_err(|e| {
            error!("[voice-pipeline] LLM failed: {}", e);
            AgentError::Other(format!("Voice pipeline LLM stage failed: {}", e))
        })?;

        let llm_reply = response.content().unwrap_or("").to_string();

        debug!("[voice-pipeline] LLM reply: \"{}\"", llm_reply);

        if llm_reply.trim().is_empty() {
            warn!("[voice-pipeline] LLM produced empty reply");
        }

        // ── Stage 3: TTS ─────────────────────────────────────────────────
        info!(
            "[voice-pipeline] stage 3/3: TTS ({}) — {} chars, voice={}",
            self.tts.name(),
            llm_reply.len(),
            self.config.voice,
        );

        let audio_output = self
            .tts
            .synthesize(&llm_reply, &self.config.voice, &self.config.tts_config)
            .await
            .map_err(|e| {
                error!("[voice-pipeline] TTS failed: {}", e);
                AgentError::Other(format!("Voice pipeline TTS stage failed: {}", e))
            })?;

        info!(
            "[voice-pipeline] complete — {} bytes output audio",
            audio_output.data.len()
        );

        Ok(VoicePipelineResult {
            transcription,
            llm_reply,
            audio_output,
        })
    }

    /// Get the ASR adapter name.
    pub fn asr_name(&self) -> &str {
        self.asr.name()
    }

    /// Get the LLM provider name.
    pub fn llm_name(&self) -> &str {
        self.llm.name()
    }

    /// Get the TTS adapter name.
    pub fn tts_name(&self) -> &str {
        self.tts.name()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mofa_kernel::llm::types::{ChatCompletionResponse, Choice, FinishReason};
    use mofa_kernel::speech::*;

    // ---- Mock ASR ----
    struct MockAsr;

    #[async_trait]
    impl AsrAdapter for MockAsr {
        fn name(&self) -> &str {
            "mock-asr"
        }

        async fn transcribe(
            &self,
            _audio: &[u8],
            _config: &AsrConfig,
        ) -> AgentResult<TranscriptionResult> {
            Ok(TranscriptionResult::text_only("What is the weather today?"))
        }
    }

    // ---- Mock LLM ----
    struct MockLlm;

    #[async_trait]
    impl LLMProvider for MockLlm {
        fn name(&self) -> &str {
            "mock-llm"
        }

        async fn chat(
            &self,
            _request: ChatCompletionRequest,
        ) -> AgentResult<ChatCompletionResponse> {
            Ok(ChatCompletionResponse {
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage::assistant("It's sunny and 25°C today!"),
                    finish_reason: Some(FinishReason::Stop),
                    logprobs: None,
                }],
            })
        }
    }

    // ---- Mock TTS ----
    struct MockTts;

    #[async_trait]
    impl TtsAdapter for MockTts {
        fn name(&self) -> &str {
            "mock-tts"
        }

        async fn synthesize(
            &self,
            text: &str,
            _voice: &str,
            _config: &TtsConfig,
        ) -> AgentResult<AudioOutput> {
            let fake_audio = vec![0u8; text.len() * 10];
            Ok(AudioOutput::new(fake_audio, AudioFormat::Wav, 24000))
        }

        async fn list_voices(&self) -> AgentResult<Vec<VoiceDescriptor>> {
            Ok(vec![VoiceDescriptor::new("default", "Default", "en-US")])
        }
    }

    // ---- Pipeline tests ----

    #[tokio::test]
    async fn full_pipeline_with_mocks() {
        let pipeline = VoicePipeline::new(
            Arc::new(MockAsr),
            Arc::new(MockLlm),
            Arc::new(MockTts),
            VoicePipelineConfig::new().with_voice("default"),
        );

        let result = pipeline.process(&[0u8; 100]).await.unwrap();

        assert_eq!(result.transcription.text, "What is the weather today?");
        assert_eq!(result.llm_reply, "It's sunny and 25°C today!");
        assert!(!result.audio_output.data.is_empty());
        assert_eq!(result.audio_output.format, AudioFormat::Wav);
    }

    #[tokio::test]
    async fn pipeline_with_system_prompt() {
        let pipeline = VoicePipeline::new(
            Arc::new(MockAsr),
            Arc::new(MockLlm),
            Arc::new(MockTts),
            VoicePipelineConfig::new()
                .with_voice("alloy")
                .with_system_prompt("You are a helpful weather assistant."),
        );

        let result = pipeline.process(&[0u8; 50]).await.unwrap();
        assert!(!result.llm_reply.is_empty());
    }

    #[test]
    fn pipeline_adapter_names() {
        let pipeline = VoicePipeline::new(
            Arc::new(MockAsr),
            Arc::new(MockLlm),
            Arc::new(MockTts),
            VoicePipelineConfig::default(),
        );

        assert_eq!(pipeline.asr_name(), "mock-asr");
        assert_eq!(pipeline.llm_name(), "mock-llm");
        assert_eq!(pipeline.tts_name(), "mock-tts");
    }

    #[test]
    fn config_builders() {
        let cfg = VoicePipelineConfig::new()
            .with_voice("nova")
            .with_system_prompt("You are an agent.");
        assert_eq!(cfg.voice, "nova");
        assert_eq!(cfg.system_prompt.as_deref(), Some("You are an agent."));
    }
}
