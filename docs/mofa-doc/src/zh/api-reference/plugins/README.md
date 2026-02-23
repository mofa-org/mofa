# 插件 API 参考

用于扩展 MoFA 功能的插件系统。

## 模块

### rhai
用于运行时插件的 Rhai 脚本引擎。

- `RhaiPlugin` — 插件包装器
- `RhaiPluginManager` — 插件管理器
- `HotReloadWatcher` — 文件监视器

### wasm
WASM 插件支持。

- `WasmPlugin` — WASM 插件包装器
- `WasmPluginLoader` — 插件加载器

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

## 另见

- [插件概念](../../concepts/plugins.md) — 插件架构
- [插件示例](../../examples/插件.md) — 示例
