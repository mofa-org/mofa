//! Error Recovery Strategies
//!
//! Provides concrete retry and recovery infrastructure that works with
//! `GlobalError` and `GlobalResult` from the kernel layer.
//!
//! This module contains all concrete implementations:
//! - `Backoff` strategies (None, Fixed, Linear, Exponential)
//! - `RetryPolicy` with builder pattern
//! - `retry()` async function
//! - `fallback_chain()` for cascading fallback operations
//! - `CircuitBreaker` with Closed → Open → HalfOpen state machine
//!
//! The `ErrorRecovery` trait remains in `mofa_kernel::agent::types::recovery`.

use mofa_kernel::agent::types::error::{ErrorCategory, ErrorSeverity, GlobalError, GlobalResult};
use std::future::Future;
use std::time::Duration;

// ============================================================================
// Backoff - Generic backoff strategy
// ============================================================================

/// Generic backoff strategy for retry delays.
///
/// This is the kernel-level backoff strategy. LLM-specific strategies
/// (e.g. `BackoffStrategy` in mofa-foundation) can interoperate by
/// converting to/from this type.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Backoff {
    /// No delay between retries
    None,
    /// Fixed delay between retries
    Fixed {
        /// Delay in milliseconds
        delay_ms: u64,
    },
    /// Linear backoff: delay = initial + (attempt * increment)
    Linear {
        /// Initial delay in milliseconds
        initial_ms: u64,
        /// Increment per attempt in milliseconds
        increment_ms: u64,
    },
    /// Exponential backoff: delay = min(initial * 2^attempt, max)
    Exponential {
        /// Initial delay in milliseconds
        initial_ms: u64,
        /// Maximum delay in milliseconds
        max_ms: u64,
    },
}

impl Default for Backoff {
    fn default() -> Self {
        Self::Exponential {
            initial_ms: 100,
            max_ms: 10_000,
        }
    }
}

impl Backoff {
    /// Create a fixed backoff with the given delay
    pub fn fixed(delay_ms: u64) -> Self {
        Self::Fixed { delay_ms }
    }

    /// Create an exponential backoff
    pub fn exponential(initial_ms: u64, max_ms: u64) -> Self {
        Self::Exponential { initial_ms, max_ms }
    }

    /// Create a linear backoff
    pub fn linear(initial_ms: u64, increment_ms: u64) -> Self {
        Self::Linear {
            initial_ms,
            increment_ms,
        }
    }

    /// Calculate delay for the given attempt (0-indexed)
    pub fn delay_for(&self, attempt: u32) -> Duration {
        match self {
            Self::None => Duration::ZERO,
            Self::Fixed { delay_ms } => Duration::from_millis(*delay_ms),
            Self::Linear {
                initial_ms,
                increment_ms,
            } => {
                let ms = initial_ms + (increment_ms * attempt as u64);
                Duration::from_millis(ms)
            }
            Self::Exponential { initial_ms, max_ms } => {
                let ms = initial_ms.saturating_mul(2u64.saturating_pow(attempt.min(20)));
                Duration::from_millis(ms.min(*max_ms))
            }
        }
    }
}

// ============================================================================
// RetryPolicy - Generic retry policy
// ============================================================================

/// Controls how and when operations are retried.
///
/// Unlike LLM-specific `LLMRetryPolicy`, this works with any `GlobalError`.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first)
    pub max_attempts: u32,
    /// Backoff strategy
    pub backoff: Backoff,
    /// Predicate: should we retry this error? (defaults to `is_retryable()`)
    retry_predicate: RetryPredicate,
}

/// What determines whether an error is retryable
#[derive(Debug, Clone)]
enum RetryPredicate {
    /// Use `GlobalError::is_retryable()` (ErrorSeverity-based)
    Default,
    /// Retry on specific error categories
    Categories(Vec<ErrorCategory>),
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff: Backoff::default(),
            retry_predicate: RetryPredicate::Default,
        }
    }
}

impl RetryPolicy {
    /// Create a builder for `RetryPolicy`
    pub fn builder() -> RetryPolicyBuilder {
        RetryPolicyBuilder::default()
    }

