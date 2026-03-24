# mofa-smith

mofa-smith is the production observability daemon for MoFA.

## What is mofa-smith?

mofa-monitoring already contains 6000+ lines of observability functionality, but nothing in the workspace actually started and orchestrated that stack end-to-end. mofa-smith is the daemon crate that wires those monitoring components together into a runnable service. Before this crate existed, there was no single way to run the MoFA monitoring stack as an operational daemon.

## Quick Start

Run:

```bash
cargo run -p mofa-smith
```

Expected output:

```text
INFO mofa-smith starting
INFO Dashboard → http://0.0.0.0:8080
INFO Metrics   → http://0.0.0.0:8080/metrics
```

Then open http://localhost:8080 in your browser.

## Configuration (smith.yaml)

Example:

```yaml
dashboard_port: 8080
collection_interval_ms: 1000
```

- `dashboard_port`: Port used by the monitoring dashboard HTTP server.
- `collection_interval_ms`: Metrics collection and live update interval in milliseconds.

## What it starts

- DashboardServer (web UI)
- MetricsCollector (CPU, memory, agent metrics)
- PrometheusExporter (/metrics endpoint)
- WebSocket handler (live dashboard updates)

## Architecture

SmithDaemon is the runtime entrypoint that loads `smith.yaml` (or defaults), builds `MetricsConfig` and `DashboardConfig`, and then starts the `mofa-monitoring` server stack so dashboard rendering, metrics collection, Prometheus export, and real-time WebSocket updates are all activated in one process.

## License

Apache 2.0 / MIT
