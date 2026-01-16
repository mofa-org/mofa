
//! Built-in tools module
//!
//! Provides a collection of commonly used tools that can be registered
//! with the ToolPlugin.

pub use crate::{
    AgentPlugin, PluginResult, ToolCall, ToolDefinition, ToolExecutor, ToolPlugin, ToolResult,
};

// Individual tool implementations
pub mod http;
pub mod filesystem;
pub mod shell;
pub mod datetime;
pub mod calculator;
pub mod json;
pub mod rhai;
pub mod response_optimizer;
pub mod medical_knowledge;
mod duck_search;
mod web_scrapper;
mod postgres;

pub use calculator::CalculatorTool;
pub use datetime::DateTimeTool;
pub use filesystem::FileSystemTool;
/// Re-export all tools
pub use http::HttpRequestTool;
pub use json::JsonTool;
pub use medical_knowledge::MedicalKnowledgeTool;
pub use response_optimizer::ResponseOptimizerTool;
pub use rhai::RhaiScriptTool;
pub use shell::ShellCommandTool;

/// Convenience function to create a ToolPlugin with all built-in tools
pub fn create_builtin_tool_plugin(plugin_id: &str) -> PluginResult<ToolPlugin> {
    let mut tool_plugin = ToolPlugin::new(plugin_id);

    // Register all built-in tools
    tool_plugin.register_tool(HttpRequestTool::new());
    tool_plugin.register_tool(FileSystemTool::new_with_defaults()?);
    tool_plugin.register_tool(ShellCommandTool::new_with_defaults());
    tool_plugin.register_tool(DateTimeTool::new());
    tool_plugin.register_tool(CalculatorTool::new());
    tool_plugin.register_tool(RhaiScriptTool::new()?);
    tool_plugin.register_tool(JsonTool::new());
    tool_plugin.register_tool(ResponseOptimizerTool::new());
    tool_plugin.register_tool(MedicalKnowledgeTool::new());

    Ok(tool_plugin)
}
