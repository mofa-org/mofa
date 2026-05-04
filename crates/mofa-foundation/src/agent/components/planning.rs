//! Multi-phase planning and execution primitives.
//!
//! Provides a four-phase planning loop:
//! 1. **Planning** — LLM decomposes the goal into ordered steps.
//! 2. **Execution** — each step runs with tool access; independent steps run in parallel.
//! 3. **Reflection** — LLM validates each step against its completion criterion; triggers re-plan if needed.
//! 4. **Synthesis** — all step outputs are combined into the final response.

use crate::agent::components::memory::Memory;
use crate::llm::{LLMClient, LLMError, LLMProvider};
use async_trait::async_trait;
use futures::future::join_all;
use mofa_kernel::agent::components::planner::{
    CompletedStep, ExecutionPlan, MemoryItemSnapshot, PlannedStep, Planner, PlanningRequest,
};
use mofa_kernel::agent::components::tool::{DynTool, ToolDescriptor};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;

// ============================================================================
// Public types
// ============================================================================

/// Current phase of the planning loop.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanningPhase {
    Planning,
    Execution,
    Reflection,
    Synthesis,
}

/// Reflection verdict for a single step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepEvaluation {
    /// Whether the step satisfied its completion criterion.
    pub satisfied: bool,
    /// Reasoning supplied by the evaluator.
    pub rationale: String,
}

/// Final status of a step after reflection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepExecutionStatus {
    Succeeded,
    NeedsReplan,
}

/// Raw output record from executing a step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepExecutionRecord {
    pub id: String,
    pub output: String,
    pub used_tools: Vec<String>,
}

/// Complete record for a step (execution + reflection).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanningStepRecord {
    pub step: PlannedStep,
    pub execution: StepExecutionRecord,
    pub evaluation: StepEvaluation,
    pub status: StepExecutionStatus,
}

/// Result returned by [`PlanningExecutor::execute`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanningExecutionResult {
    /// The final plan that produced the result.
    pub plan: ExecutionPlan,
    /// All step records in dependency order.
    pub steps: Vec<PlanningStepRecord>,
    /// Final synthesized answer.
    pub final_response: String,
    /// Number of re-plan cycles performed.
    pub replans: usize,
}

// ============================================================================
// PromptPlanner
// ============================================================================

/// LLM-backed implementation of [`Planner`].
pub struct PromptPlanner {
    provider: Arc<dyn LLMProvider>,
    system_prompt: String,
}

