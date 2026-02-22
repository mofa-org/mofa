# Monitoring & Observability

Monitor and observe MoFA applications in production.

## Overview

MoFA provides:
- **Metrics** — Performance and usage metrics
- **Tracing** — Distributed request tracing
- **Logging** — Structured logging

## Logging

Configure via `RUST_LOG`:

```bash
export RUST_LOG=mofa_sdk=debug,mofa_runtime=info
```

### Structured Logging

```rust
use tracing::{info, debug, error, instrument};

#[instrument(skip(input))]
async fn execute(&mut self, input: AgentInput) -> AgentResult<AgentOutput> {
    debug!(input_len = input.to_text().len(), "Processing input");

    let result = self.process(input).await?;

    info!(output_len = result.as_text().map(|s| s.len()), "Execution complete");

    Ok(result)
}
```

## Metrics

Enable the `monitoring` feature:

```toml
[dependencies]
mofa-sdk = { version = "0.1", features = ["monitoring"] }
```

### Built-in Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `mofa_agent_executions_total` | Counter | Total executions |
| `mofa_agent_execution_duration` | Histogram | Execution latency |
| `mofa_agent_errors_total` | Counter | Error count |
| `mofa_llm_tokens_total` | Counter | Token usage |
| `mofa_llm_latency` | Histogram | LLM response time |

### Prometheus Endpoint

```rust
use mofa_sdk::monitoring::MetricsServer;

let server = MetricsServer::new(9090);
server.start().await?;
```

## Tracing

Enable distributed tracing:

```rust
use mofa_sdk::monitoring::init_tracing;

init_tracing("my-service")?;

// Spans are automatically created for agent operations
```

## Health Checks

```rust
use mofa_sdk::monitoring::HealthCheck;

let health = HealthCheck::new()
    .with_database_check(|| async { store.health().await })
    .with_llm_check(|| async { llm.health().await });

// GET /health
let status = health.check().await;
```

## Dashboard

MoFA includes a monitoring dashboard:

```bash
cargo run -p monitoring_dashboard
```

Access at `http://localhost:3000`

## See Also

- [Production Deployment](../advanced/production.md) — Production setup
- [Configuration](../appendix/configuration.md) — Monitoring config
