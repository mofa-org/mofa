//! LLM-backed Planner implementation
//!
//! Uses an LLM provider to implement the [`Planner`] trait:
//! - Decomposes goals into structured JSON plans
//! - Reflects on step outputs against completion criteria
//! - Generates revised plans on failure
//! - Synthesizes step results into coherent answers
//!
//! # Design
//!
//! Each method constructs a carefully designed prompt and requests
//! structured JSON output from the LLM. JSON responses are validated
//! and parsed into the appropriate planning types.

use std::sync::Arc;

use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::workflow::planning::{Plan, PlanStep, PlanStepOutput, Planner, ReflectionVerdict};

use super::provider::LLMProvider;
use super::types::{ChatCompletionRequest, ChatMessage};

/// An LLM-backed implementation of the [`Planner`] trait.
///
/// Delegates goal decomposition, reflection, replanning, and synthesis
/// to an LLM provider using structured prompts.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::llm_planner::LLMPlanner;
///
/// let planner = LLMPlanner::new(provider)
///     .with_model("gpt-4o")
///     .with_temperature(0.3);
/// ```
pub struct LLMPlanner {
    provider: Arc<dyn LLMProvider>,
    model: Option<String>,
    temperature: Option<f32>,
}

impl LLMPlanner {
    /// Create a new LLM planner with the given provider.
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            model: None,
            temperature: Some(0.2), // Low temperature for structured output
        }
    }

    /// Set the model to use for planning calls.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the temperature for LLM calls.
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Send a chat request and return the content string.
    async fn chat(&self, system: &str, user: &str) -> AgentResult<String> {
        let model = self
            .model
            .as_deref()
            .unwrap_or(self.provider.default_model());

        let mut request = ChatCompletionRequest::new(model);
        request.messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
        request.temperature = self.temperature;

        let response = self
            .provider
            .chat(request)
            .await
            .map_err(|e| AgentError::ExecutionFailed(format!("LLM call failed: {}", e)))?;

        response
            .content()
            .map(|c| c.to_string())
            .ok_or_else(|| AgentError::ExecutionFailed("LLM returned empty response".into()))
    }

    /// Extract JSON from an LLM response that may contain markdown fences.
    fn extract_json(text: &str) -> &str {
        let trimmed = text.trim();
        // Strip ```json ... ``` fences
        if let Some(start) = trimmed.find("```json") {
            let after = &trimmed[start + 7..];
            if let Some(end) = after.find("```") {
                return after[..end].trim();
            }
        }
        // Strip ``` ... ``` fences
        if let Some(start) = trimmed.find("```") {
            let after = &trimmed[start + 3..];
            if let Some(end) = after.find("```") {
                return after[..end].trim();
            }
        }
        trimmed
    }
}

