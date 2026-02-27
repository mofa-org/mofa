//! Async Runtime Abstraction Layer
//!
//! Provides a trait-based abstraction over async runtimes, allowing MoFA to work
//! with different async runtimes beyond tokio. The default implementation uses tokio,
//! but custom runtimes can be plugged in via the `custom-runtime` feature flag.
//!
//! # Architecture
//!
//! - [`AsyncRuntime`] — Core trait defining async operations (spawn, sleep, timeout, etc.)
//! - [`TokioRuntime`](tokio_impl::TokioRuntime) — Default tokio-based implementation
//!   (requires `runtime-tokio` feature)
//! - [`RuntimeBuilder`](builder::RuntimeBuilder) — Builder for constructing runtime instances
//! - [`set_global_runtime`](builder::set_global_runtime) / [`global_runtime`](builder::global_runtime)
//!   — Global runtime accessor for framework-wide use
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_kernel::async_runtime::{AsyncRuntime, builder::RuntimeBuilder};
//!
//! // Use the default tokio runtime
//! let runtime = RuntimeBuilder::new().build();
//! runtime.sleep(std::time::Duration::from_millis(100)).await;
//! ```

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// A type-erased future that completes after a sleep duration.
pub type SleepFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A type-erased future that completes with the inner result or a timeout error.
pub type TimeoutFuture<T> = Pin<Box<dyn Future<Output = Result<T, TimeoutError>> + Send>>;

/// Error returned when an async operation exceeds its deadline.
#[derive(Debug, Clone)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for TimeoutError {}

/// A type-erased handle to a spawned task.
///
/// Awaiting a `JoinHandle` waits for the spawned task to complete and returns
/// its result. If the task panics or is cancelled, awaiting returns `None`.
pub struct JoinHandle<T> {
    inner: Pin<Box<dyn Future<Output = Option<T>> + Send>>,
}

impl<T> JoinHandle<T> {
    /// Create a new `JoinHandle` wrapping the given future.
    pub fn new(fut: impl Future<Output = Option<T>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(fut),
        }
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Option<T>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.inner.as_mut().poll(cx)
    }
}

/// Core trait for async runtime abstraction.
///
/// Implementors provide the fundamental async operations needed by MoFA:
/// spawning tasks, sleeping, timeouts, and blocking work offloading.
///
/// # Trait Object Safety
///
/// This trait is designed to be used as `Arc<dyn AsyncRuntime>`. Methods that
/// accept generic futures use `Pin<Box<dyn Future>>` parameters to maintain
/// object safety.
pub trait AsyncRuntime: Send + Sync {
    /// Spawn a `Send + 'static` future onto the runtime, returning a handle
    /// that can be awaited to get the task's result.
    fn spawn(&self, task: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> JoinHandle<()>;

    /// Run a blocking closure on a dedicated thread pool, returning a handle
    /// to await its result.
    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> JoinHandle<()>;

    /// Return a future that completes after the given duration.
    fn sleep(&self, duration: Duration) -> SleepFuture;

    /// Wrap a future with a timeout. Returns `Err(TimeoutError)` if the
    /// inner future does not complete within the given duration.
    fn timeout(
        &self,
        duration: Duration,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> TimeoutFuture<()>;

    /// Return the current instant. Useful for testing with mock clocks.
    fn now(&self) -> std::time::Instant;
}

// Sub-modules
#[cfg(feature = "runtime-tokio")]
pub mod tokio_impl;

pub mod builder;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "runtime-tokio")]
    mod tokio_tests {
        use super::*;
        use crate::async_runtime::tokio_impl::TokioRuntime;

        #[tokio::test]
        async fn test_tokio_runtime_sleep() {
            let rt = TokioRuntime;
            let start = rt.now();
            rt.sleep(Duration::from_millis(50)).await;
            let elapsed = start.elapsed();
            assert!(
                elapsed >= Duration::from_millis(40),
                "sleep should wait at least ~50ms, got {:?}",
                elapsed
            );
        }

        #[tokio::test]
        async fn test_tokio_runtime_spawn() {
            let rt = TokioRuntime;
            let (tx, rx) = tokio::sync::oneshot::channel();
            let handle = rt.spawn(Box::pin(async move {
                tx.send(42).unwrap();
            }));
            let result = handle.await;
            assert!(result.is_some(), "spawn should complete successfully");
            let value = rx.await.unwrap();
            assert_eq!(value, 42);
        }

        #[tokio::test]
        async fn test_tokio_runtime_spawn_blocking() {
            let rt = TokioRuntime;
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = rt.spawn_blocking(Box::new(move || {
                tx.send(99).unwrap();
            }));
            let result = handle.await;
            assert!(
                result.is_some(),
                "spawn_blocking should complete successfully"
            );
            let value = rx.recv().unwrap();
            assert_eq!(value, 99);
        }

        #[tokio::test]
        async fn test_tokio_runtime_timeout_ok() {
            let rt = TokioRuntime;
            let result = rt
                .timeout(
                    Duration::from_secs(1),
                    Box::pin(async {
                        // completes immediately
                    }),
                )
                .await;
            assert!(result.is_ok(), "timeout should succeed for fast future");
        }

        #[tokio::test]
        async fn test_tokio_runtime_timeout_expired() {
            let rt = TokioRuntime;
            let result = rt
                .timeout(
                    Duration::from_millis(10),
                    Box::pin(async {
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    }),
                )
                .await;
            assert!(result.is_err(), "timeout should fail for slow future");
            let err = result.unwrap_err();
            assert_eq!(err.to_string(), "operation timed out");
        }

        #[tokio::test]
        async fn test_tokio_runtime_now() {
            let rt = TokioRuntime;
            let now1 = rt.now();
            tokio::time::sleep(Duration::from_millis(10)).await;
            let now2 = rt.now();
            assert!(
                now2 > now1,
                "now() should return monotonically increasing instants"
            );
        }
    }
}
