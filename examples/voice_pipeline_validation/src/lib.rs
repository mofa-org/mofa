//! Tiny end-to-end voice pipeline validation demo.
//!
//! This example exercises MoFA's shared ASR / LLM / TTS kernel traits with
//! mock adapters so proposal work can point to a runnable artifact instead of
//! only an API sketch.

use std::sync::Arc;

use anyhow::{Context, ensure};
use async_trait::async_trait;
use futures::{StreamExt, stream};
use mofa_kernel::agent::{AgentError, AgentResult};
use mofa_kernel::llm::provider::{ChatStream, LLMProvider};
use mofa_kernel::llm::types::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice,
    ChunkChoice, ChunkDelta, FinishReason,
};
use mofa_kernel::speech::{
    AsrAdapter, AsrConfig, AudioFormat, AudioOutput, TranscriptionResult, TtsAdapter, TtsConfig,
    VoiceDescriptor,
};
use tracing::info;

/// Runtime configuration for the validation demo.
#[derive(Debug, Clone)]
pub struct DemoConfig {
    /// Mock audio bytes fed into ASR.
    pub input_audio: Vec<u8>,
    /// Transcript returned by the mock ASR adapter.
    pub transcript: String,
    /// Chunks returned by the mock LLM stream.
    pub llm_chunks: Vec<String>,
    /// Voice identifier forwarded to the TTS adapter.
    pub voice: String,
    /// ASR configuration used by the pipeline.
    pub asr_config: AsrConfig,
    /// TTS configuration used by the pipeline.
    pub tts_config: TtsConfig,
    /// Number of bytes to queue per synthetic playback chunk.
    pub audio_chunk_size: usize,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            input_audio: b"mock-user-audio".to_vec(),
            transcript: "hello world".to_owned(),
            llm_chunks: vec![
                "Hello".to_owned(),
                " from".to_owned(),
                " the".to_owned(),
                " streaming".to_owned(),
                " pipeline.".to_owned(),
            ],
            voice: "validation-voice".to_owned(),
            asr_config: AsrConfig::new().with_language("en-US"),
            tts_config: TtsConfig::new().with_format(AudioFormat::Pcm),
            audio_chunk_size: 8,
        }
    }
}

/// Structured event emitted while the validation demo runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationEvent {
    kind: &'static str,
    message: String,
}

impl ValidationEvent {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Stable event kind used by tests and proposal writeups.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// Human-readable detail for logs or CLI output.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Runs the mock voice pipeline and returns the emitted validation events.
pub async fn run_validation_demo(config: DemoConfig) -> anyhow::Result<Vec<ValidationEvent>> {
    ValidationPipeline::mock(config).run().await
}

struct ValidationPipeline {
    asr: Arc<dyn AsrAdapter>,
    llm: Arc<dyn LLMProvider>,
    tts: Arc<dyn TtsAdapter>,
    config: DemoConfig,
}

impl ValidationPipeline {
    fn mock(config: DemoConfig) -> Self {
        let asr = Arc::new(MockAsrAdapter::new(config.transcript.clone()));
        let llm = Arc::new(MockLlmProvider::new(
            "mock-llm",
            "mock-streaming-model",
            config.llm_chunks.clone(),
        ));
        let tts = Arc::new(MockTtsAdapter::new("mock-tts", AudioFormat::Pcm, 24_000));

        Self {
            asr,
            llm,
            tts,
            config,
        }
    }

    async fn run(&self) -> anyhow::Result<Vec<ValidationEvent>> {
        ensure!(
            self.config.audio_chunk_size > 0,
            "audio_chunk_size must be greater than zero"
        );

        let mut events = Vec::new();
        let mut total_audio_bytes = 0usize;
        let mut first_audio_queued = false;
        let mut tts_stream_started = false;
        let mut llm_reply = String::new();

        push_event(
            &mut events,
            ValidationEvent::new(
                "asr_input_received",
                format!(
                    "ASR input received: {} bytes",
                    self.config.input_audio.len()
                ),
            ),
        );

        let transcription = self
            .asr
            .transcribe(&self.config.input_audio, &self.config.asr_config)
            .await
            .context("ASR transcription failed")?;

        ensure!(
            !transcription.text.trim().is_empty(),
            "ASR produced an empty transcript"
        );

        push_event(
            &mut events,
            ValidationEvent::new(
                "transcript_emitted",
                format!("Transcript emitted: {}", transcription.text),
            ),
        );

        let request = ChatCompletionRequest::new(self.llm.default_model())
            .user(transcription.text.clone())
            .stream();
        let mut stream = self
            .llm
            .chat_stream(request)
            .await
            .context("LLM streaming failed to start")?;

        push_event(
            &mut events,
            ValidationEvent::new(
                "llm_streaming_started",
                format!("LLM streaming started via {}", self.llm.name()),
            ),
        );

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("LLM stream yielded an error")?;
            for choice in chunk.choices {
                let Some(content) = choice.delta.content else {
                    continue;
                };

                if content.trim().is_empty() {
                    continue;
                }

                llm_reply.push_str(&content);
                push_event(
                    &mut events,
                    ValidationEvent::new(
                        "llm_chunk_received",
                        format!("LLM chunk received: {content:?}"),
                    ),
                );

                let audio = self
                    .tts
                    .synthesize(&content, &self.config.voice, &self.config.tts_config)
                    .await
                    .context("TTS synthesis failed")?;

                if !tts_stream_started {
                    tts_stream_started = true;
                    push_event(
                        &mut events,
                        ValidationEvent::new(
                            "tts_chunk_streaming_started",
                            format!("TTS chunk streaming started via {}", self.tts.name()),
                        ),
                    );
                }

                for (index, chunk_bytes) in
                    audio.data.chunks(self.config.audio_chunk_size).enumerate()
                {
                    total_audio_bytes += chunk_bytes.len();

                    if !first_audio_queued {
                        first_audio_queued = true;
                        push_event(
                            &mut events,
                            ValidationEvent::new(
                                "first_audio_queued",
                                format!(
                                    "First audio queued: {} bytes ready for playback",
                                    chunk_bytes.len()
                                ),
                            ),
                        );
                    }

                    push_event(
                        &mut events,
                        ValidationEvent::new(
                            "audio_chunk_queued",
                            format!(
                                "Audio chunk queued: {} bytes from response segment {}",
                                chunk_bytes.len(),
                                index + 1
                            ),
                        ),
                    );
                }
            }
        }

