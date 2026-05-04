//! Planning agent example.
//!
//! Demonstrates the four-phase planning loop:
//! 1. The LLM decomposes a research question into three independent subtasks.
//! 2. The executor runs the subtasks in parallel using a mock research tool.
//! 3. The reflection phase validates each subtask output.
//! 4. A final synthesis step combines the results into a structured report.

use async_trait::async_trait;
use mofa_foundation::agent::{PlanningExecutor, PromptPlanner, Tool, ToolInput, ToolMetadata, ToolResult};
use mofa_kernel::agent::components::tool::ToolExt;
use mofa_foundation::llm::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, EmbeddingRequest,
    EmbeddingResponse, FinishReason, LLMError, LLMProvider, LLMResult, ModelInfo, Usage,
};
use mofa_kernel::agent::context::AgentContext;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

// ============================================================================
// Scripted LLM provider — replays pre-loaded responses for the demo
// ============================================================================

struct DemoProvider {
    queue: Mutex<Vec<String>>,
}

impl DemoProvider {
    fn new(responses: Vec<impl Into<String>>) -> Self {
        Self {
            queue: Mutex::new(responses.into_iter().map(Into::into).collect()),
        }
    }
}

#[async_trait]
impl LLMProvider for DemoProvider {
    fn name(&self) -> &str { "demo" }
    fn default_model(&self) -> &str { "demo-model" }

    async fn chat(&self, _req: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        let mut q = self.queue.lock().await;
        if q.is_empty() {
            return Err(LLMError::Other("scripted queue exhausted".into()));
        }
        let content = q.remove(0);
        Ok(ChatCompletionResponse {
            id: "demo-1".into(),
            object: "chat.completion".into(),
            created: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            model: "demo-model".into(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant(content),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 }),
            system_fingerprint: None,
        })
    }

    async fn embedding(&self, _: EmbeddingRequest) -> LLMResult<EmbeddingResponse> {
        Err(LLMError::ProviderNotSupported("no embeddings in demo".into()))
    }

    async fn get_model_info(&self, _: &str) -> LLMResult<ModelInfo> {
        Err(LLMError::ProviderNotSupported("no model info in demo".into()))
    }
}

// ============================================================================
// Mock research tool — simulates a web-search or knowledge-base lookup
// ============================================================================

struct ResearchTool;

#[async_trait]
impl Tool for ResearchTool {
    fn name(&self) -> &str { "research_lookup" }
    fn description(&self) -> &str { "Looks up information for a planning step." }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "step_id":          { "type": "string" },
                "step_description": { "type": "string" }
            },
            "required": ["step_id", "step_description"]
        })
    }

    async fn execute(&self, input: ToolInput<Value>, _ctx: &AgentContext) -> ToolResult<Value> {
        let step_id = input.get_str("step_id").unwrap_or("unknown");
        let description = input.get_str("step_description").unwrap_or("");
        // Simulate a brief async lookup.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        ToolResult::success(json!({
            "step": step_id,
            "finding": format!("Research finding for '{description}': [mock data]")
        }))
    }

    fn metadata(&self) -> ToolMetadata { ToolMetadata::default() }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let provider = Arc::new(DemoProvider::new(vec![
        // Phase 1 — planning: decompose into three parallel research subtasks + synthesis
        json!({
            "goal": "What makes Rust a good choice for systems programming?",
            "steps": [
                {
                    "id": "research-safety",
                    "description": "Research Rust memory safety guarantees",
                    "dependencies": [],
                    "required_tools": ["research_lookup"],
                    "expected_inputs": ["goal"],
                    "completion_criterion": "Covers ownership, borrowing, and absence of undefined behaviour"
                },
                {
                    "id": "research-performance",
                    "description": "Research Rust performance characteristics",
                    "dependencies": [],
                    "required_tools": ["research_lookup"],
                    "expected_inputs": ["goal"],
                    "completion_criterion": "Covers zero-cost abstractions and comparable-to-C throughput"
                },
                {
                    "id": "research-ecosystem",
                    "description": "Research Rust tooling and ecosystem",
                    "dependencies": [],
                    "required_tools": ["research_lookup"],
                    "expected_inputs": ["goal"],
                    "completion_criterion": "Covers Cargo, crates.io, and community support"
                },
                {
                    "id": "synthesise",
                    "description": "Combine the three research findings into a structured report",
                    "dependencies": ["research-safety", "research-performance", "research-ecosystem"],
                    "required_tools": [],
                    "expected_inputs": ["research-safety", "research-performance", "research-ecosystem"],
                    "completion_criterion": "Integrates all three findings coherently"
                }
            ]
        }).to_string(),

        // Phase 3 — reflection (one per step)
        json!({"satisfied": true, "rationale": "safety finding is adequate"}).to_string(),
        json!({"satisfied": true, "rationale": "performance finding is adequate"}).to_string(),
        json!({"satisfied": true, "rationale": "ecosystem finding is adequate"}).to_string(),
        json!({"satisfied": true, "rationale": "synthesis step is adequate"}).to_string(),

        // Phase 4 — synthesis
        "**Rust for Systems Programming**\n\n\
         1. **Memory Safety** — Ownership and borrowing eliminate entire classes of bugs at compile time.\n\
         2. **Performance** — Zero-cost abstractions deliver C-level throughput with high-level ergonomics.\n\
         3. **Ecosystem** — Cargo, crates.io, and an active community make Rust productive day-to-day.".to_string(),
    ]));

    let tool = ResearchTool.into_dynamic();
    let planner = Arc::new(PromptPlanner::new(provider.clone()));
    let executor = PlanningExecutor::new(planner, provider)
        .with_tools(vec![tool])
        .with_max_replans(1);

    println!("Running planning agent...\n");

    match executor
        .execute(
            "What makes Rust a good choice for systems programming?",
            &AgentContext::new("planning-agent-demo"),
        )
        .await
    {
        Ok(result) => {
            println!("Plan executed in {} re-plan cycle(s).\n", result.replans);
            println!("Steps completed: {}", result.steps.len());
            for record in &result.steps {
                println!(
                    "  [{}] {} — {}",
                    if record.evaluation.satisfied { "✓" } else { "✗" },
                    record.step.id,
                    record.step.description
                );
            }
            println!("\n--- Final Report ---\n{}", result.final_response);
        }
        Err(e) => eprintln!("Planning failed: {e}"),
    }
}
