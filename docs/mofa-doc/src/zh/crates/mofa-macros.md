# mofa-macros

MoFA 的过程宏。

## 目的

`mofa-macros` 提供:
- 常见 trait 的派生宏
- 用于配置的属性宏
- 代码生成辅助工具

## 可用宏

### #[agent]

自动实现常见智能体功能:

```rust
use mofa_macros::agent;

#[agent(tags = ["llm", "qa"])]
struct MyAgent {
    llm: LLMClient,
}

// 自动实现:
// - id(), name(), capabilities()
// - 默认状态管理
```

### #[tool]

用更少的样板代码定义工具:

```rust
use mofa_macros::tool;

#[tool(name = "calculator", description = "执行算术运算")]
fn calculate(operation: String, a: f64, b: f64) -> f64 {
    match operation.as_str() {
        "add" => a + b,
        "subtract" => a - b,
        _ => panic!("未知操作"),
    }
}
```

## 用法

添加到 `Cargo.toml`:

```toml
[dependencies]
mofa-macros = "0.1"
```

## 另见

- [工具开发](../guides/tool-development.md) — 工具指南
- [智能体](../concepts/agents.md) — 智能体概念
