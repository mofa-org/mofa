# Metrics Export Pipeline

MoFA monitoring now supports two export paths:

- `GET /metrics`: Prometheus text exposition
- Optional OTLP push bridge (`otlp-metrics` feature)

## Prometheus Endpoint

The dashboard server exposes Prometheus data at:

```text
GET /metrics
Content-Type: text/plain; version=0.0.4; charset=utf-8
```

The exporter uses a background cache worker (default refresh: `1s`).
Scrapes read cached output, so request-time work is minimal even under high
concurrency.

## Cardinality Controls

Default hard limits:

- `agent_id`: 100
- `workflow_id`: 100
- `plugin_or_tool`: 100
- `provider+model`: 50

When a limit is exceeded, overflow series are aggregated into
`label="__other__"`.

Exporter self-metrics:

- `mofa_exporter_render_duration_seconds`
- `mofa_exporter_dropped_series_total{label=...}`
- `mofa_exporter_cache_age_seconds`
- `mofa_exporter_refresh_failures_total`

## OTLP Push Bridge

Enable feature:

```toml
mofa-monitoring = { version = "0.1", features = ["otlp-metrics"] }
```

Use `OtlpMetricsExporter` to periodically sample `MetricsCollector` snapshots
and push batched payloads to an OTLP endpoint.

Backpressure is enforced with a bounded queue (`max_queue_size`), and dropped
samples are counted.

## Local Verification

```bash
cargo check -p mofa-monitoring --offline
cargo check -p mofa-monitoring --features otlp-metrics --offline
cargo test -p mofa-monitoring dashboard::prometheus --offline
cargo test -p mofa-monitoring --features otlp-metrics tracing::metrics_exporter --offline
```
