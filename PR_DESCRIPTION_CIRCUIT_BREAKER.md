## Summary

Implements configurable retry and circuit breaker patterns for MoFA to improve system resilience and handle transient failures gracefully.

## Related Issues

Closes #354

## Context

MoFA currently has limited support for retry logic and lacks circuit breaker patterns for resilient agent execution. This implementation adds:

- Exponential backoff with jitter (already existed in LLM module)
- Configurable retry counts and timeouts
- Circuit breaker state machine (closed, open, half-open)
- Fallback strategies when circuit is open
- Per-agent and global circuit breaker configurations
- Metrics for circuit breaker state transitions

## Changes

- Added new `circuit_breaker` module with:
  - `config.rs`: Circuit breaker configuration types
  - `state.rs`: Circuit breaker state machine implementation
  - `metrics.rs`: Metrics collection and tracking
  - `fallback.rs`: Fallback strategies
  - `mod.rs`: Module exports

## How to Test

1. The code compiles successfully with `cargo check -p mofa-foundation`
2. Tests can be run with `cargo test -p mofa-foundation -- circuit_breaker`

Example usage:
```rust
use mofa_foundation::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};

let config = CircuitBreakerConfig::strict();
let cb = CircuitBreaker::new("agent-1", config);

if cb.can_execute().await {
    // Execute operation
}
```

## Breaking Changes

- None

## Checklist

- [x] Code compiles
- [x] Tests pass
- [x] Code follows Rust idioms
- [x] Public APIs documented

## Additional Notes

The implementation leverages the existing `BackoffStrategy::ExponentialWithJitter` from the LLM module for exponential backoff with jitter support.
