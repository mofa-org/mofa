//! Kernel-level rate-limiting contract for the gateway.
//!
//! This module defines the trait boundary and supporting types that all rate
//! limiter implementations must satisfy.  Concrete implementations (e.g.
//! token-bucket, sliding-window) live in `mofa-foundation::gateway`.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Decision
// ─────────────────────────────────────────────────────────────────────────────

/// The outcome of a rate-limit check.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RateLimitDecision {
    /// Request is within quota.
    Allowed {
        /// Tokens remaining in the bucket after this request.
        remaining: u32,
    },
    /// Request exceeds quota.
    Denied {
        /// Milliseconds the caller should wait before retrying.
        retry_after_ms: u64,
    },
}

impl RateLimitDecision {
    /// Returns `true` if the request was allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// KeyStrategy
// ─────────────────────────────────────────────────────────────────────────────

/// Which dimension to key the rate limiter on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum KeyStrategy {
    /// One bucket per agent ID (from the matched route).
    PerAgent,
    /// One bucket per originating client IP address.
    PerClient,
}

// ─────────────────────────────────────────────────────────────────────────────
// RateLimiterConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration shared by all rate limiter implementations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    /// Maximum number of tokens (burst size).
    pub capacity: u32,
    /// Number of tokens added per second (sustained rate).
    pub refill_rate: u32,
    /// Keying strategy.
    pub strategy: KeyStrategy,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            capacity: 100,
            refill_rate: 10,
            strategy: KeyStrategy::PerClient,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RateLimiter trait
// ─────────────────────────────────────────────────────────────────────────────

/// Kernel contract for gateway rate limiting.
///
/// Implementations must be `Send + Sync` so they can be held behind an `Arc`
/// and called from multiple Tokio tasks concurrently.
pub trait GatewayRateLimiter: Send + Sync {
    /// Attempt to consume one token from the bucket identified by `key`.
    ///
    /// Returns [`RateLimitDecision::Allowed`] when a token was successfully
    /// consumed, or [`RateLimitDecision::Denied`] with a retry hint when the
    /// bucket is empty.
    fn check_and_consume(&self, key: &str) -> RateLimitDecision;
}
