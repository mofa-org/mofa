# Chapter 8: Plugins and Scripting

> **Learning objectives:** Understand the `AgentPlugin` trait lifecycle, write a Rhai script plugin, enable hot-reloading, and understand when to use compile-time vs. runtime plugins.

## The Dual-Layer Plugin System

As introduced in Chapter 1, MoFA has two plugin layers:

| Layer | Language | When to Use |
|-------|----------|-------------|
| **Compile-time** | Rust / WASM | Performance-critical paths: LLM adapters, data processing, native APIs |
| **Runtime** | Rhai scripts | Business logic, content filters, rules engines, anything that changes frequently |

Both layers implement the same `AgentPlugin` trait, so the system manages them uniformly.

## The AgentPlugin Trait

Every plugin follows a well-defined lifecycle:

```rust
// crates/mofa-kernel/src/plugin/mod.rs

#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;
    fn state(&self) -> PluginState;

    // Lifecycle methods — called in this order:
    async fn load(&mut self, ctx: &PluginContext) -> PluginResult<()>;
    async fn init_plugin(&mut self) -> PluginResult<()>;
    async fn start(&mut self) -> PluginResult<()>;
    async fn pause(&mut self) -> PluginResult<()>;   // optional
    async fn resume(&mut self) -> PluginResult<()>;  // optional
    async fn stop(&mut self) -> PluginResult<()>;
    async fn unload(&mut self) -> PluginResult<()>;

    // Main execution
    async fn execute(&mut self, input: String) -> PluginResult<String>;
    async fn health_check(&self) -> PluginResult<bool>;
}
```

The lifecycle progression:

```
load → init_plugin → start → [execute...] → stop → unload
                       ↕
                  pause / resume
```

### PluginMetadata

Each plugin declares its identity and capabilities:

```rust
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginType,
    pub priority: PluginPriority,
    pub dependencies: Vec<String>,
    pub capabilities: Vec<String>,
}
```

Plugin types include:

```rust
pub enum PluginType {
    LLM,       // LLM provider adapter
    Tool,      // Tool implementation
    Storage,   // Persistence backend
    Memory,    // Memory implementation
    Scripting, // Script engine (Rhai, etc.)
    Skill,     // Skill package
    Custom(String),
}
```

## Rhai: The Runtime Scripting Engine

[Rhai](https://rhai.rs/) is a lightweight, fast, embedded scripting language designed for Rust. MoFA uses it for runtime plugins because:

- **Hot-reloadable**: Change the script, see results immediately (no recompile)
- **Sandboxed**: Scripts can't access the filesystem or network unless you explicitly allow it
- **Rust-friendly**: Easy to call Rust functions from Rhai and vice versa
- **Fast**: Compiled to bytecode, much faster than interpreted languages

### Basic Rhai Syntax

```javascript
// Variables
let x = 42;
let name = "MoFA";

// Functions
fn greet(name) {
    "Hello, " + name + "!"
}

// Conditionals
if x > 40 {
    print("x is big");
} else {
    print("x is small");
}

// Objects (maps)
let config = #{
    max_retries: 3,
    timeout: 30,
    enabled: true
};

// JSON processing (built-in)
let data = parse_json(input);
let result = #{
    processed: true,
    original: data
};
to_json(result)
```

## Build: A Hot-Reloadable Content Filter

Let's build a Rhai plugin that filters content based on rules that can be updated at runtime without restarting the application.

Create a new project:

```bash
cargo new content_filter
cd content_filter
mkdir -p plugins
```

First, create the Rhai script. Write `plugins/content_filter.rhai`:

```javascript
// Content filter rules — edit this file and the plugin reloads automatically!

// List of blocked words
let blocked_words = ["spam", "scam", "phishing"];

// Process the input
fn process(input) {
    let text = input.to_lower();
    let issues = [];

    // Check for blocked words
    for word in blocked_words {
        if text.contains(word) {
            issues.push("Contains blocked word: " + word);
        }
    }

    // Check text length
    if input.len() > 1000 {
        issues.push("Text exceeds 1000 character limit");
    }

    // Check for excessive caps (shouting)
    let upper_count = 0;
    for ch in input.chars() {
        if ch >= 'A' && ch <= 'Z' {
            upper_count += 1;
        }
    }
    if input.len() > 10 && upper_count * 100 / input.len() > 70 {
        issues.push("Too many capital letters (possible shouting)");
    }

    // Build result
    if issues.is_empty() {
        to_json(#{
            status: "approved",
            message: "Content passed all checks"
        })
    } else {
        to_json(#{
            status: "rejected",
            issues: issues,
            message: "Content failed " + issues.len() + " check(s)"
        })
    }
}

// Entry point — called by the plugin system
process(input)
```

Now write `Cargo.toml`:

```toml
[package]
name = "content_filter"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
mofa-plugins = { path = "../../crates/mofa-plugins" }
mofa-kernel = { path = "../../crates/mofa-kernel" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

Write `src/main.rs`:

```rust
use mofa_kernel::plugin::PluginContext;
use mofa_plugins::rhai_runtime::{RhaiPlugin, RhaiPluginConfig};
use std::path::Path;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plugin_path = Path::new("plugins/content_filter.rhai");

    // --- Step 1: Create and initialize the Rhai plugin ---
    let config = RhaiPluginConfig::new_file("content_filter", plugin_path);
    let mut plugin = RhaiPlugin::new(config).await?;

    let ctx = PluginContext::new("tutorial_agent");
    plugin.load(&ctx).await?;
    plugin.init_plugin().await?;
    plugin.start().await?;

    println!("Content filter plugin loaded and started!\n");

    // --- Step 2: Test with various inputs ---
    let test_inputs = vec![
        "Hello, this is a normal message about Rust programming.",
        "CLICK HERE FOR FREE MONEY! This is totally not a scam!",
        "Buy our product! No spam involved, we promise.",
        "THIS IS ALL CAPS AND VERY SHOUTY MESSAGE HERE!!!",
        "A short, friendly note.",
    ];

    for input in &test_inputs {
        let result = plugin.execute(input.to_string()).await?;
        let parsed: serde_json::Value = serde_json::from_str(&result)?;
        println!("Input:  \"{}\"", &input[..input.len().min(50)]);
        println!("Result: {} — {}\n",
            parsed["status"].as_str().unwrap_or("?"),
            parsed["message"].as_str().unwrap_or("?"),
        );
    }

    // --- Step 3: Hot-reload demonstration ---
    println!("=== Hot Reload Demo ===");
    println!("Modify plugins/content_filter.rhai and watch the output change!");
    println!("Press Ctrl+C to stop.\n");

    // Poll for changes and re-execute
    let test_message = "Check this spam content for compliance.";
    let mut last_modified = std::fs::metadata(plugin_path)?.modified()?;

    for i in 1..=30 {
        // Check if file was modified
        let current_modified = std::fs::metadata(plugin_path)?.modified()?;
        if current_modified != last_modified {
            println!("  [Reload] Script changed, reloading...");

            // Reload the plugin
            plugin.stop().await?;
            plugin.unload().await?;

            let config = RhaiPluginConfig::new_file("content_filter", plugin_path);
            plugin = RhaiPlugin::new(config).await?;
            plugin.load(&ctx).await?;
            plugin.init_plugin().await?;
            plugin.start().await?;

            last_modified = current_modified;
            println!("  [Reload] Done!");
        }

        let result = plugin.execute(test_message.to_string()).await?;
        println!("  [{}] {}", i, result);

        time::sleep(time::Duration::from_secs(2)).await;
    }

    // --- Cleanup ---
    plugin.stop().await?;
    plugin.unload().await?;

    Ok(())
}
```

Run it:

```bash
cargo run
```

While it's running, try editing `plugins/content_filter.rhai` — for example, add "compliance" to the `blocked_words` list. The plugin will reload and the output will change.

## What Just Happened?

1. **`RhaiPluginConfig::new_file()`** — Points the plugin to a Rhai script file
2. **`RhaiPlugin::new(config)`** — Creates the plugin (compiles the script)
3. **Lifecycle**: `load → init_plugin → start` prepares the plugin for execution
4. **`plugin.execute(input)`** — Runs the Rhai script with `input` as a variable
5. **Hot-reload**: We detect file changes and recreate the plugin, which recompiles the script

> **Architecture note:** `RhaiPlugin` lives in `mofa-plugins` (`crates/mofa-plugins/src/rhai_runtime/plugin.rs`). The underlying Rhai engine is in `mofa-extra` (`crates/mofa-extra/src/rhai/`). The `AgentPlugin` trait is in `mofa-kernel`. This follows the architecture: kernel defines the interface, plugins provide the implementation.

## Plugin Manager

In a real application, you'd use `PluginManager` to handle multiple plugins:

```rust
use mofa_sdk::plugins::PluginManager;

