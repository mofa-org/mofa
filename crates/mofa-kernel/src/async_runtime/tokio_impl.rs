//! Tokio-based implementation of [`AsyncRuntime`].
//!
//! This module provides [`TokioRuntime`], the default async runtime for MoFA.
//! It is enabled by the `runtime-tokio` feature flag (on by default).

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use super::{AsyncRuntime, JoinHandle, SleepFuture, TimeoutError, TimeoutFuture};

/// Default async runtime implementation backed by tokio.
///
/// This is a zero-sized struct that delegates all operations to tokio's
/// global runtime. It requires that a tokio runtime is already running
/// (e.g., via `#[tokio::main]` or `tokio::runtime::Runtime`).
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::async_runtime::tokio_impl::TokioRuntime;
/// use mofa_kernel::async_runtime::AsyncRuntime;
///
/// let rt = TokioRuntime;
/// rt.sleep(std::time::Duration::from_millis(100)).await;
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRuntime;

impl TokioRuntime {
    /// Create a new `TokioRuntime` instance.
    pub fn new() -> Self {
        Self
    }
}

impl AsyncRuntime for TokioRuntime {
    fn spawn(&self, task: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> JoinHandle<()> {
        let handle = tokio::spawn(task);
        JoinHandle::new(async move { handle.await.ok() })
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> JoinHandle<()> {
        let handle = tokio::task::spawn_blocking(f);
        JoinHandle::new(async move { handle.await.ok() })
    }

    fn sleep(&self, duration: Duration) -> SleepFuture {
        Box::pin(tokio::time::sleep(duration))
    }

    fn timeout(
        &self,
        duration: Duration,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> TimeoutFuture<()> {
        Box::pin(async move {
            tokio::time::timeout(duration, future)
                .await
                .map_err(|_| TimeoutError)
        })
    }

    fn now(&self) -> std::time::Instant {
        std::time::Instant::now()
    }
}
