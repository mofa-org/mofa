//! Runtime behavioral diff example.
//!
//! Runs two real `ExecutionEngine` executions, converts their
//! `ExecutionResult`s into `mofa-testing` reports, and compares them with
//! `BehaviorDiff`.

use async_trait::async_trait;
use mofa_kernel::agent::types::{AgentInput, AgentOutput, AgentState, ToolUsage};
use mofa_runtime::agent::capabilities::AgentCapabilities;
use mofa_runtime::agent::context::AgentContext;
use mofa_runtime::agent::core::MoFAAgent;
use mofa_runtime::agent::error::AgentResult;
use mofa_runtime::agent::execution::{ExecutionEngine, ExecutionOptions};
use mofa_runtime::agent::registry::AgentRegistry;
use mofa_testing::behavior_diff::{BehaviorDiff, JsonBehaviorDiffFormatter, MarkdownBehaviorDiffFormatter};
use mofa_testing::{BehaviorDiffFormatter, TestReportBuilder};
use std::sync::Arc;
use tokio::sync::RwLock;

struct ScriptedAgent {
    id: String,
    response: String,
    should_fail: bool,
    retry_count: usize,
    fallback_triggered: bool,
    tool_names: Vec<String>,
    capabilities: AgentCapabilities,
    state: AgentState,
}

impl ScriptedAgent {
    fn new(
        id: &str,
        response: &str,
        should_fail: bool,
        retry_count: usize,
        fallback_triggered: bool,
        tool_names: &[&str],
    ) -> Self {
        Self {
            id: id.to_string(),
            response: response.to_string(),
            should_fail,
            retry_count,
            fallback_triggered,
            tool_names: tool_names.iter().map(|name| name.to_string()).collect(),
            capabilities: AgentCapabilities::default(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for ScriptedAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &AgentCapabilities {
        &self.capabilities
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, _input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        if self.should_fail {
            return Err(mofa_runtime::agent::error::AgentError::ExecutionFailed(
                self.response.clone(),
            ));
        }

        // The runtime result only sees normal agent output, so we encode
        // retry/fallback test data into output metadata and promote it later.
        let mut output = AgentOutput::text(&self.response);
        output.tools_used = self
            .tool_names
            .iter()
            .map(|name| {
                ToolUsage::success(
                    name.clone(),
                    serde_json::json!({"source": "example"}),
                    serde_json::json!({"ok": true}),
                    10,
                )
            })
            .collect();
        output.metadata.insert(
            "scripted_retry_count".into(),
            serde_json::json!(self.retry_count),
        );
        output.metadata.insert(
            "scripted_fallback".into(),
            serde_json::json!(self.fallback_triggered),
        );
        Ok(output)
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}

async fn run_case(
    suite_name: &str,
    case_name: &str,
    agent: ScriptedAgent,
) -> mofa_testing::TestReport {
    // Register the agent into a real runtime registry and execute it through
    // the standard execution engine path.
    let registry = Arc::new(AgentRegistry::new());
    registry
        .register(Arc::new(RwLock::new(agent)))
        .await
        .expect("agent registration should succeed");

    let engine = ExecutionEngine::new(registry);
    let mut result = engine
        .execute(case_name, AgentInput::text("compare this run"), ExecutionOptions::default())
        .await
        .expect("execution should return a runtime result");

    // Promote the scripted metadata into canonical runtime fields that the
    // report adapter and behavioral diff already understand.
    if let Some(output) = result.output.as_ref() {
        if let Some(retry_count) = output
            .metadata
            .get("scripted_retry_count")
            .and_then(|value| value.as_u64())
        {
            result.retries = retry_count as usize;
        }
        if let Some(fallback_triggered) = output
            .metadata
            .get("scripted_fallback")
            .and_then(|value| value.as_bool())
        {
            result
                .metadata
                .insert("fallback".into(), serde_json::json!(fallback_triggered));
        }
    }

    TestReportBuilder::new(suite_name)
        .add_execution_result(case_name, &result)
        .build()
}

#[tokio::main]
async fn main() {
    // Baseline and candidate run the same case with different output,
    // tool usage, retry count, and fallback state.
    let baseline = run_case(
        "baseline-runtime",
        "search-answer",
        ScriptedAgent::new(
            "search-answer",
            "Paris",
            false,
            0,
            false,
            &["knowledge_base"],
        ),
    )
    .await;

    let candidate = run_case(
        "candidate-runtime",
        "search-answer",
        ScriptedAgent::new(
            "search-answer",
            "Lyon",
            false,
            2,
            true,
            &["knowledge_base", "search"],
        ),
    )
    .await;

    // Compare the runtime-derived reports using the testing-layer diff.
    let diff = BehaviorDiff::between(&baseline, &candidate);

    println!("== Runtime Markdown ==");
    println!("{}", MarkdownBehaviorDiffFormatter.format(&diff));

    println!("== Runtime JSON ==");
    println!("{}", JsonBehaviorDiffFormatter.format(&diff));
}
