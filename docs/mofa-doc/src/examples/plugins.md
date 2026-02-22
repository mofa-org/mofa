# Plugin Examples

Examples of MoFA's plugin system.

## Rhai Scripting

Hot-reloadable runtime plugins.

**Location:** `examples/rhai_scripting/`

```rust
use mofa_sdk::plugins::{RhaiPlugin, RhaiPluginManager};

#[tokio::main]
async fn main() -> Result<()> {
    let mut manager = RhaiPluginManager::new();

    // Load plugin from file
    let plugin = RhaiPlugin::from_file("./plugins/transform.rhai").await?;
    manager.register(plugin).await?;

    // Call plugin function
    let result = manager.call("transform", json!({"text": "hello world"})).await?;
    println!("Result: {:?}", result);

    Ok(())
}
```

### Rhai Plugin Script

```rhai
// plugins/transform.rhai

fn transform(input) {
    let text = input["text"];
    let upper = text.to_upper_case();
    let words = upper.split(" ");

    let result = [];
    for word in words {
        result.push(word);
    }

    result
}

fn on_init() {
    print("Transform plugin loaded!");
}
```

## Hot Reloading

Automatically reload plugins on file changes.

**Location:** `examples/rhai_hot_reload/`

```rust
use mofa_sdk::plugins::HotReloadWatcher;

#[tokio::main]
async fn main() -> Result<()> {
    let manager = Arc::new(RwLock::new(RhaiPluginManager::new()));

    // Watch for changes
    let watcher = HotReloadWatcher::new("./plugins/")?;

    let manager_clone = manager.clone();
    watcher.on_change(move |path| {
        let manager = manager_clone.clone();
        async move {
            let mut mgr = manager.write().await;
            mgr.reload(&path).await?;
            println!("Reloaded: {:?}", path);
            Ok(())
        }
    });

    // Keep running
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

## Rust Plugin

Compile-time plugin for maximum performance.

```rust
use mofa_sdk::kernel::plugin::{AgentPlugin, PluginContext, PluginResult};

pub struct LoggingPlugin {
    level: String,
}

#[async_trait]
impl AgentPlugin for LoggingPlugin {
    fn name(&self) -> &str { "logging" }
    fn version(&self) -> &str { "1.0.0" }

    async fn initialize(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        println!("Logging plugin initialized with level: {}", self.level);
        Ok(())
    }

    async fn on_before_execute(&self, input: &AgentInput) -> PluginResult<()> {
        println!("[{}] Input: {}", self.level, input.to_text());
        Ok(())
    }

    async fn on_after_execute(&self, output: &AgentOutput) -> PluginResult<()> {
        println!("[{}] Output: {:?}", self.level, output.as_text());
        Ok(())
    }
}
```

## WASM Plugin

Cross-language plugins with sandboxing.

**Location:** `examples/wasm_plugin/`

```rust
use mofa_sdk::plugins::WasmPlugin;

#[tokio::main]
async fn main() -> Result<()> {
    // Load WASM plugin
    let plugin = WasmPlugin::load("./plugins/my_plugin.wasm").await?;

    // Call exported function
    let result = plugin.call("process", b"input data").await?;
    println!("Result: {}", String::from_utf8_lossy(&result));

    Ok(())
}
```

### WASM Plugin (Rust Source)

```rust
// In plugins/my_plugin/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process(input: &[u8]) -> Vec<u8> {
    // Process input and return output
    input.to_vec()
}
```

## Tool Plugin Adapter

Wrap tools as plugins.

```rust
use mofa_sdk::plugins::ToolPluginAdapter;

let tool = CalculatorTool;
let plugin = ToolPluginAdapter::new(tool);

// Now the tool can be used as a plugin
manager.register(Box::new(plugin)).await?;
```

## Running Examples

```bash
# Rhai scripting
cargo run -p rhai_scripting

# Hot reload
cargo run -p rhai_hot_reload

# WASM plugin
cargo run -p wasm_plugin

# Plugin demo
cargo run -p plugin_demo
```

## See Also

- [Plugins Concept](../concepts/plugins.md) — Plugin architecture
- [API Reference: Plugins](../api-reference/plugins/README.md) — Plugin API