let mut manager = PluginManager::new();

// Register plugins
manager.register(Box::new(content_filter_plugin));
manager.register(Box::new(analytics_plugin));
manager.register(Box::new(logging_plugin));

// Initialize all plugins
manager.init_all().await?;

// Start all plugins
manager.start_all().await?;

// Execute a specific plugin
let result = manager.execute("content_filter", input).await?;
```

## Integrating Plugins with LLMAgent

Plugins can be attached to an `LLMAgent` via the builder:

```rust
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_plugin(content_filter_plugin)
    .with_plugin(analytics_plugin)
    .build();
```

The agent will call plugin hooks during its lifecycle — for example, `before_chat` and `after_chat` events let plugins intercept and modify messages.

## WASM Plugins (Advanced)

For performance-critical plugins that still need to be dynamically loadable, MoFA supports WASM:

```rust
use mofa_sdk::plugins::WasmPlugin;

// Load a compiled WASM module
let plugin = WasmPlugin::from_file("plugins/my_plugin.wasm").await?;
```

WASM plugins are compiled from Rust (or any language that targets WASM) and run in a sandboxed environment. They're faster than Rhai scripts but require recompilation when changed.

> **When to use which?**
> - **Rhai**: Business rules, content filters, workflow logic — anything that changes frequently and doesn't need extreme performance
> - **WASM**: Data processing, encryption, compression — computationally intensive tasks that benefit from native-like speed
> - **Native Rust**: LLM providers, database adapters, core infrastructure — things that rarely change and need the full Rust ecosystem

## Key Takeaways

- `AgentPlugin` defines a lifecycle: `load → init → start → execute → stop → unload`
- Plugins have metadata (id, name, type, priority, dependencies)
- Rhai scripts are the runtime plugin layer — hot-reloadable, sandboxed, fast
- Hot-reload: detect file changes, stop the old plugin, create a new one from the updated script
- `PluginManager` handles multiple plugins in a real application
- WASM plugins offer dynamic loading with near-native performance
- Choose Rhai for flexibility, WASM for performance, native Rust for infrastructure

---

**Next:** [Chapter 9: What's Next](09-whats-next.md) — Contributing, GSoC ideas, and advanced topics.

[← Back to Table of Contents](README.md)

---

**English** | [简体中文](../zh-CN/tutorial/08-plugins-and-scripting.md)