impl PromptPlanner {
    /// Create a planner backed by the given LLM provider.
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            system_prompt: "You decompose a goal into executable steps. \
                Prefer independent steps when possible; only add a dependency when the output \
                of one step is a required input for another. List tool names exactly as given. \
                Return ONLY valid JSON matching the schema provided — no prose."
                .to_string(),
        }
    }

    fn build_prompt(&self, req: &PlanningRequest) -> String {
        let tools = if req.available_tools.is_empty() {
            "No tools available.".to_string()
        } else {
            req.available_tools
                .iter()
                .map(|t| format!("- {}: {}", t.name, t.description))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let memory = if req.recalled_memory.is_empty() {
            "None.".to_string()
        } else {
            req.recalled_memory
                .iter()
                .map(|m| format!("- {}: {}", m.key, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let completed = if req.completed_steps.is_empty() {
            "None.".to_string()
        } else {
            req.completed_steps
                .iter()
                .map(|s| format!("- {} => {}", s.id, s.output))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let prior = req
            .prior_plan
            .as_ref()
            .map(|p| serde_json::to_string_pretty(p).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| "None.".to_string());

        format!(
            "Goal:\n{goal}\n\n\
             Available tools:\n{tools}\n\n\
             Recalled memory:\n{memory}\n\n\
             Completed steps:\n{completed}\n\n\
             Prior plan:\n{prior}\n\n\
             Re-plan reason:\n{reason}\n\n\
             Return strict JSON:\n\
             {{\"goal\":\"string\",\"steps\":[{{\"id\":\"string\",\"description\":\"string\",\
             \"dependencies\":[\"step-id\"],\"required_tools\":[\"tool-name\"],\
             \"expected_inputs\":[\"text\"],\"completion_criterion\":\"string\"}}]}}",
            goal = req.goal,
            reason = req.replan_reason.as_deref().unwrap_or("None."),
        )
    }
}

#[async_trait]
impl Planner for PromptPlanner {
    async fn plan(&self, req: &PlanningRequest, _ctx: &AgentContext) -> AgentResult<ExecutionPlan> {
        let client = LLMClient::new(self.provider.clone());
        let response = client
            .chat()
            .system(self.system_prompt.clone())
            .user(self.build_prompt(req))
            .send()
            .await
            .map_err(llm_to_agent)?;

        let content = response
            .content()
            .ok_or_else(|| AgentError::ReasoningError("planner returned no content".to_string()))?;

        let plan: ExecutionPlan = serde_json::from_str(content).map_err(|e| {
            AgentError::ReasoningError(format!("planner returned invalid JSON: {e}"))
        })?;

        plan.validate()?;
        Ok(plan)
    }

    fn name(&self) -> &str {
        "prompt-planner"
    }
}

// ============================================================================
// PlanningExecutor
// ============================================================================

/// Runs the four-phase planning loop for a goal.
///
/// Independent steps are executed in parallel; reflection triggers re-planning
/// when a step does not satisfy its completion criterion.
pub struct PlanningExecutor {
    planner: Arc<dyn Planner>,
    /// Provider used for reflection and synthesis LLM calls.
    provider: Arc<dyn LLMProvider>,
    tools: Arc<HashMap<String, Arc<dyn DynTool>>>,
    memory: Option<Arc<Mutex<Box<dyn Memory>>>>,
    memory_search_limit: usize,
    max_replans: usize,
}

impl PlanningExecutor {
    /// Create an executor with the given planner and LLM provider.
    pub fn new(planner: Arc<dyn Planner>, provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            planner,
            provider,
            tools: Arc::new(HashMap::new()),
            memory: None,
            memory_search_limit: 5,
            max_replans: 1,
        }
    }

    /// Register tools available to steps.
    pub fn with_tools(mut self, tools: Vec<Arc<dyn DynTool>>) -> Self {
        let map = tools.into_iter().map(|t| (t.name().to_string(), t)).collect();
        self.tools = Arc::new(map);
        self
    }

    /// Attach a long-term memory store for context recall during planning.
    pub fn with_memory(mut self, memory: Box<dyn Memory>) -> Self {
        self.memory = Some(Arc::new(Mutex::new(memory)));
        self
    }

    /// Maximum number of re-plan cycles before failing (default: 1).
    pub fn with_max_replans(mut self, n: usize) -> Self {
        self.max_replans = n;
        self
    }

    /// Number of memory items to recall per planning request (default: 5).
    pub fn with_memory_search_limit(mut self, n: usize) -> Self {
        self.memory_search_limit = n;
        self
    }

    /// Run the full planning loop for `goal` and return the final result.
    pub async fn execute(
        &self,
        goal: impl Into<String>,
        ctx: &AgentContext,
    ) -> AgentResult<PlanningExecutionResult> {
        let goal = goal.into();
        let mut replans = 0usize;
        let mut completed_steps: Vec<PlanningStepRecord> = Vec::new();
        let mut req = self
            .build_request(goal.clone(), None, None, &completed_steps)
            .await?;

        loop {
            let plan = self.planner.plan(&req, ctx).await?;
            let step_records = self.execute_plan(&plan, ctx).await?;

            if let Some(reason) = first_replan_reason(&step_records) {
                if replans < self.max_replans && self.planner.supports_replanning() {
                    replans += 1;
                    completed_steps = successful_records(&step_records);
                    req = self
                        .build_request(goal.clone(), Some(plan), Some(reason), &completed_steps)
                        .await?;
                    continue;
                }
            }

            let final_response = self.synthesize(&goal, &step_records).await?;
            return Ok(PlanningExecutionResult {
                plan,
                steps: step_records,
                final_response,
                replans,
            });
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    async fn build_request(
        &self,
        goal: String,
        prior_plan: Option<ExecutionPlan>,
        replan_reason: Option<String>,
        done: &[PlanningStepRecord],
    ) -> AgentResult<PlanningRequest> {
        let recalled_memory = self.recall_memory(&goal).await?;
        let available_tools = self
            .tools
            .values()
            .map(|t| ToolDescriptor::from_dyn_tool(t.as_ref()))
            .collect();
        let completed_steps = done
            .iter()
            .map(|r| CompletedStep::new(r.step.id.clone(), r.execution.output.clone()))
            .collect();

        Ok(PlanningRequest {
            goal,
            available_tools,
            recalled_memory,
            completed_steps,
            prior_plan,
            replan_reason,
        })
    }

    async fn recall_memory(&self, goal: &str) -> AgentResult<Vec<MemoryItemSnapshot>> {
        let Some(mem) = &self.memory else {
            return Ok(Vec::new());
        };
        let guard = mem.lock().await;
        let items = guard.search(goal, self.memory_search_limit).await?;
        Ok(items
            .into_iter()
            .filter_map(|item| {
                item.value.as_text().map(|text| {
                    MemoryItemSnapshot::new(
                        item.key.clone(),
                        text.to_string(),
                        item.metadata.into_iter().collect(),
                    )
                })
            })
            .collect())
    }

    async fn execute_plan(
        &self,
        plan: &ExecutionPlan,
        ctx: &AgentContext,
    ) -> AgentResult<Vec<PlanningStepRecord>> {
        plan.validate()?;
        let groups = plan.topological_groups()?;
        let mut outputs: BTreeMap<String, String> = BTreeMap::new();
        let mut all_records: Vec<PlanningStepRecord> = Vec::new();

        for group in groups {
            // Build futures for all steps in this group; they share no mutable
            // state so they can run concurrently via join_all.
            let futures: Vec<_> = group
                .into_iter()
                .map(|step| {
                    let child_ctx = ctx.child(format!("plan-step-{}", step.id));
                    let prior = outputs.clone();
                    let tools = self.tools.clone();
                    let provider = self.provider.clone();
                    async move {
                        run_step(tools, provider, step, prior, child_ctx).await
                    }
                })
                .collect();

            let results = join_all(futures).await;

            for result in results {
                let record = result?;
                outputs.insert(record.step.id.clone(), record.execution.output.clone());
                all_records.push(record);
            }
        }

        all_records.sort_by(|a, b| a.step.id.cmp(&b.step.id));
        Ok(all_records)
    }

    async fn synthesize(
        &self,
        goal: &str,
        records: &[PlanningStepRecord],
    ) -> AgentResult<String> {
        let step_outputs: Vec<_> = records
            .iter()
            .map(|r| {
                json!({
                    "id": r.step.id,
                    "description": r.step.description,
                    "output": r.execution.output,
                })
            })
            .collect();

        let prompt = format!(
            "Goal: {goal}\nStep outputs: {}\n\nSynthesize a final answer from the step outputs.",
            serde_json::to_string_pretty(&step_outputs)
                .unwrap_or_else(|_| "[]".to_string())
        );

        LLMClient::new(self.provider.clone())
            .ask_with_system(
                "You synthesize planning step outputs into a cohesive final response.",
                prompt,
            )
            .await
            .map_err(llm_to_agent)
    }
}

// ============================================================================
// Free functions used by the parallel step runner
// ============================================================================

/// Execute a single step (tool calls + reflection) without borrowing the executor.
async fn run_step(
    tools: Arc<HashMap<String, Arc<dyn DynTool>>>,
    provider: Arc<dyn LLMProvider>,
    step: PlannedStep,
    prior_outputs: BTreeMap<String, String>,
    ctx: AgentContext,
) -> AgentResult<PlanningStepRecord> {
    let execution = execute_step(&tools, &step, &prior_outputs, &ctx).await?;
    let evaluation = reflect_on_step(&provider, &step, &execution, &prior_outputs).await?;
    let status = if evaluation.satisfied {
        StepExecutionStatus::Succeeded
    } else {
        StepExecutionStatus::NeedsReplan
    };

    Ok(PlanningStepRecord {
        step,
        execution,
        evaluation,
        status,
    })
}

async fn execute_step(
    tools: &HashMap<String, Arc<dyn DynTool>>,
    step: &PlannedStep,
    prior_outputs: &BTreeMap<String, String>,
    ctx: &AgentContext,
) -> AgentResult<StepExecutionRecord> {
    if step.required_tools.is_empty() {
        return Ok(StepExecutionRecord {
            id: step.id.clone(),
            output: fallback_output(step, prior_outputs),
            used_tools: Vec::new(),
        });
    }

    let mut tool_outputs = Vec::new();
    let mut used_tools = Vec::new();

    for name in &step.required_tools {
        let tool = tools
            .get(name)
            .ok_or_else(|| AgentError::ToolNotFound(name.clone()))?;

        let input = json!({
            "goal": step.description,
            "step_id": step.id,
            "step_description": step.description,
            "expected_inputs": step.expected_inputs,
            "dependency_outputs": prior_outputs,
        });

        let output = tool.execute_dynamic(input, ctx).await?;
        tool_outputs.push(format!("{name}: {}", json_to_string(&output)));
        used_tools.push(name.clone());
    }

    Ok(StepExecutionRecord {
        id: step.id.clone(),
        output: tool_outputs.join("\n"),
        used_tools,
    })
}

async fn reflect_on_step(
    provider: &Arc<dyn LLMProvider>,
    step: &PlannedStep,
    execution: &StepExecutionRecord,
    prior_outputs: &BTreeMap<String, String>,
) -> AgentResult<StepEvaluation> {
    let prompt = format!(
        "Step description: {}\n\
         Completion criterion: {}\n\
         Expected inputs: {}\n\
         Dependency outputs: {}\n\
         Actual output: {}\n\n\
         Return strict JSON with keys: satisfied (boolean), rationale (string).",
        step.description,
        step.completion_criterion,
        serde_json::to_string(&step.expected_inputs).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(prior_outputs).unwrap_or_else(|_| "{}".to_string()),
        execution.output,
    );

    let content = LLMClient::new(provider.clone())
        .ask_with_system(
            "You validate whether a planning step satisfies its completion criterion.",
            prompt,
        )
        .await
        .map_err(llm_to_agent)?;

    serde_json::from_str(&content).map_err(|e| {
        AgentError::ReasoningError(format!("reflection returned invalid JSON: {e}"))
    })
}

// ============================================================================
// Small utilities
// ============================================================================

fn fallback_output(step: &PlannedStep, prior: &BTreeMap<String, String>) -> String {
    if step.dependencies.is_empty() {
        format!("Completed '{}': {}", step.id, step.description)
    } else {
        let inputs = step
            .dependencies
            .iter()
            .filter_map(|d| prior.get(d).map(|o| format!("{d} => {o}")))
            .collect::<Vec<_>>()
            .join("; ");
        format!("Completed '{}': {} | Inputs: {inputs}", step.id, step.description)
    }
}

fn json_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn first_replan_reason(records: &[PlanningStepRecord]) -> Option<String> {
    records.iter().find_map(|r| {
        (r.status == StepExecutionStatus::NeedsReplan).then(|| {
            format!(
                "step '{}' did not satisfy criterion: {}",
                r.step.id, r.evaluation.rationale
            )
        })
    })
}

fn successful_records(records: &[PlanningStepRecord]) -> Vec<PlanningStepRecord> {
    records
        .iter()
        .filter(|r| r.status == StepExecutionStatus::Succeeded)
        .cloned()
        .collect()
}

fn llm_to_agent(e: LLMError) -> AgentError {
    AgentError::ReasoningError(e.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::components::memory::{InMemoryStorage, MemoryValue};
    use crate::llm::{
        ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, EmbeddingRequest,
        EmbeddingResponse, FinishReason, LLMResult, ModelInfo, Usage,
    };
    use mofa_kernel::agent::components::tool::{Tool, ToolExt, ToolInput, ToolMetadata, ToolResult};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use tokio::sync::Mutex as TokioMutex;

    // -------------------------------------------------------------------
    // Scripted LLM provider — returns responses from a pre-loaded queue
    // -------------------------------------------------------------------

    struct ScriptedProvider {
        responses: Arc<TokioMutex<Vec<String>>>,
        captured: Arc<TokioMutex<Vec<ChatCompletionRequest>>>,
    }

    impl ScriptedProvider {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Arc::new(TokioMutex::new(responses)),
                captured: Arc::new(TokioMutex::new(Vec::new())),
            }
        }

        async fn captured_requests(&self) -> Vec<ChatCompletionRequest> {
            self.captured.lock().await.clone()
        }
    }

    #[async_trait]
    impl LLMProvider for ScriptedProvider {
        fn name(&self) -> &str { "scripted" }
        fn default_model(&self) -> &str { "scripted-model" }

        async fn chat(&self, req: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
            self.captured.lock().await.push(req);
            let next = self.responses.lock().await.remove(0);
            Ok(ChatCompletionResponse {
                id: "r-1".to_string(),
                object: "chat.completion".to_string(),
                created: now_secs(),
                model: "scripted-model".to_string(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage::assistant(next),
                    finish_reason: Some(FinishReason::Stop),
                    logprobs: None,
                }],
                usage: Some(Usage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 }),
                system_fingerprint: None,
            })
        }

        async fn embedding(&self, _: EmbeddingRequest) -> LLMResult<EmbeddingResponse> {
            Err(crate::llm::LLMError::ProviderNotSupported("no embeddings".into()))
        }

        async fn get_model_info(&self, _: &str) -> LLMResult<ModelInfo> {
            Err(crate::llm::LLMError::ProviderNotSupported("no model info".into()))
        }
    }

    // -------------------------------------------------------------------
    // A tool with a configurable delay for measuring parallel execution
    // -------------------------------------------------------------------

    struct DelayTool {
        tool_name: &'static str,
        delay: Duration,
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for DelayTool {
        fn name(&self) -> &str { self.tool_name }
        fn description(&self) -> &str { "Delayed stub tool" }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {"step_id": {"type": "string"}}})
        }

        async fn execute(&self, _: ToolInput<Value>, _: &AgentContext) -> ToolResult<Value> {
            tokio::time::sleep(self.delay).await;
            self.call_count.fetch_add(1, Ordering::SeqCst);
            ToolResult::success(json!({"done": self.tool_name}))
        }

        fn metadata(&self) -> ToolMetadata { ToolMetadata::default() }
    }

    // -------------------------------------------------------------------
    // Test 1: independent steps run in parallel
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn independent_steps_run_in_parallel() {
        let provider = Arc::new(ScriptedProvider::new(vec![
            // planning response — two independent search steps + one dependent synthesis
            json!({
                "goal": "Research Rust",
                "steps": [
                    {
                        "id": "search-a",
                        "description": "Research memory safety",
                        "dependencies": [],
                        "required_tools": ["search"],
                        "expected_inputs": ["goal"],
                        "completion_criterion": "mentions memory safety"
                    },
                    {
                        "id": "search-b",
                        "description": "Research performance",
                        "dependencies": [],
                        "required_tools": ["search"],
                        "expected_inputs": ["goal"],
                        "completion_criterion": "mentions performance"
                    },
                    {
                        "id": "combine",
                        "description": "Combine findings",
                        "dependencies": ["search-a", "search-b"],
                        "required_tools": [],
                        "expected_inputs": ["search-a", "search-b"],
                        "completion_criterion": "combines both"
                    }
                ]
            }).to_string(),
            // reflect search-a
            json!({"satisfied": true, "rationale": "ok"}).to_string(),
            // reflect search-b
            json!({"satisfied": true, "rationale": "ok"}).to_string(),
            // reflect combine
            json!({"satisfied": true, "rationale": "ok"}).to_string(),
            // synthesis
            "Final Rust report".to_string(),
        ]));

        let counter = Arc::new(AtomicUsize::new(0));
        let tool = DelayTool {
            tool_name: "search",
            delay: Duration::from_millis(200),
            call_count: counter.clone(),
        }
        .into_dynamic();

        let planner = Arc::new(PromptPlanner::new(provider.clone()));
        let executor = PlanningExecutor::new(planner, provider)
            .with_tools(vec![tool])
            .with_max_replans(0);

        let start = Instant::now();
        let result = executor
            .execute("Research Rust", &AgentContext::new("parallel-test"))
            .await
            .expect("executor should succeed");
        let elapsed = start.elapsed();

        assert_eq!(counter.load(Ordering::SeqCst), 2, "both tools must have been called");
        assert_eq!(result.final_response, "Final Rust report");
        // Sequential would take ≥ 400ms; parallel should finish in < 500ms.
        assert!(
            elapsed < Duration::from_millis(500),
            "parallel steps took too long: {elapsed:?}"
        );
    }

    // -------------------------------------------------------------------
    // Test 2: recalled memory appears in the planning prompt
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn recalled_memory_included_in_planning_prompt() {
        let provider = Arc::new(ScriptedProvider::new(vec![
            json!({
                "goal": "Summarise Rust",
                "steps": [{
                    "id": "summarise",
                    "description": "Write summary",
                    "dependencies": [],
                    "required_tools": [],
                    "expected_inputs": ["goal"],
                    "completion_criterion": "is a summary"
                }]
            }).to_string(),
            json!({"satisfied": true, "rationale": "ok"}).to_string(),
            "Done".to_string(),
        ]));

        let mut mem = InMemoryStorage::new();
        mem.store(
            "rust-fact",
            MemoryValue::text("Rust ownership prevents data races at compile time."),
        )
        .await
        .unwrap();

        let planner = Arc::new(PromptPlanner::new(provider.clone()));
        let executor = PlanningExecutor::new(planner, provider.clone())
            .with_memory(Box::new(mem))
            .with_max_replans(0);

        executor
            .execute("Summarise Rust", &AgentContext::new("memory-test"))
            .await
            .expect("executor should succeed");

        // The first captured request should be the planning call; its user
        // message must contain the recalled memory text.
        let reqs = provider.captured_requests().await;
        let plan_user_msg = reqs[0]
            .messages
            .iter()
            .find(|m| matches!(m.role, crate::llm::Role::User))
            .and_then(|m| m.text_content())
            .expect("planning request must have a user message");

        assert!(
            plan_user_msg.contains("Rust ownership prevents data races"),
            "recalled memory must appear in planning prompt"
        );
    }

    // -------------------------------------------------------------------
    // Test 3: reflection failure triggers a re-plan
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn reflection_failure_triggers_replan() {
        let provider = Arc::new(ScriptedProvider::new(vec![
            // First plan
            json!({
                "goal": "Write report",
                "steps": [{
                    "id": "draft",
                    "description": "Write a weak draft",
                    "dependencies": [],
                    "required_tools": [],
                    "expected_inputs": ["goal"],
                    "completion_criterion": "draft is thorough"
                }]
            }).to_string(),
            // Reflection: not satisfied → triggers re-plan
            json!({"satisfied": false, "rationale": "too shallow"}).to_string(),
            // Second plan (re-plan)
            json!({
                "goal": "Write report",
                "steps": [{
                    "id": "rewrite",
                    "description": "Write a thorough draft",
                    "dependencies": [],
                    "required_tools": [],
                    "expected_inputs": ["goal"],
                    "completion_criterion": "draft is thorough"
                }]
            }).to_string(),
            // Reflection: satisfied
            json!({"satisfied": true, "rationale": "thorough enough"}).to_string(),
            // Synthesis
            "Final report".to_string(),
        ]));

        let planner = Arc::new(PromptPlanner::new(provider.clone()));
        let executor = PlanningExecutor::new(planner, provider)
            .with_max_replans(1);

        let result = executor
            .execute("Write report", &AgentContext::new("replan-test"))
            .await
            .expect("executor should succeed after re-planning");

        assert_eq!(result.replans, 1, "exactly one re-plan should have occurred");
        assert_eq!(result.final_response, "Final report");
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].step.id, "rewrite");
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}
