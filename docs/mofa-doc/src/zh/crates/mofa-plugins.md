# mofa-plugins

通过 Rust/WASM 和 Rhai 实现可扩展性的插件系统。

## 目的

`mofa-plugins` 提供:
- 编译时插件基础设施（Rust/WASM）
- 运行时插件引擎（Rhai 脚本）
- 热重载支持
- 插件适配器

## 插件类型

| 类型 | 技术 | 用例 |
|------|------------|----------|
| 编译时 | Rust / WASM | 性能关键 |
| 运行时 | Rhai 脚本 | 业务逻辑，热重载 |

## 用法

### Rhai 插件

```rust
use mofa_plugins::{RhaiPlugin, RhaiPluginManager};

let mut manager = RhaiPluginManager::new();
let plugin = RhaiPlugin::from_file("./plugins/my_plugin.rhai").await?;
manager.register(plugin).await?;
```

### Rust 插件

```rust
use mofa_kernel::plugin::AgentPlugin;

pub struct MyPlugin;

#[async_trait]
impl AgentPlugin for MyPlugin {
    fn name(&self) -> &str { "my_plugin" }
    // ...
}
```

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `rhai` | Rhai 脚本引擎 |
| `wasm` | WASM 插件支持 |

## 另见

- [插件](../concepts/plugins.md) — 插件概念
- [示例](../examples/插件.md) — 插件示例
