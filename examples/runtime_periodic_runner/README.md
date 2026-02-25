# Runtime Periodic Runner Example

This example demonstrates practical periodic scheduling with `AgentRunner`:

1. Interval scheduling (`run_periodic`) with bounded runs.
2. Cron scheduling (`run_periodic_cron`) with real cron expressions.
3. Policy controls:
   - `PeriodicMissedTickPolicy` for interval tick behavior.
   - `CronMisfirePolicy` for cron misfire behavior.

## Run

From the `examples` workspace:

```bash
cargo run -p runtime_periodic_runner
```

You should see:

- interval runs with output payloads
- cron runs with output payloads
- stats summary at the end
