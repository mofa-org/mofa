//! MCP Server Example
//!
//! Demonstrates how to expose MoFA tools as an MCP server so external systems
//! like Claude Desktop, Cursor, or other MCP clients can discover and call them.
//!
//! # Running
//!
//! ```bash
//! cargo run -p mcp_server
//! ```
//!
//! Then connect any MCP client to: http://127.0.0.1:3000/mcp
//!
//! # Claude Desktop configuration example
//!
//! Add to `claude_desktop_config.json`:
//! ```json
//! {
//!   "mcpServers": {
//!     "mofa-agent": {
//!       "url": "http://127.0.0.1:3000/mcp"
//!     }
//!   }
//! }
//! ```

use mofa_foundation::agent::tools::mcp::McpServerManager;
use mofa_kernel::agent::components::mcp::McpHostConfig;
use mofa_kernel::agent::components::tool::{ToolExt, ToolInput, ToolResult};
use mofa_kernel::agent::context::AgentContext;
use tokio_util::sync::CancellationToken;

// ============================================================================
// Example tools
// ============================================================================

/// A simple echo tool that returns its input unchanged.
struct EchoTool;

#[async_trait::async_trait]
impl mofa_kernel::agent::components::tool::Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Returns the input message unchanged. Useful for testing connectivity."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back"
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        let message = input
            .arguments
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("(no message)");

        ToolResult::success(serde_json::json!({ "echo": message }))
    }
}

/// A tool that adds two integers.
struct AddTool;

#[async_trait::async_trait]
impl mofa_kernel::agent::components::tool::Tool for AddTool {
    fn name(&self) -> &str {
        "add"
    }

    fn description(&self) -> &str {
        "Adds two integers and returns the result."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "a": { "type": "integer", "description": "First operand" },
                "b": { "type": "integer", "description": "Second operand" }
            },
            "required": ["a", "b"]
        })
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        let a = input
            .arguments
            .get("a")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let b = input
            .arguments
            .get("b")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        ToolResult::success(serde_json::json!({ "result": a + b }))
    }
}

// ============================================================================
// Entry point
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = McpHostConfig::new("mofa-agent", "127.0.0.1", 3000)
        .with_instructions(
            "A MoFA agent exposing its tools over MCP. \
             Connect from Claude Desktop, Cursor, or any MCP-compatible client.",
        );

    let mut server = McpServerManager::new(config);
    server.register_tool(EchoTool.into_dynamic())?;
    server.register_tool(AddTool.into_dynamic())?;

    tracing::info!("Registered tools: {:?}", server.registered_tools());
    tracing::info!("Connect MCP clients to: http://127.0.0.1:3000/mcp");

    let ct = CancellationToken::new();

    // Shut down on Ctrl-C
    let ct_clone = ct.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Received Ctrl-C, shutting down...");
        ct_clone.cancel();
    });

    server.serve_with_cancellation(ct).await?;

    Ok(())
}
