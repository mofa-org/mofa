Implements configurable retry and circuit breaker patterns for MoFA to improve system resilience and handle transient failures gracefully.

## Changes
- Added circuit breaker state machine (Closed → Open → Half-Open)
- Per-agent and global circuit breaker configurations
- Fallback strategies when circuit is open
- Metrics for circuit breaker state transitions
- Uses existing Exponential backoff with jitter from LLM module

Closes #354
