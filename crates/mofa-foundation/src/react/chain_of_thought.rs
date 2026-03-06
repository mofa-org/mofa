//! Chain-of-Thought pattern.
//!
//! Provides a structured step-by-step reasoning loop for tasks that benefit
//! from deliberate decomposition before producing a final answer.

use crate::llm::{LLMAgent, LLMError, LLMResult};
use mofa_kernel::agent::{ReasoningStep, ReasoningStepType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Runtime configuration for [`ChainOfThought`].
#[derive(Debug, Clone)]
pub struct ChainOfThoughtConfig {
    /// Number of reasoning steps to generate before synthesis.
    pub steps: usize,
    /// Emit tracing logs during execution.
    pub verbose: bool,
    /// Prompt template used for each reasoning step.
    pub step_prompt_template: String,
    /// Prompt template used to synthesize the final answer.
    pub final_prompt_template: String,
}

impl Default for ChainOfThoughtConfig {
    fn default() -> Self {
        Self {
            steps: 4,
            verbose: true,
            step_prompt_template: concat!(
                "You are solving a task with deliberate chain-of-thought reasoning.\n\n",
                "Task:\n{task}\n\n",
                "Reasoning so far:\n{previous_steps}\n\n",
                "Produce reasoning step {step_number}/{total_steps}. ",
                "Advance the solution, but do not give the final answer yet."
            )
            .to_string(),
            final_prompt_template: concat!(
                "You have produced a chain-of-thought trace for the task below.\n\n",
                "Task:\n{task}\n\n",
                "Reasoning trace:\n{reasoning_trace}\n\n",
                "Now provide the best final answer."
            )
            .to_string(),
        }
    }
}

impl ChainOfThoughtConfig {
    /// Set the number of reasoning steps.
    pub fn with_steps(mut self, steps: usize) -> Self {
        self.steps = steps.max(1);
        self
    }

    /// Set verbose tracing mode.
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Override the per-step prompt template.
    pub fn with_step_prompt_template(mut self, template: impl Into<String>) -> Self {
        self.step_prompt_template = template.into();
        self
    }

    /// Override the final synthesis prompt template.
    pub fn with_final_prompt_template(mut self, template: impl Into<String>) -> Self {
        self.final_prompt_template = template.into();
        self
    }
}

/// Result of a chain-of-thought run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainOfThoughtResult {
    /// Original task.
    pub task: String,
    /// Generated reasoning steps.
    pub reasoning_steps: Vec<ReasoningStep>,
    /// Final answer synthesized from the reasoning trace.
    pub final_answer: String,
    /// Total runtime in milliseconds.
    pub total_duration_ms: u64,
}

impl ChainOfThoughtResult {
    /// Render the reasoning trace as markdown.
    pub fn to_markdown_trace(&self) -> String {
        let mut lines = vec![
            "# Chain-of-Thought Trace".to_string(),
            String::new(),
            format!("**Task:** {}", self.task),
            format!("**Duration:** {} ms", self.total_duration_ms),
            String::new(),
            "## Reasoning Steps".to_string(),
        ];

        for step in &self.reasoning_steps {
            lines.push(format!("{}. {}", step.step_number, step.content));
        }

        lines.push(String::new());
        lines.push("## Final Answer".to_string());
        lines.push("```text".to_string());
        lines.push(self.final_answer.clone());
        lines.push("```".to_string());

        lines.join("\n")
    }
}

/// Classic chain-of-thought reasoning pattern.
pub struct ChainOfThought {
    thinker: Arc<LLMAgent>,
    synthesizer: Option<Arc<LLMAgent>>,
    config: ChainOfThoughtConfig,
}

impl ChainOfThought {
    /// Create a builder for [`ChainOfThought`].
    pub fn builder() -> ChainOfThoughtBuilder {
        ChainOfThoughtBuilder::new()
    }

