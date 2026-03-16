//! Mock Voice Stages for CI and Testing
//!
//! Provides functional implementations of ASR, LLM, and TTS stages
//! that do not require GPU, model files, or network access.

use async_trait::async_trait;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::agent::voice::{StageInput, StageOutput, VoiceStage};
use std::time::Duration;
use tokio::time::sleep;

/// A Mock Auto Speech Recognition (ASR) Stage
pub struct MockAsrStage {
    delay_ms: u64,
}

impl MockAsrStage {
    pub fn new(delay_ms: u64) -> Self {
        Self { delay_ms }
    }
}

#[async_trait]
impl VoiceStage for MockAsrStage {
    fn name(&self) -> &str {
        "MockASR"
    }

    async fn process(&self, input: StageInput) -> AgentResult<StageOutput> {
        sleep(Duration::from_millis(self.delay_ms)).await;

        match input {
            StageInput::Audio(samples) => {
                let simulated_text =
                    format!("(Transcribed {} samples of audio data)", samples.len());
                Ok(StageOutput::Text(simulated_text))
            }
            StageInput::Text(_) => Err(AgentError::InvalidInput(
                "MockASR expects Audio input".to_string(),
            )),
        }
    }
}

/// A Mock Large Language Model (LLM) Stage
pub struct MockLlmStage {
    delay_ms: u64,
}

impl MockLlmStage {
    pub fn new(delay_ms: u64) -> Self {
        Self { delay_ms }
    }
}

#[async_trait]
impl VoiceStage for MockLlmStage {
    fn name(&self) -> &str {
        "MockLLM"
    }

    async fn process(&self, input: StageInput) -> AgentResult<StageOutput> {
        sleep(Duration::from_millis(self.delay_ms)).await;

        match input {
            StageInput::Text(prompt) => {
                let response = format!(
                    "I heard you say: '{}'. This is my mock LLM response.",
                    prompt
                );
                Ok(StageOutput::Text(response))
            }
            StageInput::Audio(_) => Err(AgentError::InvalidInput(
                "MockLLM expects Text input".to_string(),
            )),
        }
    }
}

/// A Mock Text-to-Speech (TTS) Stage
pub struct MockTtsStage {
    delay_ms: u64,
}

impl MockTtsStage {
    pub fn new(delay_ms: u64) -> Self {
        Self { delay_ms }
    }
}

#[async_trait]
impl VoiceStage for MockTtsStage {
    fn name(&self) -> &str {
        "MockTTS"
    }

    async fn process(&self, input: StageInput) -> AgentResult<StageOutput> {
        sleep(Duration::from_millis(self.delay_ms)).await;

        match input {
            StageInput::Text(text) => {
                // Generate 1 second of mock 24kHz audio
                let mock_audio_samples = vec![0.0f32; 24000];
                let _ = text; // Just ignore the text in the mock
                Ok(StageOutput::Audio(mock_audio_samples))
            }
            StageInput::Audio(_) => Err(AgentError::InvalidInput(
                "MockTTS expects Text input".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::voice::VoicePipelineExecutor;
    use mofa_kernel::agent::voice::VoicePipelineConfig;

    #[tokio::test]
    async fn test_mock_voice_pipeline() {
        let asr = Box::new(MockAsrStage::new(10));
        let llm = Box::new(MockLlmStage::new(10));
        let tts = Box::new(MockTtsStage::new(10));

        let stages: Vec<Box<dyn VoiceStage>> = vec![asr, llm, tts];
        let pipeline = VoicePipelineExecutor::new(stages, VoicePipelineConfig::default());

        let initial_audio = vec![0.1, 0.2, 0.3];
        let result = pipeline.execute(StageInput::Audio(initial_audio)).await;

        assert!(result.is_ok());
        let output = result.unwrap();

        // Assert on final output type (TTS outputs audio)
        match output.final_output {
            StageOutput::Audio(samples) => {
                assert_eq!(samples.len(), 24000);
            }
            StageOutput::Text(_) => panic!("Expected audio output from TTS"),
        }

        // Check latency tracking
        assert_eq!(output.stage_latencies.len(), 3);
        assert_eq!(output.stage_latencies[0].0, "MockASR");
        assert_eq!(output.stage_latencies[1].0, "MockLLM");
        assert_eq!(output.stage_latencies[2].0, "MockTTS");
    }
}
