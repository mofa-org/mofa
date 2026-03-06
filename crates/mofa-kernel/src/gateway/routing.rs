//! Kernel-level routing strategy trait.
//!
//! [`RoutingStrategy`] is the single contract every routing backend must
//! implement.  Concrete strategies (weighted round-robin, capability-match,
//! etc.) live in `mofa-foundation`.

use super::envelope::RequestEnvelope;

/// Kernel contract for a request routing strategy.
///
/// Given an inbound [`RequestEnvelope`], the strategy returns the agent ID
/// that should handle the request, or `None` when no suitable agent is
/// available (e.g. all backends are unhealthy or the capability threshold is
/// not met).
///
/// Implementations must be `Send + Sync` so they can be held behind an `Arc`
/// and called concurrently from multiple Tokio tasks.
pub trait RoutingStrategy: Send + Sync {
    /// Select an agent for the given request.
    ///
    /// Returns `Some(agent_id)` on success, `None` when the strategy cannot
    /// select an agent.
    fn select_agent(&self, envelope: &RequestEnvelope) -> Option<String>;
}
