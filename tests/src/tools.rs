use async_trait::async_trait;
use mofa_foundation::agent::components::tool::{SimpleTool, ToolCategory};
use mofa_kernel::agent::components::tool::{ToolInput, ToolResult};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A mock tool simulating a real agent tool
///
/// It allows developers to specify predefined execution outcomes
/// and track inputs that were passed to it during execution.
#[derive(Clone)]
pub struct MockTool {
    name: String,
    description: String,
    schema: Value,
    category: ToolCategory,
    /// Store execution outcomes. Could be sequential or static
    pub stubbed_result: Arc<RwLock<ToolResult>>,
    /// Track all inputs passed to this tool
    pub call_history: Arc<RwLock<Vec<ToolInput>>>,
}

impl MockTool {
    pub fn new(name: &str, description: &str, schema: Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            schema,
            category: ToolCategory::Custom,
            stubbed_result: Arc::new(RwLock::new(ToolResult::success_text("Mock Execution Default"))),
            call_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Sets the result this tool will produce when executed.
    pub async fn set_result(&self, result: ToolResult) {
        *self.stubbed_result.write().await = result;
    }

    /// Retrieve the history of calls made to this tool
    pub async fn history(&self) -> Vec<ToolInput> {
        self.call_history.read().await.clone()
    }

    /// Check the total number of times this tool was executed
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

#[macro_export]
macro_rules! assert_tool_called {
    ($tool:expr, $expected_count:expr) => {
        let count = $tool.call_count().await;
        assert_eq!(
            count, $expected_count,
            "Expected tool '{}' to be called {} times, but was called {} times",
            $tool.name(),
            $expected_count,
            count
        );
    };
}
