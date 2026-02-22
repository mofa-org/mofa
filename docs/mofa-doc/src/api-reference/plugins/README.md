# Plugins API Reference

The plugin system for extending MoFA functionality.

## Modules

### rhai
Rhai scripting engine for runtime plugins.

- `RhaiPlugin` — Plugin wrapper
- `RhaiPluginManager` — Plugin manager
- `HotReloadWatcher` — File watcher

### wasm
WASM plugin support.

- `WasmPlugin` — WASM plugin wrapper
- `WasmPluginLoader` — Plugin loader

## Plugin Trait

```rust
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    async fn initialize(&mut self, ctx: &PluginContext) -> PluginResult<()>;
    async fn on_before_execute(&self, input: &AgentInput) -> PluginResult<()>;
    async fn on_after_execute(&self, output: &mut AgentOutput) -> PluginResult<()>;
    async fn shutdown(&mut self) -> PluginResult<()>;
}
```

## See Also

- [Plugins Concept](../../concepts/plugins.md) — Plugin architecture
- [Plugin Examples](../../examples/plugins.md) — Examples
