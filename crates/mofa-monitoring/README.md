# mofa-monitoring

MoFA Monitoring - Web-based dashboard and metrics collection

## Installation

```toml
[dependencies]
mofa-monitoring = "0.1"
```

## Features

- Web-based dashboard for monitoring agent execution
- Metrics collection and visualization
- Prometheus text exposition via `GET /metrics`
- Distributed tracing support with OpenTelemetry
- Real-time agent status monitoring
- Health checks and alerts
- HTTP server for dashboard UI
- Static file embedding for frontend assets

## Prometheus Export

`DashboardServer` now exposes a Prometheus-compatible endpoint at `/metrics`.
The exporter uses a background cache worker so scrape requests stay read-only
and low-overhead under high concurrency.

Cardinality is guarded with configurable caps and an overflow `__other__`
series to protect TSDB backends from unbounded label growth.

## Optional OTLP Metrics Export

Enable the `otlp-metrics` feature to use the native OpenTelemetry OTLP
metrics push exporter:

```toml
[dependencies]
mofa-monitoring = { version = "0.1", features = ["otlp-metrics"] }
```

## Documentation

- [API Documentation](https://docs.rs/mofa-monitoring)
- [Main Repository](https://github.com/mofa-org/mofa)

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