    /// Create a policy that never retries
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            backoff: Backoff::None,
            retry_predicate: RetryPredicate::Default,
        }
    }

    /// Create a policy with a given number of attempts and default backoff
    pub fn with_attempts(max_attempts: u32) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            ..Default::default()
        }
    }

    /// Whether the given error should be retried under this policy
    pub fn should_retry(&self, error: &GlobalError) -> bool {
        match &self.retry_predicate {
            RetryPredicate::Default => error.is_retryable(),
            RetryPredicate::Categories(cats) => cats.contains(&error.category()),
        }
    }
}

/// Builder for `RetryPolicy`
#[derive(Debug, Default)]
pub struct RetryPolicyBuilder {
    max_attempts: Option<u32>,
    backoff: Option<Backoff>,
    categories: Option<Vec<ErrorCategory>>,
}

impl RetryPolicyBuilder {
    /// Set maximum attempts (including the first)
    pub fn max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = Some(n.max(1));
        self
    }

    /// Set the backoff strategy
    pub fn backoff(mut self, backoff: Backoff) -> Self {
        self.backoff = Some(backoff);
        self
    }

    /// Only retry on specific error categories
    pub fn retry_on(mut self, categories: Vec<ErrorCategory>) -> Self {
        self.categories = Some(categories);
        self
    }

    /// Build the retry policy
    pub fn build(self) -> RetryPolicy {
        RetryPolicy {
            max_attempts: self.max_attempts.unwrap_or(3),
            backoff: self.backoff.unwrap_or_default(),
            retry_predicate: match self.categories {
                Some(cats) => RetryPredicate::Categories(cats),
                None => RetryPredicate::Default,
            },
        }
    }
}

// ============================================================================
// retry() - Generic async retry function
// ============================================================================

/// Execute an async operation with retry logic.
///
/// Retries the operation according to the given `RetryPolicy` when it
/// returns a retryable `GlobalError`.
///
/// # Example
///
/// ```rust,ignore
/// let result = retry(RetryPolicy::with_attempts(3), || async {
///     some_fallible_operation().await
/// }).await;
/// ```
pub async fn retry<F, Fut, T>(policy: RetryPolicy, mut operation: F) -> GlobalResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = GlobalResult<T>>,
{
    let mut last_error = None;

    for attempt in 0..policy.max_attempts {
        // Apply backoff delay on retries
        if attempt > 0 {
            let delay = policy.backoff.delay_for(attempt - 1);
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
        }

        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                let is_last = attempt + 1 >= policy.max_attempts;
                if is_last || !policy.should_retry(&err) {
                    return Err(err);
                }
                last_error = Some(err);
            }
        }
    }

    // Shouldn't reach here, but just in case
    Err(last_error.unwrap_or_else(|| GlobalError::Other("retry loop exhausted".to_string())))
}

// ============================================================================
// FallbackChain - Execute a chain of fallback operations
// ============================================================================

/// Execute a series of fallback operations, returning the first success.
///
/// Each operation in the chain is tried in order. If one fails, the next
/// is attempted. If all fail, the last error is returned.
///
/// # Example
///
/// ```rust,ignore
/// let result = fallback_chain(vec![
///     Box::new(|| Box::pin(primary_operation())),
///     Box::new(|| Box::pin(secondary_operation())),
///     Box::new(|| Box::pin(cached_fallback())),
/// ]).await;
/// ```
pub async fn fallback_chain<T>(operations: Vec<FallbackOperation<T>>) -> GlobalResult<T> {
    let mut last_error = GlobalError::Other("no fallback operations provided".to_string());

    for operation in operations {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_error = err;
            }
        }
    }

    Err(last_error)
}

type BoxedFallbackFuture<T> = std::pin::Pin<Box<dyn Future<Output = GlobalResult<T>> + Send>>;
type FallbackOperation<T> = Box<dyn FnOnce() -> BoxedFallbackFuture<T> + Send>;

