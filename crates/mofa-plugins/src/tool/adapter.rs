//! Tool to Plugin Adapter
//!
//! Provides an adapter that converts Tool implementations to AgentPlugin implementations.
//! This allows tools to be registered and managed through the plugin system.

use async_trait::async_trait;
use mofa_kernel::agent::components::tool::{Tool, ToolInput};
use mofa_kernel::{
    AgentPlugin, PluginContext, PluginMetadata, PluginResult, PluginState, PluginType,
};
use std::any::Any;
use std::sync::Arc;

/// Adapter that converts a Tool to an AgentPlugin
///
/// This allows any Tool implementation to be registered as a plugin,
/// enabling centralized management through the plugin system.
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
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Failed to parse tool input: {}", e)))?;

        // Execute the tool with a minimal context
        let ctx = mofa_kernel::agent::context::AgentContext::new("tool-execution");
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
