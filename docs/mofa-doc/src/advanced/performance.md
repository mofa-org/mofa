# Performance Tuning

Optimize MoFA applications for maximum performance.

## Build Optimization

### Release Profile

```toml
# Cargo.toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### Feature Flags

Only enable features you need:

```toml
[dependencies]
# Minimal: smaller binary, faster compile
mofa-sdk = { version = "0.1", default-features = false, features = ["openai"] }

# Avoid unused features
# mofa-sdk = { version = "0.1", features = ["full"] }  # Don't do this
```

## Concurrency

### Agent Concurrency

```rust
// Limit concurrent executions
let capabilities = AgentCapabilities::builder()
    .max_concurrency(100)
    .build();
```

### Database Connections

```rust
// Tune connection pool
let pool = sqlx::postgres::PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .connect(&database_url)
    .await?;
```

### Tokio Runtime

```rust
// Configure runtime
#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() {
    // ...
}
```

## Memory Management

### Session Caching

```rust
// Limit session cache size
let config = PersistenceConfig {
    session_cache_size: 1000,
    session_ttl: Duration::from_secs(3600),
};
```

### Context Window

```rust
// Use sliding window for long conversations
let agent = LLMAgentBuilder::from_env()?
    .with_sliding_window(20)  // Keep last 20 messages
    .build_async()
    .await;
```

## LLM Optimization

### Batching

```rust
// Batch multiple requests
let results = run_agents(agent, inputs).await?;
```

### Caching

```rust
// Enable response caching
let client = LLMClient::builder()
    .with_cache(CacheConfig {
        enabled: true,
        ttl: Duration::from_secs(300),
        max_entries: 1000,
    })
    .build();
```

### Streaming

```rust
// Use streaming for better UX
let stream = client.stream()
    .system("You are helpful.")
    .user("Tell a story")
    .start()
    .await?;

while let Some(chunk) = stream.next().await {
    print!("{}", chunk?);
}
```

## Profiling

### CPU Profiling

```bash
# Using perf
cargo build --release
perf record -g ./target/release/my-agent
perf report
```

### Memory Profiling

```bash
# Using valgrind
valgrind --tool=massif ./target/release/my-agent
```

### Flamegraphs

```bash
cargo install flamegraph
cargo flamegraph --root
```

## Benchmarks

```bash
# Run built-in benchmarks
cargo bench

# Benchmark specific operations
cargo bench -- agent_execution
```

## See Also

- [Production Deployment](production.md) — Deployment guide
- [Configuration](../appendix/configuration.md) — Runtime configuration