// ============================================================================
// CircuitBreaker - Prevent cascading failures
// ============================================================================

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CircuitState {
    /// Normal operation — requests are allowed
    Closed,
    /// Too many failures — requests are rejected immediately
    Open,
    /// Testing if the service has recovered — limited requests allowed
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "closed"),
            Self::Open => write!(f, "open"),
            Self::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Configuration for a circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit
    pub failure_threshold: u32,
    /// Duration to wait in the open state before transitioning to half-open
    pub recovery_timeout: Duration,
    /// Number of successful requests needed in half-open state to close
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

/// Circuit breaker for preventing cascading failures.
///
/// Tracks consecutive failures and prevents calls when the failure
/// threshold is exceeded. Automatically tests recovery after a timeout.
///
/// # Example
///
/// ```rust,ignore
/// let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
///
/// match cb.call(|| async { some_operation().await }).await {
///     Ok(result) => println!("Success: {}", result),
///     Err(err) => println!("Failed: {}", err),
/// }
/// ```
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: std::sync::Mutex<CircuitBreakerState>,
}

struct CircuitBreakerState {
    state: CircuitState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_failure_time: Option<std::time::Instant>,
    active_probes: u32,
}

/// RAII guard ensuring active probe counter in `CircuitBreakerState` is always
/// decremented, even if the probe operation future is cancelled or dropped.
struct ProbeGuard<'a> {
    cb: &'a CircuitBreaker,
    active: bool,
}

impl<'a> ProbeGuard<'a> {
    fn new(cb: &'a CircuitBreaker) -> Self {
        Self { cb, active: true }
    }

    fn defuse(&mut self) {
        self.active = false;
    }
}

