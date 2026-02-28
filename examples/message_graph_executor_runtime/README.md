# MessageGraph Executor Runtime Example

Practical runtime verification for Task 19 (Phase 2).

This example demonstrates:
- async routing through `MessageGraphExecutor`,
- message dispatch to agent/stream targets through `AgentBus`,
- dead-letter routing for unmatched envelopes.

## Run

```bash
cargo run --manifest-path examples/Cargo.toml -p message_graph_executor_runtime
```

## Expected Output (shape)

```text
normal-routing: dispatched=..., dead_letters=0
fraud target received type='order.created' hop_count=...
stream target received stream_id='orders.fulfillment' sequence=...
unmatched-routing: dispatched=..., dead_letters=1
dlq received reason='no_route_match'
```
