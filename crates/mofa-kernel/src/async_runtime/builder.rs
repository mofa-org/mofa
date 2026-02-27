//! Runtime builder and global runtime accessor.
//!
//! Provides [`RuntimeBuilder`] for constructing an [`AsyncRuntime`] instance,
//! and global accessor functions for framework-wide runtime access.
//!
//! # Global Runtime
//!
//! The global runtime is set once at application startup and accessed throughout
//! the framework. If not explicitly set, it defaults to [`TokioRuntime`] when
//! the `runtime-tokio` feature is enabled.
//!
//! ```rust,ignore
//! use mofa_kernel::async_runtime::builder::{set_global_runtime, global_runtime, RuntimeBuilder};
//!
//! // Initialize once at startup
//! let runtime = RuntimeBuilder::new().build();
//! set_global_runtime(runtime).expect("runtime already set");
//!
//! // Access anywhere in the framework
//! let rt = global_runtime();
//! rt.sleep(std::time::Duration::from_millis(100)).await;
//! ```

use std::sync::{Arc, OnceLock};

use super::AsyncRuntime;

/// Global runtime singleton.
static GLOBAL_RUNTIME: OnceLock<Arc<dyn AsyncRuntime>> = OnceLock::new();

/// Set the global async runtime.
///
/// This should be called once at application startup. Returns `Err` with the
/// provided runtime if a global runtime has already been set.
pub fn set_global_runtime(runtime: Arc<dyn AsyncRuntime>) -> Result<(), Arc<dyn AsyncRuntime>> {
    GLOBAL_RUNTIME.set(runtime)
}

/// Get the global async runtime.
///
/// Returns the runtime previously set via [`set_global_runtime`]. If no runtime
/// has been set and the `runtime-tokio` feature is enabled, it automatically
/// initializes with [`TokioRuntime`](super::tokio_impl::TokioRuntime).
///
/// # Panics
///
/// Panics if no runtime has been set and the `runtime-tokio` feature is disabled.
pub fn global_runtime() -> Arc<dyn AsyncRuntime> {
    GLOBAL_RUNTIME
        .get_or_init(|| {
            #[cfg(feature = "runtime-tokio")]
            {
                Arc::new(super::tokio_impl::TokioRuntime)
            }
            #[cfg(not(feature = "runtime-tokio"))]
            {
                panic!(
                    "No global async runtime set. Call set_global_runtime() at startup, \
                     or enable the `runtime-tokio` feature for automatic initialization."
                );
            }
        })
        .clone()
}

/// Builder for constructing an [`AsyncRuntime`] instance.
///
/// Defaults to [`TokioRuntime`](super::tokio_impl::TokioRuntime) when the
/// `runtime-tokio` feature is enabled.
pub struct RuntimeBuilder {
    runtime: Option<Arc<dyn AsyncRuntime>>,
}

impl RuntimeBuilder {
    /// Create a new builder. Defaults to tokio when the `runtime-tokio` feature is enabled.
    pub fn new() -> Self {
        Self { runtime: None }
    }

    /// Use a custom runtime implementation.
    pub fn with_custom(mut self, runtime: impl AsyncRuntime + 'static) -> Self {
        self.runtime = Some(Arc::new(runtime));
        self
    }

    /// Build the runtime. Returns the configured runtime, or the default tokio
    /// runtime if none was explicitly set.
    ///
    /// # Panics
    ///
    /// Panics if no custom runtime was provided and the `runtime-tokio` feature
    /// is disabled.
    pub fn build(self) -> Arc<dyn AsyncRuntime> {
        if let Some(runtime) = self.runtime {
            return runtime;
        }

        #[cfg(feature = "runtime-tokio")]
        {
            Arc::new(super::tokio_impl::TokioRuntime)
        }

        #[cfg(not(feature = "runtime-tokio"))]
        {
            panic!(
                "No runtime configured. Either provide a custom runtime via \
                 with_custom() or enable the `runtime-tokio` feature."
            );
        }
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "runtime-tokio")]
    mod builder_tests {
        use super::*;
        use std::time::Duration;

        #[tokio::test]
        async fn test_runtime_builder_default() {
            let rt = RuntimeBuilder::new().build();
            // Verify it works by calling sleep
            rt.sleep(Duration::from_millis(1)).await;
        }

        #[tokio::test]
        async fn test_runtime_builder_with_custom() {
            use crate::async_runtime::tokio_impl::TokioRuntime;
            // Use TokioRuntime as a "custom" runtime to verify the builder path
            let rt = RuntimeBuilder::new().with_custom(TokioRuntime).build();
            rt.sleep(Duration::from_millis(1)).await;
        }

        #[tokio::test]
        async fn test_global_runtime_auto_init() {
            // global_runtime() should auto-initialize with TokioRuntime
            let rt = global_runtime();
            rt.sleep(Duration::from_millis(1)).await;
        }
    }
}
