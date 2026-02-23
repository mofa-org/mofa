# mofa-extra

MoFA 的额外工具和扩展。

## 目的

`mofa-extra` 提供:
- 工具函数
- 扩展 trait
- 辅助类型
- 实验性功能

## 内容

| 模块 | 描述 |
|--------|-------------|
| `utils` | 通用工具 |
| `extensions` | 扩展 trait |
| `experimental` | 实验性功能 |

## 用法

```rust
use mofa_extra::utils::json::parse_safe;
use mofa_extra::extensions::AgentExt;

let parsed = parse_safe(&json_string);
agent.execute_with_retry(input, 3).await?;
```

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `all` | 启用所有工具 |

## 另见

- [API 参考](../api-reference/kernel/README.md) — 核心 API
