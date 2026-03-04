# Feature Flags

MoFA uses feature flags to control which functionality is included in your build.

## Core Features

| Feature | Default | Description |
|---------|---------|-------------|
| `default` | ✓ | Basic agent functionality |
| `openai` | ✓ | OpenAI provider support |
| `anthropic` | | Anthropic provider support |
| `uniffi` | | Cross-language bindings |
| `python` | | Native Python bindings (PyO3) |

## Persistence Features

| Feature | Description |
|---------|-------------|
| `persistence` | Enable persistence layer |
| `persistence-postgres` | PostgreSQL backend |
| `persistence-mysql` | MySQL backend |
| `persistence-sqlite` | SQLite backend |

## Runtime Features

| Feature | Description |
|---------|-------------|
| `dora` | Dora-rs distributed runtime |
| `rhai` | Rhai scripting engine |
| `wasm` | WASM plugin support |

## Using Feature Flags

### In Cargo.toml

```toml
[dependencies]
# Default features
mofa-sdk = "0.1"

# Specific features only
mofa-sdk = { version = "0.1", default-features = false, features = ["openai"] }

# Multiple features
mofa-sdk = { version = "0.1", features = ["openai", "anthropic", "persistence-postgres"] }

# All features
mofa-sdk = { version = "0.1", features = ["full"] }
```

### Feature Combinations

```toml
# Minimal setup (no LLM)
mofa-sdk = { version = "0.1", default-features = false }

# With OpenAI and SQLite persistence
mofa-sdk = { version = "0.1", features = ["openai", "persistence-sqlite"] }

# Production setup with PostgreSQL
mofa-sdk = { version = "0.1", features = [
    "openai",
    "anthropic",
    "persistence-postgres",
    "rhai",
] }
```

## Crate-Specific Features

### mofa-kernel

No optional features - always minimal core.

### mofa-foundation

| Feature | Description |
|---------|-------------|
| `openai` | OpenAI LLM provider |
| `anthropic` | Anthropic LLM provider |
| `persistence` | Persistence abstractions |

### mofa-runtime

| Feature | Description |
|---------|-------------|
| `dora` | Dora-rs integration |
| `monitoring` | Built-in monitoring |

### mofa-ffi

| Feature | Description |
|---------|-------------|
| `uniffi` | Generate bindings via UniFFI |
| `python` | Native Python bindings via PyO3 |

## Build Size Impact

| Configuration | Binary Size | Compile Time |
|---------------|-------------|--------------|
| Minimal (no LLM) | ~5 MB | Fast |
| Default | ~10 MB | Medium |
| Full features | ~20 MB | Slow |

## Conditional Compilation

```rust
#[cfg(feature = "openai")]
pub fn openai_from_env() -> Result<OpenAIProvider, LLMError> {
    // OpenAI implementation
}

#[cfg(feature = "persistence-postgres")]
pub async fn connect_postgres(url: &str) -> Result<PostgresStore, Error> {
    // PostgreSQL implementation
}

#[cfg(not(feature = "openai"))]
compile_error!("OpenAI feature must be enabled to use openai_from_env");
```

## See Also

- [Configuration](configuration.md) — Runtime configuration
- [Installation](../getting-started/installation.md) — Setup guide
