# Configuration Reference

Complete reference for MoFA configuration options.

## Environment Variables

### LLM Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAI_API_KEY` | - | OpenAI API key |
| `OPENAI_MODEL` | `gpt-4o` | Model to use |
| `OPENAI_BASE_URL` | - | Custom endpoint |
| `ANTHROPIC_API_KEY` | - | Anthropic API key |
| `ANTHROPIC_MODEL` | `claude-sonnet-4-5-latest` | Model to use |

### Persistence Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | - | Database connection string |
| `MOFA_SESSION_TTL` | `3600` | Session timeout (seconds) |
| `MOFA_MAX_CONNECTIONS` | `10` | Max DB connections |

### Runtime Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Logging level |
| `MOFA_MAX_AGENTS` | `100` | Max concurrent agents |
| `MOFA_TIMEOUT` | `30` | Default timeout (seconds) |

## Configuration File

Create `mofa.toml` in your project root:

```toml
[agent]
default_timeout = 30
max_retries = 3
concurrency_limit = 10

[llm]
provider = "openai"
model = "gpt-4o"
temperature = 0.7
max_tokens = 4096

[llm.openai]
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"

[persistence]
enabled = true
backend = "postgres"
session_ttl = 3600

[persistence.postgres]
url_env = "DATABASE_URL"
max_connections = 10
min_connections = 2

[plugins]
hot_reload = true
watch_dirs = ["./plugins"]

[monitoring]
enabled = true
metrics_port = 9090
tracing = true
```

## Loading Configuration

```rust
use mofa_sdk::config::Config;

// Load from environment and config file
let config = Config::load()?;

// Access values
let timeout = config.agent.default_timeout;
let model = config.llm.model;

// Use with agent
let agent = LLMAgentBuilder::from_config(&config)?
    .build_async()
    .await;
```

## Programmatic Configuration

### Agent Configuration

```rust
use mofa_sdk::runtime::{AgentConfig, AgentConfigBuilder};

let config = AgentConfigBuilder::new()
    .timeout(Duration::from_secs(60))
    .max_retries(5)
    .rate_limit(100)  // requests per minute
    .build();
```

### LLM Configuration

```rust
use mofa_sdk::llm::{LLMConfig, LLMConfigBuilder};

let config = LLMConfigBuilder::new()
    .model("gpt-4o")
    .temperature(0.7)
    .max_tokens(4096)
    .top_p(1.0)
    .frequency_penalty(0.0)
    .presence_penalty(0.0)
    .build();

let client = LLMClient::with_config(provider, config);
```

### Persistence Configuration

```rust
use mofa_sdk::persistence::{PersistenceConfig, Backend};

let config = PersistenceConfig {
    enabled: true,
    backend: Backend::Postgres {
        url: std::env::var("DATABASE_URL")?,
        max_connections: 10,
        min_connections: 2,
    },
    session_ttl: Duration::from_secs(3600),
};
```

## Logging Configuration

Configure logging via `RUST_LOG`:

```bash
# Set logging level
export RUST_LOG=debug

# Per-module logging
export RUST_LOG=mofa_sdk=debug,mofa_runtime=info

# JSON format for production
export RUST_LOG_FORMAT=json
```

## See Also

- [Feature Flags](feature-flags.md) — Feature configuration
- [Production Deployment](../advanced/production.md) — Production setup
