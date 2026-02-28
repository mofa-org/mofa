# Metrics Export Pipeline

MoFA monitoring now supports two export paths:

- `GET /metrics`: Prometheus text exposition
- Optional native OTLP metrics push (`otlp-metrics` feature)

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

When a limit is exceeded, overflow series preserve original label keys and use
`"__other__"` as the label value (for example,
`agent_id="__other__"` or `provider="__other__",model="__other__"`).

Exporter self-metrics:

- `mofa_exporter_render_duration_seconds`
- `mofa_exporter_dropped_series_total{label=...}`
- `mofa_exporter_cache_age_seconds`
- `mofa_exporter_refresh_failures_total`

## OTLP Push Export

Enable feature:

```toml
mofa-monitoring = { version = "0.1", features = ["otlp-metrics"] }
```

Use `OtlpMetricsExporter` to periodically sample `MetricsCollector` snapshots
and record them via native OpenTelemetry metric instruments exported through
the OTLP SDK pipeline.

Backpressure is enforced with a bounded queue (`max_queue_size`), and dropped
samples are counted.

Exporter config guards clamp invalid values to safe defaults:

- Empty `endpoint` -> default OTLP endpoint
- `batch_size == 0` -> `1`
- `max_queue_size == 0` -> `1`
- Zero durations (`collect_interval`, `export_interval`, `timeout`) -> `1s`

## Local Verification

```bash
cargo check -p mofa-monitoring --offline
cargo check -p mofa-monitoring --features otlp-metrics --offline
cargo test -p mofa-monitoring dashboard::prometheus --offline
cargo test -p mofa-monitoring --features otlp-metrics tracing::metrics_exporter --offline
cargo clippy -p mofa-monitoring --features otlp-metrics --lib -- -D warnings
```
