# mofa-plugins

The plugin system enabling extensibility through Rust/WASM and Rhai.

## Purpose

`mofa-plugins` provides:
- Compile-time plugin infrastructure (Rust/WASM)
- Runtime plugin engine (Rhai scripts)
- Hot-reload support
- Plugin adapters

## Plugin Types

| Type | Technology | Use Case |
|------|------------|----------|
| Compile-time | Rust / WASM | Performance critical |
| Runtime | Rhai scripts | Business logic, hot-reload |

## Usage

### Rhai Plugin

```rust
use mofa_plugins::{RhaiPlugin, RhaiPluginManager};

let mut manager = RhaiPluginManager::new();
let plugin = RhaiPlugin::from_file("./plugins/my_plugin.rhai").await?;
manager.register(plugin).await?;
```

### Rust Plugin

```rust
use mofa_kernel::plugin::AgentPlugin;

pub struct MyPlugin;

#[async_trait]
impl AgentPlugin for MyPlugin {
    fn name(&self) -> &str { "my_plugin" }
    // ...
}
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `rhai` | Rhai scripting engine |
| `wasm` | WASM plugin support |

## See Also

- [Plugins](../concepts/plugins.md) — Plugin concepts
- [Examples](../examples/plugins.md) — Plugin examples
