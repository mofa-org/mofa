//! Tool executor trait shared across LLM client and agent loop.
//!
//! This trait unifies tool execution for both direct LLM calls and AgentLoop
//! processing, and supports async discovery of available tools.

use super::types::{LLMResult, Tool};

/// Tool executor trait for LLM tool calling.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool call by name with JSON arguments.
    async fn execute(&self, name: &str, arguments: &str) -> LLMResult<String>;

    /// Get available tool definitions.
    ///
    /// Default returns an empty list for executors that don't expose tools.
    async fn available_tools(&self) -> LLMResult<Vec<Tool>> {
        Ok(Vec::new())
    }
}
