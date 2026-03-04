//! Mock tools for testing agent tool-selection logic.

use async_trait::async_trait;
use mofa_foundation::agent::components::tool::{SimpleTool, ToolCategory};
use mofa_kernel::agent::components::tool::{ToolInput, ToolResult};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A mock tool that records calls and returns a configurable result.
#[derive(Clone)]
pub struct MockTool {
    name: String,
    description: String,
    schema: Value,
    category: ToolCategory,
    /// The result returned by every call to [`execute`](SimpleTool::execute).
    pub stubbed_result: Arc<RwLock<ToolResult>>,
    /// Chronologically ordered inputs passed to this tool.
    pub call_history: Arc<RwLock<Vec<ToolInput>>>,
}

impl MockTool {
    /// Create a mock tool with the given identity and a default "success" result.
    pub fn new(name: &str, description: &str, schema: Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            schema,
            category: ToolCategory::Custom,
            stubbed_result: Arc::new(RwLock::new(ToolResult::success_text(
                "Mock execution default",
            ))),
            call_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Replace the result that will be returned on subsequent calls.
    pub async fn set_result(&self, result: ToolResult) {
        *self.stubbed_result.write().await = result;
    }

    /// Retrieve a clone of the full call history.
    pub async fn history(&self) -> Vec<ToolInput> {
        self.call_history.read().await.clone()
    }

    /// Number of times this tool has been executed.
    pub async fn call_count(&self) -> usize {
        self.call_history.read().await.len()
    }
}

#[async_trait]
impl SimpleTool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.schema.clone()
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        self.call_history.write().await.push(input);
        self.stubbed_result.read().await.clone()
    }

    fn category(&self) -> ToolCategory {
        self.category
    }
}

/// Assert that a [`MockTool`] was called exactly `$expected` times.
#[macro_export]
macro_rules! assert_tool_called {
    ($tool:expr, $expected_count:expr) => {{
        use mofa_foundation::agent::components::tool::SimpleTool as _;
        let count = $tool.call_count().await;
        assert_eq!(
            count, $expected_count,
            "Expected tool '{}' to be called {} time(s), but was called {} time(s)",
            $tool.name(),
            $expected_count,
            count
        );
    }};
}
