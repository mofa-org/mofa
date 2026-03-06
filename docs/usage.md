# Usage Guide

## Persistence Feature

If the persistence feature is enabled, PostgreSQL (version 18 recommended) is the primary backend with `uuid_v7` support.

---

## Quick Start

### Persistence
- `examples/streaming_persistence`

### Initialize Agent from Database
- `examples/agent_from_database_streaming`

### Using TTS Plugin
- `examples/llm_tts_streaming`

The examples above cover the most common workflow for enterprise agent development: create session → configure agent → initiate conversation → log to database.

---

# Creating Agents

MoFA provides `LLMAgentBuilder` as a builder pattern for LLM agents, supporting chained configuration of agent properties.

## Basic Usage

### 1. Simplest LLM Agent Creation

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};

// Create from environment variables
let agent = LLMAgentBuilder::new()
    .with_provider(std::sync::Arc::new(OpenAIProvider::from_env()))
    .build();
```

### 2. Full Configuration Example

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;
use uuid::Uuid;

let provider = OpenAIProvider::from_env();

let agent = LLMAgentBuilder::new()
    .with_id(Uuid::new_v4().to_string())  // Must use UUID format, or omit for auto-generation
    .with_name("My LLM Agent".to_string())
    .with_provider(Arc::new(provider))
    .with_system_prompt("You are a helpful AI assistant.".to_string())
    .with_temperature(0.7)
    .with_max_tokens(2048)
    .build();
```

### 3. Agent with Tool Calling

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider, ToolExecutor, ToolPluginExecutor};
use mofa_sdk::plugins::tools::create_builtin_tool_plugin;
use std::sync::Arc;

// Create built-in tool plugin (includes HTTP, filesystem, shell, calculator, etc.)
let mut tool_plugin = create_builtin_tool_plugin("comprehensive_tools")?;
tool_plugin.init_plugin().await?;

// Create adapter to connect to LLM (auto-discovers tools)
let executor: Arc<dyn ToolExecutor> = Arc::new(ToolPluginExecutor::new(tool_plugin));

// Build agent with tools
let agent = LLMAgentBuilder::new()
    .with_name("Tool Assistant".to_string())
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_system_prompt("You are an AI assistant that can use tools.".to_string())
    .with_tool_executor(executor)
    .build();
```

### 4. Agent with Persistence

```rust
use mofa_sdk::llm::LLMAgentBuilder;
use mofa_sdk::persistence::{PersistencePlugin, PostgresStore};
use std::sync::Arc;
use uuid::Uuid;

let user_id = Uuid::now_v7();
let tenant_id = Uuid::now_v7();
let agent_id = Uuid::now_v7();
let session_id = Uuid::now_v7();

// Create persistence plugin
let store = Arc::new(PostgresStore::connect("postgresql://...").await?);
let persistence = PersistencePlugin::new(
    "persistence-plugin",
    store,
    user_id,
    tenant_id,
    agent_id,
    session_id,
);

let agent = LLMAgentBuilder::from_env()?
    .with_id(agent_id.to_string())
    .with_session_id(session_id.to_string())
    .with_sliding_window(20)  // Keep last 20 conversation turns
    .with_persistence_plugin(persistence)
    .build_async()
    .await;
```

### 5. Official AgentLoop (with ContextBuilder + Session)

```rust
use mofa_sdk::llm::{AgentLoop, AgentLoopConfig, AgentLoopRunner, AgentContextBuilder, ChatSession, LLMClient, OpenAIProvider, ToolExecutor};
use std::path::PathBuf;
use std::sync::Arc;

let provider = Arc::new(OpenAIProvider::from_env());
let tool_executor: Arc<dyn ToolExecutor> = /* your executor */;

let loop_config = AgentLoopConfig::default();
let agent_loop = AgentLoop::new(provider.clone(), tool_executor.clone(), loop_config);

let workspace = PathBuf::from("./workspace");
let context_builder = AgentContextBuilder::new(workspace);

let client = LLMClient::new(provider);
let mut session = ChatSession::new(client).with_tool_executor(tool_executor);

let mut runner = AgentLoopRunner::new(agent_loop)
    .with_context_builder(context_builder)
    .with_session(session);

let reply = runner
    .run(
        "Please analyze this image",
        Some(vec!["/path/to/image.png".to_string()]),
    )
    .await?;
```

### 6. Agent with TTS Plugin

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use mofa_sdk::plugins::TTSPlugin;
use uuid::Uuid;

// Using TTS plugin (client)
let agent = Arc::new(
    LLMAgentBuilder::new()
    .with_id(Uuid::new_v4().to_string())
    .with_name("Chat TTS Agent")
    .with_session_id(Uuid::new_v4().to_string())
    .with_provider(Arc::new(openai_from_env()?))
    .with_system_prompt("You are a friendly AI assistant.")
    .with_temperature(0.7)
    .with_plugin(TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_088")))
    .build();
);
```