    /// Execute the reasoning loop.
    pub async fn run(&self, task: impl Into<String>) -> LLMResult<ChainOfThoughtResult> {
        let task = task.into();
        let start = std::time::Instant::now();
        let mut reasoning_steps = Vec::with_capacity(self.config.steps);

        for step_number in 1..=self.config.steps {
            if self.config.verbose {
                tracing::info!(
                    "[ChainOfThought] Generating reasoning step {}/{}",
                    step_number,
                    self.config.steps
                );
            }

            let previous_steps = render_reasoning_trace(&reasoning_steps);
            let step_number_text = step_number.to_string();
            let total_steps_text = self.config.steps.to_string();
            let prompt = fill_template(
                &self.config.step_prompt_template,
                &[
                    ("task", task.as_str()),
                    ("previous_steps", previous_steps.as_str()),
                    ("step_number", step_number_text.as_str()),
                    ("total_steps", total_steps_text.as_str()),
                ],
            );

            let thought = self.thinker.ask(&prompt).await?;
            reasoning_steps.push(ReasoningStep::new(
                ReasoningStepType::Thought,
                thought,
                step_number,
            ));
        }

        let reasoning_trace = render_reasoning_trace(&reasoning_steps);
        let final_prompt = fill_template(
            &self.config.final_prompt_template,
            &[
                ("task", task.as_str()),
                ("reasoning_trace", reasoning_trace.as_str()),
            ],
        );
        let final_answer = self
            .synthesizer
            .as_ref()
            .unwrap_or(&self.thinker)
            .ask(&final_prompt)
            .await?;

        Ok(ChainOfThoughtResult {
            task,
            reasoning_steps,
            final_answer,
            total_duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Alias for [`Self::run`].
    pub async fn execute(&self, task: impl Into<String>) -> LLMResult<ChainOfThoughtResult> {
        self.run(task).await
    }
}

/// Builder for [`ChainOfThought`].
pub struct ChainOfThoughtBuilder {
    thinker: Option<Arc<LLMAgent>>,
    synthesizer: Option<Arc<LLMAgent>>,
    config: ChainOfThoughtConfig,
}

impl ChainOfThoughtBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            thinker: None,
            synthesizer: None,
            config: ChainOfThoughtConfig::default(),
        }
    }

    /// Set the reasoning agent.
    pub fn with_llm(mut self, thinker: Arc<LLMAgent>) -> Self {
        self.thinker = Some(thinker);
        self
    }

    /// Set an optional dedicated synthesizer agent.
    pub fn with_synthesizer(mut self, synthesizer: Arc<LLMAgent>) -> Self {
        self.synthesizer = Some(synthesizer);
        self
    }

    /// Set the number of reasoning steps.
    pub fn with_steps(mut self, steps: usize) -> Self {
        self.config = self.config.with_steps(steps);
        self
    }

    /// Set verbose tracing mode.
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.config = self.config.with_verbose(verbose);
        self
    }

    /// Set the runtime config.
    pub fn with_config(mut self, config: ChainOfThoughtConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the pattern.
    pub fn build(self) -> Result<ChainOfThought, LLMError> {
        let thinker = self
            .thinker
            .ok_or_else(|| LLMError::ConfigError("ChainOfThought requires an LLM".to_string()))?;

        Ok(ChainOfThought {
            thinker,
            synthesizer: self.synthesizer,
            config: self.config,
        })
    }
}

impl Default for ChainOfThoughtBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn render_reasoning_trace(steps: &[ReasoningStep]) -> String {
    if steps.is_empty() {
        return "No reasoning steps yet.".to_string();
    }

    steps
        .iter()
        .map(|step| format!("{}. {}", step.step_number, step.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fill_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in replacements {
        rendered = rendered.replace(&format!("{{{}}}", key), value);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{MockLLMProvider, simple_llm_agent};

    #[test]
    fn config_defaults() {
        let config = ChainOfThoughtConfig::default();
        assert_eq!(config.steps, 4);
        assert!(config.verbose);
    }

    #[tokio::test]
    async fn chain_of_thought_runs_reasoning_and_synthesis() {
        let provider = Arc::new(MockLLMProvider::new("thoughts"));
        provider
            .add_response("Break the problem into sub-parts.")
            .await;
        provider
            .add_response("Check constraints and likely edge cases.")
            .await;
        provider
            .add_response("Combine the strongest observations into an answer.")
            .await;
        provider
            .add_response("Final answer generated from the trace.")
            .await;

        let llm = Arc::new(simple_llm_agent("cot", provider, "Reason carefully."));
        let chain = ChainOfThought::builder()
            .with_llm(llm)
            .with_steps(3)
            .with_verbose(false)
            .build()
            .unwrap();

        let result = chain.run("Explain why retries need backoff").await.unwrap();

        assert_eq!(result.reasoning_steps.len(), 3);
        assert_eq!(
            result.final_answer,
            "Final answer generated from the trace."
        );
        assert!(result.to_markdown_trace().contains("## Final Answer"));
    }
}
