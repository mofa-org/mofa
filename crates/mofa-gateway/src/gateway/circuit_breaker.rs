//! Circuit breaker implementation.
//!
//! This module provides circuit breakers to prevent cascading failures by
//! automatically stopping requests to failing nodes.
//!
//! # Implementation Status
//!
//! **Complete** - Circuit breaker implementation with automatic failure detection and recovery

use crate::error::GatewayResult;
use crate::types::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CircuitState {
    /// Circuit is closed - requests flow normally.
    Closed,
    /// Circuit is open - requests are rejected immediately.
    Open,
    /// Circuit is half-open - testing if service recovered.
    HalfOpen,
}

/// Circuit breaker for a node.
pub struct CircuitBreaker {
    node_id: NodeId,
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    last_failure: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(
        node_id: NodeId,
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    ) -> Self {
        Self {
            node_id,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            failure_threshold,
            success_threshold,
            timeout,
            last_failure: Arc::new(RwLock::new(None)),
        }
    }

    /// Try to allow a request through the circuit breaker.
    pub async fn try_acquire(&self) -> GatewayResult<bool> {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => Ok(true),
            CircuitState::Open => {
                // Check if timeout has passed (transition to half-open)
                let last_failure = *self.last_failure.read().await;
                if let Some(last) = last_failure
                    && last.elapsed() >= self.timeout
                {
                    let mut state = self.state.write().await;
                    *state = CircuitState::HalfOpen;
                    let mut failures = self.failure_count.write().await;
                    *failures = 0;
                    let mut successes = self.success_count.write().await;
                    *successes = 0;
                    return Ok(true);
                }
                Ok(false)
            }
            CircuitState::HalfOpen => Ok(true),
        }
    }

    /// Record a successful request.
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;
        let mut failures = self.failure_count.write().await;
        let mut successes = self.success_count.write().await;

        match *state {
            CircuitState::Closed => {
                // Reset failure count on success
                *failures = 0;
                *successes = 0;
            }
            CircuitState::HalfOpen => {
                // Track consecutive successes in half-open state and only close
                // the circuit once we reach the configured success threshold.
                *successes += 1;
                if *successes >= self.success_threshold {
                    *state = CircuitState::Closed;
                    *failures = 0;
                    *successes = 0;
                }
            }
            CircuitState::Open => {
                // Should not happen, but handle gracefully
            }
        }
    }

    /// Record a failed request.
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;
        let mut failures = self.failure_count.write().await;
        let mut last_failure = self.last_failure.write().await;

        *failures += 1;
        *last_failure = Some(Instant::now());

        match *state {
            CircuitState::Closed => {
                if *failures >= self.failure_threshold {
                    *state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open opens the circuit
                *state = CircuitState::Open;
            }
            CircuitState::Open => {
                // Already open, just update failure time
            }
        }
    }

    /// Get current circuit state.
    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }
}

/// Registry of circuit breakers for all nodes.
pub struct CircuitBreakerRegistry {
    breakers: Arc<RwLock<std::collections::HashMap<NodeId, Arc<CircuitBreaker>>>>,
    default_failure_threshold: u32,
    default_success_threshold: u32,
    default_timeout: Duration,
}

impl CircuitBreakerRegistry {
    /// Create a new circuit breaker registry.
    pub fn new(
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    ) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            default_failure_threshold: failure_threshold,
            default_success_threshold: success_threshold,
            default_timeout: timeout,
        }
    }

    /// Get or create a circuit breaker for a node.
    pub async fn get_or_create(&self, node_id: &NodeId) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.write().await;
        breakers
            .entry(node_id.clone())
            .or_insert_with(|| {
                Arc::new(CircuitBreaker::new(
                    node_id.clone(),
                    self.default_failure_threshold,
                    self.default_success_threshold,
                    self.default_timeout,
                ))
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let cb = CircuitBreaker::new(NodeId::new("node-1"), 3, 2, Duration::from_secs(5));

        // Initially closed
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.try_acquire().await.unwrap());

        // Record failures
        for _ in 0..3 {
            cb.record_failure().await;
        }

        // Should now be open
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.try_acquire().await.unwrap());
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let cb = CircuitBreaker::new(NodeId::new("node-1"), 3, 2, Duration::from_millis(100));

        // Open the circuit
        for _ in 0..3 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert!(cb.try_acquire().await.unwrap());
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Two consecutive successes should close it (success_threshold = 2)
        cb.record_success().await;
        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