### 7. Agent with Rhai Runtime Plugin

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use mofa_sdk::plugins::{RhaiPlugin, RhaiPluginConfig, PluginContext};

// Create Rhai plugin (supports hot reload)
let config = RhaiPluginConfig::new_file("dynamic_rules", "./rules/plugin.rhai");
let mut rhai_plugin = RhaiPlugin::new(config).await?;

let ctx = PluginContext::new("rules_engine_agent");
rhai_plugin.load(&ctx).await?;
rhai_plugin.init_plugin().await?;
rhai_plugin.start().await?;

let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_plugin(rhai_plugin)
    .build();
```

### 8. Multi-Tenant Scenario

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use uuid::Uuid;

let agent = LLMAgentBuilder::new()
    .with_id(Uuid::new_v4().to_string())
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_user("user_abc".to_string())    // User isolation
    .with_tenant("tenant_xyz".to_string()) // Tenant isolation
    .build();
```

### 9. With Event Handling

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider, LLMAgentEventHandler};

struct MyEventHandler;

impl LLMAgentEventHandler for MyEventHandler {
    fn on_message_start(&self, msg: &str) {
        println!("Message processing started: {}", msg);
    }

    fn on_message_complete(&self, result: &str) {
        println!("Message processing completed: {}", result);
    }
}

let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_event_handler(Box::new(MyEventHandler))
    .build();
```

### 10. Using Hot-Reloadable Prompt Templates

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider, HotReloadableRhaiPromptPlugin};

// Supports runtime prompt modification without restart
let prompt_plugin = HotReloadableRhaiPromptPlugin::new("./prompts/template.rhai")?;

let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_prompt_plugin(prompt_plugin)
    .build();
```

## LLMAgentBuilder Method Reference

### Core Configuration

| Method | Parameter | Description | Default |
|--------|-----------|-------------|---------|
| `new()` | - | Create new Builder instance | - |
| `with_id()` | `id: String` | Set agent ID (UUID format only) | Auto-generated UUID v7 |
| `with_name()` | `name: String` | Set agent name | - |
| `with_provider()` | `provider: Arc<dyn LLMProvider>` | Set LLM provider | **Required** |
| `with_system_prompt()` | `prompt: String` | Set system prompt | - |
| `with_temperature()` | `temperature: f32` | Set temperature (0.0-1.0) | - |
| `with_max_tokens()` | `max_tokens: u32` | Set max output tokens | - |

### Tools and Execution

| Method | Parameter | Description | Default |
|--------|-----------|-------------|---------|
| `with_tool()` | `tool: Tool` | Add single tool | - |
| `with_tools()` | `tools: Vec<Tool>` | Add multiple tools | - |
| `with_tool_executor()` | `executor: Arc<dyn ToolExecutor>` | Set tool executor | - |

### Plugin System

| Method | Parameter | Description |
|--------|-----------|-------------|
| `with_plugin()` | `plugin: AgentPlugin` | Add single plugin |
| `with_plugins()` | `plugins: Vec<Box<dyn AgentPlugin>>` | Add multiple plugins |
| `with_tts_engine()` | `tts_engine: TTSPlugin` | Set TTS plugin |
| `with_prompt_plugin()` | `plugin: PromptTemplatePlugin` | Set prompt template plugin |
| `with_hot_reload_prompt_plugin()` | `plugin: HotReloadableRhaiPromptPlugin` | Set hot-reload prompt plugin |

### Events and Persistence

| Method | Parameter | Description |
|--------|-----------|-------------|
| `with_event_handler()` | `handler: Box<dyn LLMAgentEventHandler>` | Set event handler |
| `with_persistence_plugin()` | `plugin: PersistencePlugin` | Add persistence plugin |

### Session Management

| Method | Parameter | Description | Default |
|--------|-----------|-------------|---------|
| `with_session_id()` | `session_id: String` | Set session ID | - |
| `with_sliding_window()` | `size: usize` | Set context window size (turns) | - |

### Multi-Tenancy

| Method | Parameter | Description |
|--------|-----------|-------------|
| `with_user()` | `user_id: String` | Set user ID |
| `with_tenant()` | `tenant_id: String` | Set tenant ID |

### Configuration Helpers

| Method | Parameter | Description |
|--------|-----------|-------------|
| `with_config()` | `key: String, value: String` | Add custom configuration |
| `from_env()` | - | Create configuration from environment |

### Build Methods

| Method | Description |
|--------|-------------|
| `build()` | Synchronous build (panics if provider not set) |
| `try_build()` | Synchronous build, returns Result |
| `build_async()` | Async build (supports loading from database) |