#[async_trait::async_trait]
impl Planner for LLMPlanner {
    async fn decompose(&self, goal: &str) -> AgentResult<Plan> {
        let system = r#"You are a planning agent. Given a goal, decompose it into executable steps.
Return a JSON object with this exact structure:
{
  "goal": "the original goal",
  "steps": [
    {
      "id": "unique_step_id",
      "description": "what this step does",
      "tools_needed": ["tool_name"],
      "depends_on": ["id_of_dependency_step"],
      "completion_criterion": "how to know this step is done",
      "max_retries": 2
    }
  ]
}

Rules:
- Each step must have a unique string ID (use snake_case).
- depends_on must only reference IDs of other steps in this plan.
- Steps with no dependencies can run in parallel.
- Order steps so dependencies come before dependents.
- Be specific about completion criteria.
- Only include tools_needed if the step requires specific tool invocations.
- Return ONLY the JSON object, no other text."#;

        let user = format!("Decompose this goal into executable steps:\n\n{}", goal);

        let response = self.chat(system, &user).await?;
        let json_str = Self::extract_json(&response);

        // Parse the response into a Plan
        let raw: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            AgentError::ExecutionFailed(format!(
                "Failed to parse plan JSON: {}. Response was: {}",
                e, json_str
            ))
        })?;

        let mut plan = Plan::new(raw["goal"].as_str().unwrap_or(goal).to_string());

        if let Some(steps) = raw["steps"].as_array() {
            for step_val in steps {
                let id = step_val["id"].as_str().unwrap_or("unknown").to_string();
                let description = step_val["description"].as_str().unwrap_or("").to_string();

                let mut step = PlanStep::new(&id, &description);

                if let Some(tools) = step_val["tools_needed"].as_array() {
                    for tool in tools {
                        if let Some(t) = tool.as_str() {
                            step = step.with_tool(t);
                        }
                    }
                }

                if let Some(deps) = step_val["depends_on"].as_array() {
                    for dep in deps {
                        if let Some(d) = dep.as_str() {
                            step = step.depends_on(d);
                        }
                    }
                }

                if let Some(criterion) = step_val["completion_criterion"].as_str() {
                    step = step.with_criterion(criterion);
                }

                if let Some(retries) = step_val["max_retries"].as_u64() {
                    let max_retries = u32::try_from(retries).map_err(|_| {
                        AgentError::ValidationFailed(format!(
                            "max_retries value {} is out of range for u32",
                            retries
                        ))
                    })?;
                    step = step.with_max_retries(max_retries);
                }

                plan = plan.add_step(step);
            }
        }

        Ok(plan)
    }

    async fn reflect(&self, step: &PlanStep, result: &str) -> AgentResult<ReflectionVerdict> {
        let system = r#"You are a quality evaluator. Given a step's description, its completion criterion, and its output, decide:
1. "accept" - the output satisfies the criterion
2. "retry" - the output is close but needs improvement (include feedback)
3. "replan" - the approach is fundamentally wrong (include reason)

Return a JSON object with this structure:
{"verdict": "accept"} OR
{"verdict": "retry", "feedback": "what to improve"} OR
{"verdict": "replan", "reason": "why the approach failed"}

Return ONLY the JSON object."#;

        let user = format!(
            "Step: {}\nCompletion criterion: {}\n\nStep output:\n{}",
            step.description, step.completion_criterion, result
        );

        let response = self.chat(system, &user).await?;
        let json_str = Self::extract_json(&response);

        let raw: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            AgentError::ExecutionFailed(format!("Failed to parse reflection JSON: {}", e))
        })?;

        match raw["verdict"].as_str() {
            Some("accept") => Ok(ReflectionVerdict::Accept),
            Some("retry") => {
                let feedback = raw["feedback"]
                    .as_str()
                    .unwrap_or("Please try again")
                    .to_string();
                Ok(ReflectionVerdict::Retry(feedback))
            }
            Some("replan") => {
                let reason = raw["reason"]
                    .as_str()
                    .unwrap_or("Approach needs revision")
                    .to_string();
                Ok(ReflectionVerdict::Replan(reason))
            }
            other => {
                // Treat unexpected or missing verdicts as an error to avoid
                // silently accepting bad LLM output.
                let verdict_str = other.unwrap_or("<missing verdict>");
                Err(AgentError::ExecutionFailed(format!(
                    "Unexpected reflection verdict from LLM: '{}'",
                    verdict_str
                )))
            }
        }
    }

    async fn replan(&self, plan: &Plan, failed_step: &PlanStep, error: &str) -> AgentResult<Plan> {
        let completed: Vec<String> = plan
            .steps
            .iter()
            .filter(|s| s.status.is_success())
            .map(|s| {
                format!(
                    "  - {} (completed): {}",
                    s.id,
                    s.result.as_deref().unwrap_or("N/A")
                )
            })
            .collect();

        let system = r#"You are a replanning agent. A previous plan partially failed. Using the context of what succeeded and what failed, create a new plan.

Return the same JSON format as a decompose call:
{
  "goal": "the goal",
  "steps": [...]
}

Rules:
- Do NOT include steps that already completed successfully.
- Build on the completed work.
- Avoid the same approach that caused the failure.
- Return ONLY the JSON object."#;

        let user = format!(
            "Original goal: {}\n\nCompleted steps:\n{}\n\nFailed step: {} - {}\nError: {}",
            plan.goal,
            if completed.is_empty() {
                "  (none)".to_string()
            } else {
                completed.join("\n")
            },
            failed_step.id,
            failed_step.description,
            error
        );

        let response = self.chat(system, &user).await?;
        let json_str = Self::extract_json(&response);

        // Re-use decompose parsing
        let raw: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            AgentError::ExecutionFailed(format!("Failed to parse replan JSON: {}", e))
        })?;

        let mut new_plan = Plan::new(raw["goal"].as_str().unwrap_or(&plan.goal).to_string());
        new_plan.iteration = plan.iteration + 1;

        if let Some(steps) = raw["steps"].as_array() {
            for step_val in steps {
                let id = step_val["id"].as_str().unwrap_or("unknown").to_string();
                let description = step_val["description"].as_str().unwrap_or("").to_string();

                let mut step = PlanStep::new(&id, &description);

                if let Some(tools) = step_val["tools_needed"].as_array() {
                    for tool in tools {
                        if let Some(t) = tool.as_str() {
                            step = step.with_tool(t);
                        }
                    }
                }

                if let Some(deps) = step_val["depends_on"].as_array() {
                    for dep in deps {
                        if let Some(d) = dep.as_str() {
                            step = step.depends_on(d);
                        }
                    }
                }

                if let Some(criterion) = step_val["completion_criterion"].as_str() {
                    step = step.with_criterion(criterion);
                }

                if let Some(retries) = step_val["max_retries"].as_u64() {
                    let max_retries = u32::try_from(retries).map_err(|_| {
                        AgentError::ValidationFailed(format!(
                            "max_retries value {} is out of range for u32",
                            retries
                        ))
                    })?;
                    step = step.with_max_retries(max_retries);
                }

                new_plan = new_plan.add_step(step);
            }
        }

        Ok(new_plan)
    }

    async fn synthesize(&self, goal: &str, results: &[PlanStepOutput]) -> AgentResult<String> {
        let system = "You are a synthesis agent. Given a goal and the outputs from multiple steps, combine them into a single coherent response that fully addresses the original goal. Be thorough but concise.";

        let step_outputs: Vec<String> = results
            .iter()
            .map(|r| format!("### Step: {}\n{}", r.step_id, r.output))
            .collect();

        let user = format!(
            "Original goal: {}\n\nStep outputs:\n\n{}",
            goal,
            step_outputs.join("\n\n")
        );

        self.chat(system, &user).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let input = r#"{"verdict": "accept"}"#;
        assert_eq!(LLMPlanner::extract_json(input), input);
    }

    #[test]
    fn test_extract_json_with_fence() {
        let input = "```json\n{\"verdict\": \"accept\"}\n```";
        assert_eq!(LLMPlanner::extract_json(input), r#"{"verdict": "accept"}"#);
    }

    #[test]
    fn test_extract_json_with_plain_fence() {
        let input = "```\n{\"verdict\": \"retry\"}\n```";
        assert_eq!(LLMPlanner::extract_json(input), r#"{"verdict": "retry"}"#);
    }

    #[test]
    fn test_extract_json_with_surrounding_text() {
        let input = "Here is the plan:\n```json\n{\"goal\": \"test\"}\n```\nDone!";
        assert_eq!(LLMPlanner::extract_json(input), r#"{"goal": "test"}"#);
    }
}
