## Summary

Implements configurable retry and circuit breaker patterns for MoFA to improve system resilience and handle transient failures gracefully.

## Related Issues

Closes #354

## Context

MoFA currently has limited support for retry logic and lacks circuit breaker patterns for resilient agent execution.

## Changes

- Added circuit breaker state machine (Closed → Open → Half-Open)
- Per-agent and global circuit breaker configurations
- Fallback strategies when circuit is open
- Metrics for circuit breaker state transitions
- Uses existing Exponential backoff with jitter from LLM module
- Added new `circuit_breaker` module with config, state, metrics, and fallback

## How to Test

1. Run `cargo check -p mofa-foundation` - should compile
2. Run `cargo test -p mofa-foundation -- circuit_breaker` - tests should pass
3. Example usage:
```rust
use mofa_foundation::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};

let config = CircuitBreakerConfig::strict();
let cb = CircuitBreaker::new("agent-1", config);

if cb.can_execute().await {
    // Execute operation
}
```

## Breaking Changes

- No breaking changes

## Checklist

- [x] Code follows Rust idioms
- [x] `cargo fmt` run
- [x] Tests added
- [x] Public APIs documented
