//! MCP Tool Integration Example
//!
//! Demonstrates how to:
//!
//! 1. Connect to an MCP server via the stdio transport (spawning a child process).
//! 2. Discover all tools the server exposes.
//! 3. Call a specific tool and print the result.
//! 4. Register the MCP tools into a [`ToolRegistry`] so they can be used by
//!    any MoFA agent alongside builtin tools.
//!
//! ## Prerequisites
//!
//! The example uses the official
//! [`@modelcontextprotocol/server-filesystem`](https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem)
//! reference server.  Install it globally (Node ≥ 18):
//!
//! ```text
//! npm install -g @modelcontextprotocol/server-filesystem
//! ```
//!
//! Alternatively, install it locally and ensure `npx` is on `$PATH`.
//!
//! ## Running
//!
//! ```text
//! cargo run -p mcp_tools
//! ```
//!
//! The server is given access to the system temp directory by default.  Pass a
//! different directory as a CLI argument to restrict or expand access:
//!
//! ```text
//! cargo run -p mcp_tools -- /path/to/allowed/dir
//! ```

use mofa_foundation::agent::tools::ToolRegistry;
use mofa_kernel::agent::components::mcp::{McpClient, McpServerConfig};
use mofa_foundation::agent::tools::mcp::McpClientManager;
use std::env;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialise tracing so we can see MCP-related log output.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // The directory the filesystem server will be allowed to access.
    let allowed_dir = env::args()
        .nth(1)
        .unwrap_or_else(|| env::temp_dir().to_string_lossy().to_string());

    println!("═══════════════════════════════════════════════════════════");
    println!("  MoFA — MCP Tool Integration Example");
    println!("  Filesystem server root: {allowed_dir}");
    println!("═══════════════════════════════════════════════════════════\n");

    // ── Step 1: Connect via McpClientManager directly ──────────────────────
    //
    // McpClientManager is the low-level client.  You can use it directly when
    // you only need the MCP protocol without the full ToolRegistry machinery.

    println!("Step 1 — Connecting directly via McpClientManager …");

    let config = McpServerConfig::stdio(
        "filesystem",
        "npx",
        vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
            allowed_dir.clone(),
        ],
    );

    let mut manager = McpClientManager::new();
    manager.connect(config.clone()).await.map_err(|e| {
        eprintln!(
            "\n[ERROR] Could not connect to MCP server: {e}\n\n\
             Make sure `npx` is installed and the package is available:\n\
             \tnpm install -g @modelcontextprotocol/server-filesystem\n"
        );
        e
    })?;

    info!("Connected.  Fetching server info …");
    let server_info = manager.server_info("filesystem").await?;
    println!(
        "  Server: {} v{}",
        server_info.name, server_info.version
    );
    if let Some(instructions) = &server_info.instructions {
        println!("  Instructions: {instructions}");
    }

    // ── Step 2: Discover available tools ───────────────────────────────────

    println!("\nStep 2 — Listing available tools …");

    let tools = manager.list_tools("filesystem").await?;
    println!("  Found {} tool(s):", tools.len());
    for tool in &tools {
        println!("    • {} — {}", tool.name, tool.description);
    }

    // ── Step 3: Call a tool directly (list_directory) ──────────────────────

    println!("\nStep 3 — Calling `list_directory` on {allowed_dir} …");

    let raw_result = manager
        .call_tool(
            "filesystem",
            "list_directory",
            serde_json::json!({ "path": allowed_dir }),
        )
        .await?;

    println!(
        "  Raw response:\n{}",
        serde_json::to_string_pretty(&raw_result)?
    );

    manager.disconnect("filesystem").await?;
    info!("Direct McpClientManager connection closed.");

    // ── Step 4: Register all MCP tools in ToolRegistry ────────────────────
    //
    // ToolRegistry is the agent-level abstraction.  After calling
    // `load_mcp_server`, each MCP tool is automatically wrapped in a
    // McpToolAdapter and registered as a standard MoFA `Tool`.  Any agent
    // that holds the registry can call these tools transparently.

    println!("\nStep 4 — Loading MCP server tools into ToolRegistry …");

    let mut registry = ToolRegistry::new();
    let registered_names = registry.load_mcp_server(config).await?;

    println!(
        "  Registered {} MCP tool(s) into ToolRegistry:",
        registered_names.len()
    );
    for name in &registered_names {
        println!("    ✓ {name}");
    }

    // Demonstrate that MCP tools are discoverable via filter_by_source.
    let mcp_tools = registry.filter_by_source("mcp");
    println!("\n  filter_by_source(\"mcp\") returned {} tool(s).", mcp_tools.len());

    // ── Step 5: Invoke an MCP tool through the registry ───────────────────

    println!("\nStep 5 — Calling `list_directory` through ToolRegistry …");

    // Both traits must be in scope: ToolRegistry for .get().
    use mofa_kernel::agent::components::tool::ToolRegistry as _;
    use mofa_kernel::agent::context::AgentContext;

    if let Some(tool) = registry.get("list_directory") {
        // AgentContext requires an execution ID; use any descriptive string.
        let ctx = AgentContext::new("mcp-demo");

        match tool
            .execute_dynamic(serde_json::json!({ "path": allowed_dir }), &ctx)
            .await
        {
            Ok(output) => {
                println!(
                    "  Result:\n{}",
                    serde_json::to_string_pretty(&output)?
                );
            }
            Err(e) => eprintln!("  Tool call failed: {e}"),
        }
    } else {
        println!("  `list_directory` not found in registry (server may not expose it).");
    }

    // ── Unload the server and verify cleanup ──────────────────────────────

    println!("\nStep 6 — Unloading MCP server from registry …");

    let removed = registry.unload_mcp_server("filesystem").await?;
    println!("  Removed {} tool(s): {removed:?}", removed.len());

    let remaining_mcp = registry.filter_by_source("mcp");
    assert!(
        remaining_mcp.is_empty(),
        "All MCP tools must be removed after unload"
    );
    println!("  Registry is now free of MCP tools. ✓");

    println!("\n═══════════════════════════════════════════════════════════");
    println!("  Done!  See docs/usage.md for full MCP setup documentation.");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
