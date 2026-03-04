//! Error Recovery Traits
//!
//! Provides the core `ErrorRecovery` trait for domain-specific recovery logic.
//!
//! Concrete implementations (Backoff, RetryPolicy, CircuitBreaker, retry(),
//! fallback_chain()) are in `mofa_foundation::recovery`.

use super::error::GlobalError;

// ============================================================================
// ErrorRecovery trait - domain-specific recovery logic
// ============================================================================

/// Trait for types that know how to recover from errors.
///
/// Implement this on your service/component to define domain-specific
/// recovery logic based on the error category and severity.
#[async_trait::async_trait]
pub trait ErrorRecovery {
    /// The output type produced on successful recovery
    type Output;

    /// Attempt to recover from the error.
    ///
    /// Returns `Some(output)` if recovery succeeded, `None` if the error
    /// is unrecoverable. The default implementation returns `None` for
    /// fatal errors and delegates to `recover_impl` for others.
    async fn recover(&self, error: &GlobalError) -> Option<Self::Output> {
        if error.is_fatal() {
            return None;
        }
        self.recover_impl(error).await
    }

    /// Implementation-specific recovery logic.
    ///
    /// Override this to define how your service recovers from different
    /// error categories.
    async fn recover_impl(&self, error: &GlobalError) -> Option<Self::Output>;
}