        ensure!(
            !llm_reply.is_empty(),
            "LLM stream produced no reply content"
        );
        ensure!(first_audio_queued, "TTS did not produce queued audio");

        push_event(
            &mut events,
            ValidationEvent::new(
                "completed",
                format!(
                    "Pipeline completed: transcript={:?}, reply_chars={}, queued_audio_bytes={}",
                    transcription.text,
                    llm_reply.len(),
                    total_audio_bytes
                ),
            ),
        );

        Ok(events)
    }
}

fn push_event(events: &mut Vec<ValidationEvent>, event: ValidationEvent) {
    info!(kind = event.kind(), "{}", event.message());
    events.push(event);
}

struct MockAsrAdapter {
    transcript: String,
}

impl MockAsrAdapter {
    fn new(transcript: String) -> Self {
        Self { transcript }
    }
}

#[async_trait]
impl AsrAdapter for MockAsrAdapter {
    fn name(&self) -> &str {
        "mock-asr"
    }

    async fn transcribe(
        &self,
        audio: &[u8],
        _config: &AsrConfig,
    ) -> AgentResult<TranscriptionResult> {
        if audio.is_empty() {
            return Err(AgentError::InvalidInput(
                "mock ASR received empty audio".to_owned(),
            ));
        }

        Ok(TranscriptionResult::text_only(self.transcript.clone()))
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["en-US".to_owned()]
    }
}

struct MockLlmProvider {
    name: String,
    model: String,
    chunks: Vec<String>,
}

impl MockLlmProvider {
    fn new(name: impl Into<String>, model: impl Into<String>, chunks: Vec<String>) -> Self {
        Self {
            name: name.into(),
            model: model.into(),
            chunks,
        }
    }
}

#[async_trait]
impl LLMProvider for MockLlmProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn default_model(&self) -> &str {
        &self.model
    }

    fn supported_models(&self) -> Vec<&str> {
        vec![self.model.as_str()]
    }

    async fn chat(&self, _request: ChatCompletionRequest) -> AgentResult<ChatCompletionResponse> {
        Ok(ChatCompletionResponse {
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant(self.chunks.concat()),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
        })
    }

    async fn chat_stream(&self, _request: ChatCompletionRequest) -> AgentResult<ChatStream> {
        let mut chunks = Vec::with_capacity(self.chunks.len() + 1);

        for content in &self.chunks {
            chunks.push(Ok(ChatCompletionChunk {
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: ChunkDelta {
                        role: None,
                        content: Some(content.clone()),
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
            }));
        }

        chunks.push(Ok(ChatCompletionChunk {
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta::default(),
                finish_reason: Some(FinishReason::Stop),
            }],
        }));

        Ok(Box::pin(stream::iter(chunks)))
    }
}

struct MockTtsAdapter {
    name: String,
    format: AudioFormat,
    sample_rate: u32,
}

impl MockTtsAdapter {
    fn new(name: impl Into<String>, format: AudioFormat, sample_rate: u32) -> Self {
        Self {
            name: name.into(),
            format,
            sample_rate,
        }
    }
}

#[async_trait]
impl TtsAdapter for MockTtsAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn synthesize(
        &self,
        text: &str,
        _voice: &str,
        _config: &TtsConfig,
    ) -> AgentResult<AudioOutput> {
        if text.trim().is_empty() {
            return Err(AgentError::InvalidInput(
                "mock TTS received empty text".to_owned(),
            ));
        }

        let repeated = text.as_bytes().repeat(2);
        Ok(AudioOutput::new(repeated, self.format, self.sample_rate))
    }

    async fn list_voices(&self) -> AgentResult<Vec<VoiceDescriptor>> {
        Ok(vec![VoiceDescriptor::new(
            "validation-voice",
            "Validation Voice",
            "en-US",
        )])
    }
}
