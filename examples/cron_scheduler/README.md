# Cron Scheduler Example

This example demonstrates the `CronScheduler` with persistence and optional telemetry support.

## Features

- **Cron Scheduling**: Schedule agents using cron expressions
- **Persistence**: Schedules survive process restarts via atomic file storage
- **Telemetry**: Prometheus metrics (enabled by default in this example)
- **Graceful Shutdown**: Proper cleanup on SIGINT/SIGTERM

## Running the Example

```bash
# From the project root directory
cd examples

# Run with telemetry enabled (default)
cargo run -p cron_scheduler

# Run with telemetry disabled
cargo run -p cron_scheduler --no-default-features

# Run with telemetry explicitly enabled
cargo run -p cron_scheduler --features scheduler-telemetry
```

## What It Does

1. Creates a simple logging agent
2. Schedules it to run every minute at second 0 (only on first run)
3. Persists the schedule to `schedules.json`
4. On subsequent runs, loads the persisted schedule automatically
5. Logs execution results
6. Records telemetry metrics (enabled by default)

## Persistence

Schedules are automatically saved to `schedules.json` in the current directory. On restart, the scheduler will reload and resume all previously registered schedules.

## Telemetry

When the `scheduler-telemetry` feature is enabled, the following Prometheus metrics are available:

- `mofa_scheduler_executions_total{schedule_id, agent_id, status}` - Total executions
- `mofa_scheduler_missed_ticks_total{schedule_id}` - Missed ticks due to concurrency limits
- `mofa_scheduler_active_runs{schedule_id}` - Currently running executions
- `mofa_scheduler_last_run_timestamp_ms{schedule_id}` - Last execution timestamp
- `mofa_scheduler_execution_duration_seconds{schedule_id, agent_id}` - Execution duration histogram

### Viewing Metrics

The example emits metrics to the `metrics` crate registry, but doesn't include a built-in HTTP server. To view the metrics:

**Option 1: Use Prometheus Exporter**
In production, integrate with a metrics collection system like Prometheus by setting up a `metrics-exporter-prometheus` recorder.

**Option 2: Debug Recorder (Development)**
For development, you can modify the example to use the debug recorder:

```rust
use metrics_util::debugging::DebuggingRecorder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up debug recorder
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::set_global_recorder(recorder).unwrap();

    // ... rest of main ...

    // Periodically print metrics
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            println!("Current metrics:");
            for (key, value, labels, metric_value) in snapshotter.snapshot().into_vec() {
                println!("  {}: {:?}", key.name(), metric_value);
            }
        }
    });

    // ... rest of main ...
}
```

This example enables telemetry by default. To disable it, use `--no-default-features`.

## Testing Persistence

1. **First run**: The example will create and register a new schedule, then persist it to `schedules.json`
2. **Subsequent runs**: The schedule will be automatically loaded from `schedules.json` - no manual re-registration needed
3. **Verification**: The schedule continues running across process restarts, demonstrating persistence works

To test with a fresh schedule, delete `schedules.json` before running.