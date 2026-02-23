# 功能标志

MoFA 使用功能标志来控制构建中包含哪些功能。

## 核心功能

| 功能 | 默认 | 描述 |
|---------|---------|-------------|
| `default` | ✓ | 基本智能体功能 |
| `openai` | ✓ | OpenAI 提供商支持 |
| `anthropic` | | Anthropic 提供商支持 |
| `uniffi` | | 跨语言绑定 |
| `python` | | 原生 Python 绑定 (PyO3) |

## 持久化功能

| 功能 | 描述 |
|---------|-------------|
| `persistence` | 启用持久化层 |
| `persistence-postgres` | PostgreSQL 后端 |
| `persistence-mysql` | MySQL 后端 |
| `persistence-sqlite` | SQLite 后端 |

## 运行时功能

| 功能 | 描述 |
|---------|-------------|
| `dora` | Dora-rs 分布式运行时 |
| `rhai` | Rhai 脚本引擎 |
| `wasm` | WASM 插件支持 |

## 使用功能标志

### 在 Cargo.toml 中

```toml
[dependencies]
# 默认功能
mofa-sdk = "0.1"

# 仅特定功能
mofa-sdk = { version = "0.1", default-features = false, features = ["openai"] }

# 多个功能
mofa-sdk = { version = "0.1", features = ["openai", "anthropic", "persistence-postgres"] }

# 所有功能
mofa-sdk = { version = "0.1", features = ["full"] }
```

### 功能组合

```toml
# 最小设置（无 LLM）
mofa-sdk = { version = "0.1", default-features = false }

# 带 OpenAI 和 SQLite 持久化
mofa-sdk = { version = "0.1", features = ["openai", "persistence-sqlite"] }

# 带 PostgreSQL 的生产设置
mofa-sdk = { version = "0.1", features = [
    "openai",
    "anthropic",
    "persistence-postgres",
    "rhai",
] }
```

## Crate 特定功能

### mofa-kernel

没有可选功能 - 始终是最小化核心。

### mofa-foundation

| 功能 | 描述 |
|---------|-------------|
| `openai` | OpenAI LLM 提供商 |
| `anthropic` | Anthropic LLM 提供商 |
| `persistence` | 持久化抽象 |

### mofa-runtime

| 功能 | 描述 |
|---------|-------------|
| `dora` | Dora-rs 集成 |
| `monitoring` | 内置监控 |

### mofa-ffi

| 功能 | 描述 |
|---------|-------------|
| `uniffi` | 通过 UniFFI 生成绑定 |
| `python` | 通过 PyO3 的原生 Python 绑定 |

## 构建大小影响

| 配置 | 二进制大小 | 编译时间 |
|---------------|-------------|--------------|
| 最小（无 LLM） | ~5 MB | 快 |
| 默认 | ~10 MB | 中等 |
| 完整功能 | ~20 MB | 慢 |

## 条件编译

```rust
#[cfg(feature = "openai")]
pub fn openai_from_env() -> Result<OpenAIProvider, LLMError> {
    // OpenAI 实现
}

#[cfg(feature = "persistence-postgres")]
pub async fn connect_postgres(url: &str) -> Result<PostgresStore, Error> {
    // PostgreSQL 实现
}

#[cfg(not(feature = "openai"))]
compile_error!("必须启用 OpenAI 功能才能使用 openai_from_env");
```

## 另见

- [配置](configuration.md) — 运行时配置
- [安装](../getting-started/installation.md) — 设置指南
