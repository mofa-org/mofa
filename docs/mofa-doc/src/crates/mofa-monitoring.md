# mofa-monitoring

Monitoring and observability for MoFA applications.

## Purpose

`mofa-monitoring` provides:
- Metrics collection (Prometheus compatible)
- Distributed tracing (OpenTelemetry)
- Web dashboard
- Health check endpoints

## Feature Flags

| Flag | Description |
|------|-------------|
| `prometheus` | Prometheus metrics |
| `opentelemetry` | OpenTelemetry tracing |
| `dashboard` | Web dashboard |

## Usage

```rust
use mofa_monitoring::{MetricsServer, init_tracing};

// Initialize tracing
init_tracing("my-service")?;

// Start metrics server
let server = MetricsServer::new(9090);
server.start().await?;
```

## Dashboard

```bash
# Start monitoring dashboard
cargo run -p mofa-monitoring -- dashboard
```

Access at `http://localhost:3000`

## See Also

- [Monitoring Guide](../guides/monitoring.md) — Monitoring guide
- [Production Deployment](../advanced/production.md) — Production setup
