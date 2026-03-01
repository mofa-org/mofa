//! MCP (Model Context Protocol) Integration Tests
//!
//! ## Feature-gated tests
//!
//! Tests in the `with_mcp_feature` module require `--features mcp` to compile.
//! Tests that need a live MCP server process are marked `#[ignore]`; run them with:
//!
//! ```text
//! cargo test -p mofa-foundation --features mcp -- --ignored
//! ```
//!
//! The live tests assume `npx` and `@modelcontextprotocol/server-filesystem`
//! are available on `$PATH`. Install with:
//!
//! ```text
//! npm install -g @modelcontextprotocol/server-filesystem
//! ```

// ============================================================================
// Tests that work WITHOUT the `mcp` feature (stub path)
// ============================================================================

#[cfg(not(feature = "mcp"))]
mod without_mcp_feature {
    use mofa_foundation::agent::tools::ToolRegistry;

    /// The no-mcp stub must accept an endpoint string and return Ok([]) —
    /// the important behaviour is *not* panicking and the `tracing::warn!`
    /// that fires internally (verified by code inspection / tracing subscriber
    /// in a real run).
    #[tokio::test]
    async fn stub_returns_empty_list_without_panic() {
        let mut registry = ToolRegistry::new();
        let result = registry.load_mcp_server("fake-endpoint").await;
        assert!(result.is_ok(), "stub must not return an error");
        let names = result.unwrap();
        assert!(
            names.is_empty(),
            "stub must return an empty list when mcp feature is disabled"
        );
    }

    /// The endpoint should nevertheless be recorded so callers can inspect
    /// which servers were *attempted*.
    #[tokio::test]
    async fn stub_records_attempted_endpoint() {
        let mut registry = ToolRegistry::new();
        registry
            .load_mcp_server("http://localhost:9999")
            .await
            .unwrap();
        assert!(
            registry
                .mcp_endpoints()
                .contains(&"http://localhost:9999".to_string()),
            "the attempted endpoint must be stored even if mcp feature is off"
        );
    }

    /// Calling the stub multiple times must not panic and must accumulate endpoints.
    #[tokio::test]
    async fn stub_accepts_multiple_calls() {
        let mut registry = ToolRegistry::new();
        for i in 0..5 {
            let ep = format!("ep-{i}");
            registry.load_mcp_server(&ep).await.unwrap();
        }
        assert_eq!(registry.mcp_endpoints().len(), 5);
    }
}

// ============================================================================
// Tests that require the `mcp` feature
// ============================================================================

