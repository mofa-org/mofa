use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::agent::voice::{StageInput, StageOutput, VoicePipelineConfig, VoiceStage};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{Instrument, error, info, info_span};

/// Output from a Voice Pipeline Execution
#[derive(Debug, Clone)]
pub struct VoicePipelineOutput {
    /// The final output from the last stage
    pub final_output: StageOutput,
    /// Per-stage latencies in milliseconds: (stage_name, latency_ms)
    pub stage_latencies: Vec<(String, u128)>,
    /// Total wall-clock latency across all stages
    pub total_latency_ms: u128,
}

/// An executor that chains multiple voice stages together
pub struct VoicePipelineExecutor {
    stages: Vec<Box<dyn VoiceStage>>,
    config: VoicePipelineConfig,
}

impl VoicePipelineExecutor {
    /// Create a new pipeline executor with the given stages and config
    pub fn new(stages: Vec<Box<dyn VoiceStage>>, config: VoicePipelineConfig) -> Self {
        Self { stages, config }
    }

    /// Execute the pipeline with the initial input
    pub async fn execute(&self, initial_input: StageInput) -> AgentResult<VoicePipelineOutput> {
        if self.stages.is_empty() {
            return Err(AgentError::InvalidInput(
                "Voice pipeline has no stages".to_string(),
            ));
        }

        let pipeline_start = std::time::Instant::now();
        let mut current_input = initial_input;
        let mut stage_latencies = Vec::new();

        for stage in &self.stages {
            let stage_name = stage.name().to_string();
            let span = info_span!("voice_pipeline_stage", stage = %stage_name);

            let result: AgentResult<StageOutput> = async {
                info!("Starting stage: {}", stage_name);
                let stage_start = std::time::Instant::now();

                // Execute with optional timeout from config
                let process_future = stage.process(current_input.clone());

                let output = if let Some(ms) = self.config.timeout_ms {
                    match timeout(Duration::from_millis(ms), process_future).await {
                        Ok(res) => res,
                        Err(_) => Err(AgentError::Timeout { duration_ms: ms }),
                    }
                } else {
                    process_future.await
                };

                let latency = stage_start.elapsed().as_millis();
                stage_latencies.push((stage_name.clone(), latency));

                match output {
                    Ok(res) => {
                        info!("Completed stage: {} in {}ms", stage_name, latency);
                        Ok(res)
                    }
                    Err(e) => {
                        error!(
                            "Failed stage: {} after {}ms - Error: {}",
                            stage_name, latency, e
                        );
                        Err(e)
                    }
                }
            }
            .instrument(span)
            .await;

            match result {
                Ok(next_input_output) => {
                    // Convert Output to Input for the next stage
                    current_input = match next_input_output.clone() {
                        StageOutput::Audio(samples) => StageInput::Audio(samples),
                        StageOutput::Text(text) => StageInput::Text(text),
                    };
                    // if it is the very last stage, the loop ends and we return the output
                }
                Err(e) => {
                    if self.config.abort_on_error {
                        return Err(e);
                    } else {
                        info!("Pipeline continuing despite error in stage: {}", stage_name);
                        // If we don't abort, current_input remains the input for the *failed* stage,
                        // which gets passed to the next stage. This might not be ideal depending on
                        // pipeline design, but matches simple fallback behavior.
                    }
                }
            }
        }

        // The final input was converted from the output of the last stage
        let final_output = match current_input {
            StageInput::Audio(samples) => StageOutput::Audio(samples),
            StageInput::Text(text) => StageOutput::Text(text),
        };

        let total_latency_ms = pipeline_start.elapsed().as_millis();

        Ok(VoicePipelineOutput {
            final_output,
            stage_latencies,
            total_latency_ms,
        })
    }
}
