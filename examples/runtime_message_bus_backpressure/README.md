# Runtime Message Bus Backpressure Example

This example demonstrates that `SimpleMessageBus` remains responsive under backpressure after the lock-scope fix.

## What it verifies

1. `register_agent` completes quickly while `send_to` is waiting on a full channel.
2. `subscribe_topic` completes quickly while `publish` is waiting on a full channel.

These are practical user-facing checks for the non-Dora runtime path.

## Run

From the `examples` workspace:

```bash
cargo run -p runtime_message_bus_backpressure
```

Expected output includes:

- `register_agent completed quickly while send_to was pending`
- `subscribe_topic completed quickly while publish was pending`