impl<'a> Drop for ProbeGuard<'a> {
    fn drop(&mut self) {
        if self.active {
            let mut guard = self.cb.state.lock().unwrap_or_else(|e| e.into_inner());
            if guard.active_probes > 0 {
                guard.active_probes -= 1;
            }
        }
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: std::sync::Mutex::new(CircuitBreakerState {
                state: CircuitState::Closed,
                consecutive_failures: 0,
                consecutive_successes: 0,
                last_failure_time: None,
                active_probes: 0,
            }),
        }
    }

    /// Get the current circuit state
    pub fn state(&self) -> CircuitState {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.state
    }

    /// Get the number of currently active in-flight probes during half-open state
    pub fn active_probes(&self) -> u32 {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.active_probes
    }

    /// Execute an operation through the circuit breaker
    pub async fn call<F, Fut, T>(&self, operation: F) -> GlobalResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = GlobalResult<T>>,
    {
        // Check if we should allow the call
        let is_probe = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            match guard.state {
                CircuitState::Open => {
                    // Check if recovery timeout has elapsed
                    if let Some(last_failure) = guard.last_failure_time {
                        if last_failure.elapsed() >= self.config.recovery_timeout {
                            guard.state = CircuitState::HalfOpen;
                            guard.consecutive_successes = 0;
                            guard.active_probes = 1;
                            true
                        } else {
                            return Err(GlobalError::Runtime(format!(
                                "Circuit breaker is open ({}s until recovery test)",
                                (self.config.recovery_timeout - last_failure.elapsed()).as_secs()
                            )));
                        }
                    } else {
                        guard.state = CircuitState::HalfOpen;
                        guard.consecutive_successes = 0;
                        guard.active_probes = 1;
                        true
                    }
                }
                CircuitState::HalfOpen => {
                    if guard.active_probes >= self.config.success_threshold {
                        return Err(GlobalError::Runtime(
                            "Circuit breaker is half-open: maximum concurrent probe requests reached"
                                .to_string(),
                        ));
                    }
                    guard.active_probes += 1;
                    true
                }
                CircuitState::Closed => false,
            }
        };

        let mut probe_guard = if is_probe {
            Some(ProbeGuard::new(self))
        } else {
            None
        };

        // Execute the operation
        match operation().await {
            Ok(value) => {
                if let Some(ref mut pg) = probe_guard {
                    pg.defuse();
                }
                self.record_success(is_probe);
                Ok(value)
            }
            Err(err) => {
                if let Some(ref mut pg) = probe_guard {
                    pg.defuse();
                }
                self.record_failure(is_probe);
                Err(err)
            }
        }
    }

    fn record_success(&self, is_probe: bool) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.consecutive_failures = 0;
        guard.consecutive_successes += 1;
        if is_probe && guard.active_probes > 0 {
            guard.active_probes -= 1;
        }

        if guard.state == CircuitState::HalfOpen
            && guard.consecutive_successes >= self.config.success_threshold
        {
            guard.state = CircuitState::Closed;
            guard.consecutive_successes = 0;
            guard.active_probes = 0;
        }
    }

    fn record_failure(&self, is_probe: bool) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.consecutive_failures += 1;
        guard.consecutive_successes = 0;
        if is_probe && guard.active_probes > 0 {
            guard.active_probes -= 1;
        }
        guard.last_failure_time = Some(std::time::Instant::now());

        if guard.state == CircuitState::Closed
            && guard.consecutive_failures >= self.config.failure_threshold
        {
            guard.state = CircuitState::Open;
            guard.active_probes = 0;
        } else if guard.state == CircuitState::HalfOpen {
            // Any failure in half-open state immediately re-opens
            guard.state = CircuitState::Open;
            guard.active_probes = 0;
        }
    }

    /// Reset the circuit breaker to closed state
    pub fn reset(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.state = CircuitState::Closed;
        guard.consecutive_failures = 0;
        guard.consecutive_successes = 0;
        guard.active_probes = 0;
        guard.last_failure_time = None;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Backoff tests --

    #[test]
    fn test_backoff_none() {
        assert_eq!(Backoff::None.delay_for(0), Duration::ZERO);
        assert_eq!(Backoff::None.delay_for(5), Duration::ZERO);
    }

    #[test]
    fn test_backoff_fixed() {
        let b = Backoff::fixed(500);
        assert_eq!(b.delay_for(0), Duration::from_millis(500));
        assert_eq!(b.delay_for(5), Duration::from_millis(500));
    }

    #[test]
    fn test_backoff_linear() {
        let b = Backoff::linear(100, 200);
        assert_eq!(b.delay_for(0), Duration::from_millis(100));
        assert_eq!(b.delay_for(1), Duration::from_millis(300));
        assert_eq!(b.delay_for(2), Duration::from_millis(500));
    }

    #[test]
    fn test_backoff_exponential() {
        let b = Backoff::exponential(100, 5000);
        assert_eq!(b.delay_for(0), Duration::from_millis(100));
        assert_eq!(b.delay_for(1), Duration::from_millis(200));
        assert_eq!(b.delay_for(2), Duration::from_millis(400));
        assert_eq!(b.delay_for(3), Duration::from_millis(800));
        // Capped at max
        assert_eq!(b.delay_for(10), Duration::from_millis(5000));
    }

    // -- RetryPolicy tests --

    #[test]
    fn test_retry_policy_no_retry() {
        let p = RetryPolicy::no_retry();
        assert_eq!(p.max_attempts, 1);
    }

    #[test]
    fn test_retry_policy_default() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_attempts, 3);
        // Retryable errors should be retried
        assert!(p.should_retry(&GlobalError::llm("timeout")));
        assert!(p.should_retry(&GlobalError::runtime("network error")));
        // Non-retryable errors should not
        assert!(!p.should_retry(&GlobalError::config("bad")));
        assert!(!p.should_retry(&GlobalError::persistence("constraint")));
    }

    #[test]
    fn test_retry_policy_builder() {
        let p = RetryPolicy::builder()
            .max_attempts(5)
            .backoff(Backoff::fixed(200))
            .retry_on(vec![ErrorCategory::LLM, ErrorCategory::Plugin])
            .build();

        assert_eq!(p.max_attempts, 5);
        assert!(p.should_retry(&GlobalError::llm("timeout")));
        assert!(p.should_retry(&GlobalError::plugin("temp")));
        // Runtime is NOT in the retry list
        assert!(!p.should_retry(&GlobalError::runtime("net")));
    }

    // -- retry() tests --

    #[tokio::test]
    async fn test_retry_immediate_success() {
        let result = retry(RetryPolicy::with_attempts(3), || async {
            Ok::<_, GlobalError>("ok".to_string())
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "ok");
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();

        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .backoff(Backoff::None)
            .build();

        let result = retry(policy, || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n < 2 {
                    Err(GlobalError::runtime("transient"))
                } else {
                    Ok("recovered".to_string())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let policy = RetryPolicy::builder()
            .max_attempts(2)
            .backoff(Backoff::None)
            .build();

        let result: GlobalResult<String> = retry(policy, || async {
            Err(GlobalError::llm("persistent failure"))
        })
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("persistent"));
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error_stops_immediately() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();

        let policy = RetryPolicy::builder()
            .max_attempts(5)
            .backoff(Backoff::None)
            .build();

        let result: GlobalResult<String> = retry(policy, || {
            let c = c.clone();
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // Config errors are NOT retryable by default
                Err(GlobalError::config("fatal config issue"))
            }
        })
        .await;

        assert!(result.is_err());
        // Should have been called only once (non-retryable error stops immediately)
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    // -- CircuitBreaker tests --

    #[tokio::test]
    async fn test_circuit_breaker_success() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        assert_eq!(cb.state(), CircuitState::Closed);

        let result = cb
            .call(|| async { Ok::<_, GlobalError>("hello".to_string()) })
            .await;
        assert!(result.is_ok());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Trigger 3 failures
        for _ in 0..3 {
            let _ = cb
                .call(|| async { Err::<String, _>(GlobalError::llm("fail")) })
                .await;
        }

        assert_eq!(cb.state(), CircuitState::Open);

        // Next call should be rejected
        let result = cb
            .call(|| async { Ok::<_, GlobalError>("should not run".to_string()) })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circuit breaker"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Trigger failures to open
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<String, _>(GlobalError::llm("fail")) })
                .await;
        }
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Next call should go through (half-open), succeed, and close
        let result = cb
            .call(|| async { Ok::<_, GlobalError>("recovered".to_string()) })
            .await;
        assert!(result.is_ok());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<String, _>(GlobalError::llm("fail")) })
                .await;
        }

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Fail in half-open → should re-open
        let _ = cb
            .call(|| async { Err::<String, _>(GlobalError::llm("still failing")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        let _ = cb
            .call(|| async { Err::<String, _>(GlobalError::llm("fail")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_limits_concurrent_probes() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };
        let cb = std::sync::Arc::new(CircuitBreaker::new(config));

        // Trip the breaker to open
        let _ = cb
            .call(|| async { Err::<String, _>(GlobalError::llm("fail")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery timeout to elapse
        tokio::time::sleep(Duration::from_millis(60)).await;

        let (probe1_tx, probe1_rx) = tokio::sync::oneshot::channel::<()>();
        let cb_clone = cb.clone();

        // Launch probe 1 (holds open while waiting on channel)
        let handle1 = tokio::spawn(async move {
            cb_clone
                .call(|| async {
                    probe1_rx.await.unwrap();
                    Ok::<_, GlobalError>("probe1_success".to_string())
                })
                .await
        });

        // Small yield so probe 1 transitions breaker to HalfOpen and claims the 1 probe slot
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert_eq!(cb.active_probes(), 1);

        // Launch probe 2 while probe 1 is still in flight -> must be rejected immediately
        let probe2_res = cb
            .call(|| async { Ok::<_, GlobalError>("probe2".to_string()) })
            .await;

        assert!(probe2_res.is_err());
        let err_msg = probe2_res.unwrap_err().to_string();
        assert!(
            err_msg.contains("maximum concurrent probe requests reached"),
            "Expected concurrent probe rejection, got: {err_msg}"
        );

        // Allow probe 1 to complete
        probe1_tx.send(()).unwrap();
        let probe1_res = handle1.await.unwrap();
        assert!(probe1_res.is_ok());
        assert_eq!(probe1_res.unwrap(), "probe1_success");

        // Circuit should now be closed and active probes reset to 0
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.active_probes(), 0);

        // Subsequent normal requests now pass through
        let next_call = cb
            .call(|| async { Ok::<_, GlobalError>("normal".to_string()) })
            .await;
        assert!(next_call.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_allows_multiple_probes_up_to_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 2,
        };
        let cb = std::sync::Arc::new(CircuitBreaker::new(config));

        // Trip to open
        let _ = cb
            .call(|| async { Err::<String, _>(GlobalError::llm("fail")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        tokio::time::sleep(Duration::from_millis(60)).await;

        let (tx1, rx1) = tokio::sync::oneshot::channel::<()>();
        let (tx2, rx2) = tokio::sync::oneshot::channel::<()>();
        let cb1 = cb.clone();
        let cb2 = cb.clone();

        // Launch probe 1
        let h1 = tokio::spawn(async move {
            cb1.call(|| async {
                rx1.await.unwrap();
                Ok::<_, GlobalError>("p1".to_string())
            })
            .await
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(cb.active_probes(), 1);

        // Launch probe 2 (success_threshold is 2, so this should also be allowed)
        let h2 = tokio::spawn(async move {
            cb2.call(|| async {
                rx2.await.unwrap();
                Ok::<_, GlobalError>("p2".to_string())
            })
            .await
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(cb.active_probes(), 2);

        // Launch probe 3 (exceeds threshold 2) -> must be rejected
        let p3_res = cb
            .call(|| async { Ok::<_, GlobalError>("p3".to_string()) })
            .await;
        assert!(p3_res.is_err());
        assert!(p3_res.unwrap_err().to_string().contains("maximum concurrent probe requests reached"));

        // Finish probe 1
        tx1.send(()).unwrap();
        assert!(h1.await.unwrap().is_ok());
        // Circuit still half-open because only 1 of 2 required successes completed
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert_eq!(cb.active_probes(), 1);

        // Finish probe 2
        tx2.send(()).unwrap();
        assert!(h2.await.unwrap().is_ok());
        // Circuit now closed
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.active_probes(), 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_probe_cancellation_decrements_counter() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Trip to open
        let _ = cb
            .call(|| async { Err::<String, _>(GlobalError::llm("fail")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        tokio::time::sleep(Duration::from_millis(60)).await;

        // Run a probe that times out and is cancelled
        let timed_out = tokio::time::timeout(
            Duration::from_millis(15),
            cb.call(|| async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, GlobalError>("never".to_string())
            }),
        )
        .await;

        assert!(timed_out.is_err(), "Expected timeout");
        // Due to ProbeGuard, active_probes should have safely dropped back to 0
        assert_eq!(cb.active_probes(), 0);

        // Next probe is NOT blocked by a leaked counter
        let probe2 = cb
            .call(|| async { Ok::<_, GlobalError>("recovered".to_string()) })
            .await;
        assert!(probe2.is_ok());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // -- fallback_chain tests --

    #[tokio::test]
    async fn test_fallback_chain_first_success() {
        let result = fallback_chain(vec![
            Box::new(|| Box::pin(async { Ok::<_, GlobalError>("primary".to_string()) })),
            Box::new(|| Box::pin(async { Ok::<_, GlobalError>("secondary".to_string()) })),
        ])
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "primary");
    }

    #[tokio::test]
    async fn test_fallback_chain_falls_through() {
        let result = fallback_chain(vec![
            Box::new(|| Box::pin(async { Err::<String, _>(GlobalError::llm("primary failed")) })),
            Box::new(|| {
                Box::pin(async { Err::<String, _>(GlobalError::runtime("secondary failed")) })
            }),
            Box::new(|| Box::pin(async { Ok::<_, GlobalError>("tertiary".to_string()) })),
        ])
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "tertiary");
    }

    #[tokio::test]
    async fn test_fallback_chain_all_fail() {
        let result: GlobalResult<String> = fallback_chain(vec![
            Box::new(|| Box::pin(async { Err(GlobalError::llm("a")) })),
            Box::new(|| Box::pin(async { Err(GlobalError::llm("b")) })),
        ])
        .await;

        assert!(result.is_err());
        // Returns the LAST error
        assert!(result.unwrap_err().to_string().contains("b"));
    }
}
