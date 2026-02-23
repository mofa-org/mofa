# 监控与可观测性

在生产环境中监控和观测 MoFA 应用。

## 概述

MoFA 提供：
- **指标** — 性能和使用指标
- **追踪** — 分布式请求追踪
- **日志** — 结构化日志

## 日志

通过 `RUST_LOG` 配置：

```bash
export RUST_LOG=mofa_sdk=debug,mofa_runtime=info
```

### 结构化日志

```rust
use tracing::{info, debug, error, instrument};

#[instrument(skip(input))]
async fn execute(&mut self, input: AgentInput) -> AgentResult<AgentOutput> {
    debug!(input_len = input.to_text().len(), "处理输入");

    let result = self.process(input).await?;

    info!(output_len = result.as_text().map(|s| s.len()), "执行完成");

    Ok(result)
}
```

## 指标

启用 `monitoring` 功能：

```toml
[dependencies]
mofa-sdk = { version = "0.1", features = ["monitoring"] }
```

### 内置指标

| 指标 | 类型 | 描述 |
|------|------|------|
| `mofa_agent_executions_total` | Counter | 总执行次数 |
| `mofa_agent_execution_duration` | Histogram | 执行延迟 |
| `mofa_agent_errors_total` | Counter | 错误计数 |
| `mofa_llm_tokens_total` | Counter | Token 使用量 |
| `mofa_llm_latency` | Histogram | LLM 响应时间 |

### Prometheus 端点

```rust
use mofa_sdk::monitoring::MetricsServer;

let server = MetricsServer::new(9090);
server.start().await?;
```

## 追踪

启用分布式追踪：

```rust
use mofa_sdk::monitoring::init_tracing;

init_tracing("my-service")?;

// 智能体操作会自动创建 span
```

## 健康检查

```rust
use mofa_sdk::monitoring::HealthCheck;

let health = HealthCheck::new()
    .with_database_check(|| async { store.health().await })
    .with_llm_check(|| async { llm.health().await });

// GET /health
let status = health.check().await;
```

## 仪表板

MoFA 包含监控仪表板：

```bash
cargo run -p monitoring_dashboard
```

访问地址 `http://localhost:3000`

## 相关链接

- [生产部署](../advanced/production.md) — 生产环境设置
- [配置](../appendix/configuration.md) — 监控配置
