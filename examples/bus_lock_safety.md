# Bus Lock Safety Example

This example demonstrates lock safety in the MoFA runtime message bus, ensuring no lock is held across an await point.

## How it works
- Uses `tokio::RwLock` to simulate concurrent read/write access
- Writer acquires lock, increments value, releases lock, then awaits message send
- Reader acquires lock, reads value, releases lock, then awaits message receive
- Ensures lock is dropped before any await, preventing deadlocks

## Run the example
```
cargo run --example bus_lock_safety
```

## Output
You should see:
- Writer acquires lock, increments value
- Writer releases lock, sends message
- Reader acquires lock, reads value
- Reader releases lock, receives message
- No deadlocks or lock contention

## Source
See [bus_lock_safety.rs](bus_lock_safety.rs)
