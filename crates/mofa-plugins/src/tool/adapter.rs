//! Tool to Plugin Adapter
//!
//! Provides an adapter that converts Tool implementations to AgentPlugin implementations.
//! This allows tools to be registered and managed through the plugin system.

use mofa_kernel::agent::components::tool::{Tool, ToolInput, ToolResult};
use mofa_kernel::{
    AgentPlugin, PluginContext, PluginMetadata, PluginResult, PluginState, PluginType,
};
use std::any::Any;
use std::sync::Arc;
use async_trait::async_trait;

/// Adapter that converts a Tool to an AgentPlugin
///
/// This allows any Tool implementation to be registered as a plugin,
/// enabling unified management through the plugin system.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_plugins::tool::adapter::ToolPluginAdapter;
/// use mofa_kernel::agent::components::tool::Tool;
/// use std::sync::Arc;
///
/// let tool = Arc::new(MyTool::new());
/// let plugin = ToolPluginAdapter::new(tool);
///
/// // Now can be registered with PluginManager
/// plugin_manager.register(plugin).await?;
/// ```
pub struct ToolPluginAdapter {
    /// The underlying tool
    tool: Arc<dyn Tool>,
    /// Plugin metadata
    metadata: PluginMetadata,
    /// Current plugin state
    state: PluginState,
    /// Number of times the tool has been called
    call_count: u64,
}

impl ToolPluginAdapter {
    /// Create a new ToolPluginAdapter from a Tool
    pub fn new(tool: Arc<dyn Tool>) -> Self {
        let tool_name = tool.name();
        let metadata = PluginMetadata::new(
            &format!("tool-{}", tool_name),
            &format!("Tool: {}", tool_name),
            PluginType::Tool,
        )
        .with_description(tool.description());

        Self {
            tool,
            metadata,
            state: PluginState::Unloaded,
            call_count: 0,
        }
    }

    /// Get the number of times this tool has been called
    pub fn call_count(&self) -> u64 {
        self.call_count
    }

    /// Get a reference to the underlying tool
    pub fn tool(&self) -> &Arc<dyn Tool> {
        &self.tool
    }
}

#[async_trait]
impl AgentPlugin for ToolPluginAdapter {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    async fn load(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        self.state = PluginState::Paused;
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded;
        Ok(())
    }

    async fn execute(&mut self, input: String) -> PluginResult<String> {
        // Parse the input as ToolInput
        let tool_input: ToolInput = serde_json::from_str(&input)
            .map_err(|e| anyhow::anyhow!("Failed to parse tool input: {}", e))?;

        // Execute the tool with a minimal context
        let ctx = mofa_kernel::agent::context::CoreAgentContext::new("tool-execution");
        let result = self.tool.execute(tool_input, &ctx).await;

        self.call_count += 1;

        // Return the result as a string
        Ok(result.to_string_output())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// Create a ToolPluginAdapter from a Tool
///
/// Convenience function for creating adapters.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_plugins::tool::adapter::adapt_tool;
/// use std::sync::Arc;
///
/// let tool = Arc::new(MyTool::new());
/// let plugin = adapt_tool(tool);
/// ```
pub fn adapt_tool(tool: Arc<dyn Tool>) -> ToolPluginAdapter {
    ToolPluginAdapter::new(tool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::components::tool::{Tool, ToolMetadata};
    use serde_json::json;

    // Test tool implementation
    struct TestTool {
        name: String,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A test tool"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                }
            })
        }

        async fn execute(&self, input: ToolInput, _ctx: &mofa_kernel::agent::context::CoreAgentContext) -> ToolResult {
            if let Some(msg) = input.get_str("message") {
                ToolResult::success_text(format!("TestTool received: {}", msg))
            } else {
                ToolResult::success_text("TestTool called")
            }
        }

        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::new()
        }
    }

    #[tokio::test]
    async fn test_tool_plugin_adapter_creation() {
        let tool = Arc::new(TestTool {
            name: "test_tool".to_string(),
        });
        let adapter = ToolPluginAdapter::new(tool);

        assert_eq!(adapter.metadata().id, "tool-test_tool");
        assert_eq!(adapter.metadata().plugin_type, PluginType::Tool);
        assert_eq!(adapter.state(), PluginState::Unloaded);
    }

    #[tokio::test]
    async fn test_tool_plugin_adapter_lifecycle() {
        let tool = Arc::new(TestTool {
            name: "test_tool".to_string(),
        });
        let mut adapter = ToolPluginAdapter::new(tool);

        let ctx = PluginContext::new("test");

        // Load
        adapter.load(&ctx).await.unwrap();
        assert_eq!(adapter.state(), PluginState::Loaded);

        // Initialize
        adapter.init_plugin().await.unwrap();
        assert_eq!(adapter.state(), PluginState::Loaded);

        // Start
        adapter.start().await.unwrap();
        assert_eq!(adapter.state(), PluginState::Running);

        // Stop
        adapter.stop().await.unwrap();
        assert_eq!(adapter.state(), PluginState::Paused);

        // Unload
        adapter.unload().await.unwrap();
        assert_eq!(adapter.state(), PluginState::Unloaded);
    }

    #[tokio::test]
    async fn test_tool_plugin_adapter_execute() {
        let tool = Arc::new(TestTool {
            name: "echo_tool".to_string(),
        });
        let mut adapter = ToolPluginAdapter::new(tool);

        // Initialize plugin
        adapter.load(&PluginContext::new("test")).await.unwrap();
        adapter.init_plugin().await.unwrap();
        adapter.start().await.unwrap();

        // Execute with message
        let input = json!({"message": "Hello"}).to_string();
        let result = adapter.execute(input).await.unwrap();
        assert!(result.contains("TestTool received: Hello"));

        // Verify call count
        assert_eq!(adapter.call_count(), 1);
    }

    #[test]
    fn test_adapt_tool_convenience_function() {
        let tool = Arc::new(TestTool {
            name: "test".to_string(),
        });
        let adapter = adapt_tool(tool);

        assert_eq!(adapter.metadata().id, "tool-test");
    }
}
