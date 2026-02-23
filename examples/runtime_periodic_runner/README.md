# Runtime Periodic Runner Example

This example demonstrates a practical use case for `AgentRunner::run_periodic(...)`:
periodic health-probe execution with bounded runs.

## What it verifies

1. Periodic execution with `interval` + `max_runs`.
2. `run_immediately = true` starts work on the first tick.
3. `run_immediately = false` delays the first run by one interval.
4. Runner stats are updated across periodic executions.

## Run

From the `examples` workspace:

```bash
cargo run -p runtime_periodic_runner
```

Expected output includes:

- `Scenario 1: immediate execution (run_immediately=true)`
- `Scenario 2: delayed first execution (run_immediately=false)`
- `Runner stats: total=...`