---

# MCP (Model Context Protocol) Tool Integration

MoFA supports connecting to any [MCP](https://modelcontextprotocol.io) server and automatically surfacing its tools inside the MoFA `ToolRegistry`.  This means you can add hundreds of community-built MCP tools to any agent with just a few lines of code.

## Prerequisites

MCP support requires the `mcp` feature flag.  Add it to your `Cargo.toml`:

```toml
mofa-foundation = { version = "...", features = ["mcp"] }
```

> **Note:** Calling `load_mcp_server` without the `mcp` feature generates a tracing `WARN` log and returns an empty tool list.  No tools are silently dropped; a message explaining how to fix it is always emitted.

## Quick Start — Filesystem Server

The `@modelcontextprotocol/server-filesystem` reference server is a good starting point.

### 1. Install the server (one-time)

```text
npm install -g @modelcontextprotocol/server-filesystem
```

### 2. Connect and discover tools

```rust,ignore
use mofa_foundation::agent::tools::ToolRegistry;
use mofa_kernel::agent::components::mcp::McpServerConfig;

let config = McpServerConfig::stdio(
    "filesystem",               // logical server name
    "npx",
    vec![
        "-y".to_string(),
        "@modelcontextprotocol/server-filesystem".to_string(),
        "/tmp".to_string(),     // allowed root directory
    ],
);

let mut registry = ToolRegistry::new();
let tool_names = registry.load_mcp_server(config).await?;

println!("Loaded {} MCP tools: {:?}", tool_names.len(), tool_names);
```

### 3. Call an MCP tool through the registry

```rust,ignore
use mofa_kernel::agent::components::tool::DynTool;
use mofa_kernel::agent::context::AgentContext;

let tool = registry.get("list_directory").expect("tool not found");

// AgentContext requires a unique execution ID string.
let ctx = AgentContext::new("my-execution");

match tool
    .execute_dynamic(serde_json::json!({ "path": "/tmp" }), &ctx)
    .await
{
    Ok(output) => println!("{}", serde_json::to_string_pretty(&output)?),
    Err(e) => eprintln!("Tool call failed: {e}"),
}
```

### 4. Unload a server at runtime

```rust,ignore
let removed = registry.unload_mcp_server("filesystem").await?;
println!("Removed {} tools", removed.len());
```

## Low-Level API — McpClientManager

`McpClientManager` is the low-level client that manages the raw MCP connections.  Use it when you need direct protocol access without the `ToolRegistry` abstraction.

```rust,ignore
use mofa_foundation::agent::tools::mcp::McpClientManager;
use mofa_kernel::agent::components::mcp::{McpClient, McpServerConfig};

let config = McpServerConfig::stdio(
    "my-server",
    "npx",
    vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()],
).with_env("GITHUB_TOKEN", &std::env::var("GITHUB_TOKEN")?);

let mut manager = McpClientManager::new();
manager.connect(config).await?;

// Discover tools
let tools = manager.list_tools("my-server").await?;
for tool in &tools {
    println!("{}: {}", tool.name, tool.description);
}

// Call a tool
let result = manager.call_tool(
    "my-server",
    "list_repos",
    serde_json::json!({ "owner": "mofa-org" }),
).await?;

// Graceful disconnect
manager.disconnect("my-server").await?;
```

## Supported Transports

| Transport | Status | Notes |
|-----------|--------|-------|
| `stdio` (child process) | ✅ Supported | Start any MCP server as a subprocess |
| `HTTP/SSE` | ⏳ Planned | `transport-streamable-http-client-reqwest` feature not yet bundled |

## Environment Variables

When using the `stdio` transport you can inject environment variables into the child process:

```rust,ignore
let config = McpServerConfig::stdio("github", "npx", args)
    .with_env("GITHUB_TOKEN",  "ghp_xxxxx")
    .with_env("GITHUB_OWNER",  "mofa-org");
```

## Example

A runnable end-to-end example is located at [`examples/mcp_tools/`](../examples/mcp_tools/).

```text
cargo run -p mcp_tools
```

## Running Integration Tests

Integration tests are in `crates/mofa-foundation/tests/mcp_integration.rs`.

Tests that do **not** require a live server run by default:

```text
cargo test -p mofa-foundation --features mcp
```

Tests against a live `@modelcontextprotocol/server-filesystem` are marked `#[ignore]`.  Run them with:

```text
cargo test -p mofa-foundation --features mcp -- --ignored
```

---

# UniFFI

Generate Python bindings:

```bash
cd crates/mofa-sdk
./generate-bindings.sh python
```

---

**English** | [简体中文](zh-CN/usage.md)