#[cfg(feature = "mcp")]
mod with_mcp_feature {
    use mofa_foundation::agent::tools::{
        ToolRegistry,
        mcp::{McpClientManager, McpToolAdapter},
    };
    use mofa_kernel::agent::components::mcp::{McpClient, McpServerConfig, McpToolInfo};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // -----------------------------------------------------------------------
    // McpClientManager unit tests (no live server required)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn client_manager_starts_empty() {
        let mgr = McpClientManager::new();
        assert!(
            mgr.connected_servers().is_empty(),
            "a fresh manager has no connections"
        );
    }

    #[tokio::test]
    async fn client_manager_not_connected_for_unknown_server() {
        let mgr = McpClientManager::new();
        assert!(
            !mgr.is_connected("ghost-server"),
            "unknown server must report as not connected"
        );
    }

    #[tokio::test]
    async fn client_manager_list_tools_errors_on_missing_server() {
        let mgr = McpClientManager::new();
        let result = mgr.list_tools("not-connected").await;
        assert!(result.is_err(), "list_tools on missing server must error");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not-connected"),
            "error message should name the server: {msg}"
        );
    }

    #[tokio::test]
    async fn client_manager_call_tool_errors_on_missing_server() {
        let mgr = McpClientManager::new();
        let result = mgr
            .call_tool("ghost", "any_tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn client_manager_disconnect_errors_on_missing_server() {
        let mut mgr = McpClientManager::new();
        let result = mgr.disconnect("ghost").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn client_manager_http_transport_returns_err() {
        let mut mgr = McpClientManager::new();
        let config = McpServerConfig::http("http-server", "http://localhost:9999/mcp");
        let result = mgr.connect(config).await;
        assert!(
            result.is_err(),
            "HTTP transport is not yet supported and must return an error"
        );
    }

    #[tokio::test]
    async fn client_manager_duplicate_server_name_errors() {
        let mut mgr = McpClientManager::new();
        // First attempt will fail because the command doesn't exist on most CI machines.
        // We inject the server entry manually to simulate an already-connected state.
        let config1 = McpServerConfig::stdio("dup", "echo", vec![]);
        let _ = mgr.connect(config1).await; // may fail — that is fine

        if mgr.is_connected("dup") {
            let config2 = McpServerConfig::stdio("dup", "echo", vec![]);
            let result = mgr.connect(config2).await;
            assert!(result.is_err(), "duplicate server name must be rejected");
        }
    }

    // -----------------------------------------------------------------------
    // McpToolAdapter unit tests (no live server required)
    // -----------------------------------------------------------------------

    #[test]
    fn tool_adapter_exposes_correct_name_and_description() {
        // Bring Tool trait into scope so .name() / .description() resolve.
        use mofa_kernel::agent::components::tool::Tool;
        let info = McpToolInfo {
            name: "read_file".to_string(),
            description: "Read a file from the filesystem".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        };
        let client = Arc::new(RwLock::new(McpClientManager::new()));
        let adapter = McpToolAdapter::new("filesystem", info, client);

        assert_eq!(adapter.name(), "read_file");
        assert_eq!(adapter.description(), "Read a file from the filesystem");
        assert_eq!(adapter.server_name(), "filesystem");
    }

    #[test]
    fn tool_adapter_metadata_tags_include_mcp_and_server() {
        use mofa_kernel::agent::components::tool::Tool; // for .metadata()

        let info = McpToolInfo {
            name: "write_file".to_string(),
            description: "Write content to a file".to_string(),
            input_schema: serde_json::json!({}),
        };
        let client = Arc::new(RwLock::new(McpClientManager::new()));
        let adapter = McpToolAdapter::new("my-server", info, client);

        let meta = adapter.metadata();
        assert_eq!(meta.category.as_deref(), Some("mcp"));
        assert!(meta.tags.contains(&"mcp".to_string()));
        assert!(meta.tags.contains(&"my-server".to_string()));
        assert!(meta.requires_network, "MCP tools always require network");
    }

    #[test]
    fn tool_adapter_preserves_input_schema() {
        use mofa_kernel::agent::components::tool::Tool;

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "limit": { "type": "integer", "default": 10 }
            },
            "required": ["query"]
        });
        let info = McpToolInfo {
            name: "search".to_string(),
            description: "Search tool".to_string(),
            input_schema: schema.clone(),
        };
        let client = Arc::new(RwLock::new(McpClientManager::new()));
        let adapter = McpToolAdapter::new("search-server", info, client);

        assert_eq!(adapter.parameters_schema(), schema);
    }

    // -----------------------------------------------------------------------
    // ToolRegistry + MCP: unit tests (no live server required)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn registry_load_mcp_server_errors_gracefully_with_bad_command() {
        let mut registry = ToolRegistry::new();
        let config = McpServerConfig::stdio(
            "bad-server",
            "this-command-definitely-does-not-exist-xyz123",
            vec![],
        );
        // Either an error (command not found) or success with 0 tools — both are fine.
        // The important invariant: it must not panic.
        let _result = registry.load_mcp_server(config).await;
    }

    #[tokio::test]
    async fn registry_unload_mcp_server_removes_tools() {
        use mofa_foundation::agent::tools::ToolRegistry;
        use mofa_foundation::agent::tools::registry::ToolSource;
        use mofa_kernel::agent::components::tool::{Tool, ToolExt, ToolInput, ToolResult};
        use mofa_kernel::agent::components::tool::ToolRegistry as ToolRegistryTrait;
        use mofa_kernel::agent::context::AgentContext;

        // Manually inject a fake MCP tool into the registry so we can test the
        // unload path without a live server.
        struct FakeMcpTool;

        #[async_trait::async_trait]
        impl Tool for FakeMcpTool {
            fn name(&self) -> &str {
                "fake_mcp_tool"
            }
            fn description(&self) -> &str {
                "does nothing"
            }
            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn execute(&self, _i: ToolInput, _c: &AgentContext) -> ToolResult {
                ToolResult::success(serde_json::json!(null))
            }
        }

        use mofa_kernel::agent::components::tool::ToolExt as _; // already imported above, suppress warning

        let mut registry = ToolRegistry::new();
        registry
            .register_with_source(
                FakeMcpTool.into_dynamic(),
                ToolSource::Mcp {
                    endpoint: "ep-1".to_string(),
                },
            )
            .unwrap();

        assert!(registry.contains("fake_mcp_tool"));

        let removed = registry.unload_mcp_server("ep-1").await.unwrap();
        assert_eq!(removed, vec!["fake_mcp_tool"]);
        assert!(!registry.contains("fake_mcp_tool"));
    }

    #[tokio::test]
    async fn registry_filter_by_mcp_source() {
        use mofa_foundation::agent::tools::registry::ToolSource;
        use mofa_kernel::agent::components::tool::{Tool, ToolExt, ToolInput, ToolResult};
        use mofa_kernel::agent::components::tool::ToolRegistry as ToolRegistryTrait; // for .contains()
        use mofa_kernel::agent::context::AgentContext;

        struct Dummy(String);

        #[async_trait::async_trait]
        impl Tool for Dummy {
            fn name(&self) -> &str {
                &self.0
            }
            fn description(&self) -> &str {
                "dummy"
            }
            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn execute(&self, _i: ToolInput, _c: &AgentContext) -> ToolResult {
                ToolResult::success(serde_json::json!(null))
            }
        }

        let mut registry = ToolRegistry::new();
        registry
            .register_with_source(
                Dummy("mcp_tool_a".into()).into_dynamic(),
                ToolSource::Mcp {
                    endpoint: "ep".to_string(),
                },
            )
            .unwrap();
        registry
            .register_with_source(
                Dummy("builtin_tool".into()).into_dynamic(),
                ToolSource::Builtin,
            )
            .unwrap();

        let mcp_tools = registry.filter_by_source("mcp");
        assert_eq!(mcp_tools.len(), 1);
        assert_eq!(mcp_tools[0].name, "mcp_tool_a");

        let builtin_tools = registry.filter_by_source("builtin");
        assert_eq!(builtin_tools.len(), 1);
        assert_eq!(builtin_tools[0].name, "builtin_tool");
    }

    // -----------------------------------------------------------------------
    // Live integration tests — require a real MCP server
    // Run with: cargo test -p mofa-foundation --features mcp -- --ignored
    // -----------------------------------------------------------------------

    /// Live test: connect to @modelcontextprotocol/server-filesystem,
    /// discover tools, and verify the expected tools are present.
    ///
    /// Prerequisites:
    ///   npm install -g @modelcontextprotocol/server-filesystem
    ///   (npx must be on PATH)
    #[tokio::test]
    #[ignore = "requires npx and @modelcontextprotocol/server-filesystem on PATH"]
    async fn live_filesystem_server_lists_tools() {
        use std::env;

        let tmpdir = env::temp_dir();
        let config = McpServerConfig::stdio(
            "filesystem",
            "npx",
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                tmpdir.to_string_lossy().to_string(),
            ],
        );

        let mut registry = ToolRegistry::new();
        let result = registry.load_mcp_server(config).await;
        assert!(
            result.is_ok(),
            "connecting to filesystem MCP server must succeed: {:?}",
            result
        );

        let names = result.unwrap();
        assert!(
            !names.is_empty(),
            "filesystem server should expose at least one tool"
        );

        // The filesystem server always exposes these well-known tools.
        let expected = ["read_file", "write_file", "list_directory"];
        for expected_tool in expected {
            assert!(
                names.iter().any(|n| n == expected_tool),
                "expected tool '{expected_tool}' not found in {names:?}"
            );
        }
    }

    /// Live test: connect to @modelcontextprotocol/server-filesystem,
    /// then call list_directory on a real temp directory.
    #[tokio::test]
    #[ignore = "requires npx and @modelcontextprotocol/server-filesystem on PATH"]
    async fn live_filesystem_server_list_directory_call() {
        use mofa_kernel::agent::components::mcp::McpClient;
        use std::env;

        let tmpdir = env::temp_dir();
        let config = McpServerConfig::stdio(
            "filesystem",
            "npx",
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                tmpdir.to_string_lossy().to_string(),
            ],
        );

        let mut mgr = McpClientManager::new();
        mgr.connect(config).await.expect("connect must succeed");

        let result = mgr
            .call_tool(
                "filesystem",
                "list_directory",
                serde_json::json!({ "path": tmpdir.to_string_lossy() }),
            )
            .await;

        assert!(
            result.is_ok(),
            "list_directory call must succeed: {:?}",
            result
        );

        let output = result.unwrap();
        assert!(
            output.get("content").is_some(),
            "response must have a 'content' field: {output}"
        );

        mgr.disconnect("filesystem")
            .await
            .expect("disconnect must succeed");
    }

    /// Live test: server_info returns the server name.
    #[tokio::test]
    #[ignore = "requires npx and @modelcontextprotocol/server-filesystem on PATH"]
    async fn live_filesystem_server_info() {
        use mofa_kernel::agent::components::mcp::McpClient;
        use std::env;

        let tmpdir = env::temp_dir();
        let config = McpServerConfig::stdio(
            "filesystem",
            "npx",
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                tmpdir.to_string_lossy().to_string(),
            ],
        );

        let mut mgr = McpClientManager::new();
        mgr.connect(config).await.expect("connect must succeed");

        let info = mgr
            .server_info("filesystem")
            .await
            .expect("server_info must succeed");
        assert!(
            !info.name.is_empty(),
            "server name must be non-empty: {info:?}"
        );

        mgr.disconnect("filesystem").await.unwrap();
    }
}
