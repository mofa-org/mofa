# 配置参考

MoFA 配置选项完整参考。

## 环境变量

### LLM 配置

| 变量 | 默认值 | 描述 |
|----------|---------|-------------|
| `OPENAI_API_KEY` | - | OpenAI API 密钥 |
| `OPENAI_MODEL` | `gpt-4o` | 使用的模型 |
| `OPENAI_BASE_URL` | - | 自定义端点 |
| `ANTHROPIC_API_KEY` | - | Anthropic API 密钥 |
| `ANTHROPIC_MODEL` | `claude-sonnet-4-5-latest` | 使用的模型 |

### 持久化配置

| 变量 | 默认值 | 描述 |
|----------|---------|-------------|
| `DATABASE_URL` | - | 数据库连接字符串 |
| `MOFA_SESSION_TTL` | `3600` | 会话超时（秒） |
| `MOFA_MAX_CONNECTIONS` | `10` | 最大数据库连接数 |

### 运行时配置

| 变量 | 默认值 | 描述 |
|----------|---------|-------------|
| `RUST_LOG` | `info` | 日志级别 |
| `MOFA_MAX_AGENTS` | `100` | 最大并发智能体数 |
| `MOFA_TIMEOUT` | `30` | 默认超时（秒） |

## 配置文件

在项目根目录创建 `mofa.toml`:

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

## 加载配置

```rust
use mofa_sdk::config::Config;

// 从环境和配置文件加载
let config = Config::load()?;

// 访问值
let timeout = config.agent.default_timeout;
let model = config.llm.model;

// 用于智能体
let agent = LLMAgentBuilder::from_config(&config)?
    .build_async()
    .await;
```

## 编程式配置

### 智能体配置

```rust
use mofa_sdk::runtime::{AgentConfig, AgentConfigBuilder};

let config = AgentConfigBuilder::new()
    .timeout(Duration::from_secs(60))
    .max_retries(5)
    .rate_limit(100)  // 每分钟请求数
    .build();
```

### LLM 配置

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

### 持久化配置

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

## 日志配置

通过 `RUST_LOG` 配置日志:

```bash
# 设置日志级别
export RUST_LOG=debug

# 按模块设置日志
export RUST_LOG=mofa_sdk=debug,mofa_runtime=info

# JSON 格式（生产环境）
export RUST_LOG_FORMAT=json
```

## 另见

- [功能标志](feature-flags.md) — 功能配置
- [生产部署](../advanced/production.md) — 生产设置
