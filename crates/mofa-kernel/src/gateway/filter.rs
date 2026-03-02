//! Gateway filter trait and filter-chain types.
//!
//! A filter chain is an ordered list of [`GatewayFilter`] instances applied
//! to every request and response.  Filters are sorted by their declared
//! [`FilterOrder`] and executed in ascending order on the request path
//! (lowest value first) and descending order on the response path.
//!
//! ```text
//! Request  ──► PreAuth ──► Auth ──► RateLimit ──► Transform ──► Logging
//!                  (upstream / backend call happens here)
//! Response ◄── Logging ◄── Transform ◄── RateLimit ◄── Auth ◄── PreAuth
//! ```

use super::error::GatewayError;
use super::types::{GatewayContext, GatewayResponse};
use async_trait::async_trait;

// ─────────────────────────────────────────────────────────────────────────────
// Filter ordering
// ─────────────────────────────────────────────────────────────────────────────

/// Numeric ordering slot for a filter in the chain.
///
/// The well-known slots below act as guidelines; any `u32` value is accepted
/// so implementors can slot in custom filters between the standard phases.
/// Filters with equal order values are executed in registration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FilterOrder(pub u32);

impl FilterOrder {
    /// Executes before all authentication logic (e.g. request ID injection).
    pub const PRE_AUTH: FilterOrder = FilterOrder(0);
    /// Authentication filter slot (API key, JWT, OAuth 2.0).
    pub const AUTH: FilterOrder = FilterOrder(100);
    /// Rate-limiting / throttling slot.
    pub const RATE_LIMIT: FilterOrder = FilterOrder(200);
    /// Request / response body transformation slot.
    pub const TRANSFORM: FilterOrder = FilterOrder(300);
    /// Audit logging slot — runs after all transformations.
    pub const LOGGING: FilterOrder = FilterOrder(400);
    /// Post-processing, metrics recording, etc.
    pub const POST_PROCESS: FilterOrder = FilterOrder(500);
}

// ─────────────────────────────────────────────────────────────────────────────
// Filter action
// ─────────────────────────────────────────────────────────────────────────────

/// Instruction returned by [`GatewayFilter::on_request`] controlling what
/// the gateway does with the request after the filter runs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FilterAction {
    /// Pass the (possibly modified) request to the next filter or backend.
    Continue,
    /// Short-circuit the chain and return a synthetic error response with the
    /// given HTTP status and body string.
    Reject(u16, String),
    /// Short-circuit and redirect the caller to a different path.
    Redirect(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayFilter trait
// ─────────────────────────────────────────────────────────────────────────────

/// Kernel contract for a single filter in the gateway pipeline.
///
/// Implementations must be `Send + Sync` so they can be shared across Tokio
/// tasks without additional synchronization by the caller.
#[async_trait]
pub trait GatewayFilter: Send + Sync {
    /// Stable, human-readable identifier for this filter (used in logs).
    fn name(&self) -> &str;

    /// Position in the filter chain.  Lower values execute first on the
    /// request path.
    fn order(&self) -> FilterOrder;

    /// Called with the inbound request *before* it is forwarded to the backend.
    ///
    /// Implementations may mutate `ctx` (e.g. add authentication claims to
    /// `ctx.auth_principal`, remove sensitive headers, …).  Return
    /// [`FilterAction::Continue`] to proceed, or a `Reject`/`Redirect` variant
    /// to short-circuit the chain.
    async fn on_request(&self, ctx: &mut GatewayContext) -> Result<FilterAction, GatewayError>;

    /// Called with the backend response *before* it is returned to the caller.
    ///
    /// Implementations may mutate `resp` (e.g. strip internal headers, append
    /// cache-control metadata, record latency metrics, …).
    async fn on_response(
        &self,
        ctx: &GatewayContext,
        resp: &mut GatewayResponse,
    ) -> Result<(), GatewayError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// FilterChainConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Ordered list of filter names that make up a named filter chain.
///
/// This is the *configuration* representation (list of string names).  The
/// runtime binds names to concrete [`GatewayFilter`] implementations during
/// startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterChainConfig {
    /// Human-readable name for this chain (used in logs and metrics).
    pub name: String,
    /// Ordered filter names.  Must not be empty — validated by
    /// [`GatewayConfig::validate()`](super::validation::GatewayConfig::validate).
    pub filter_names: Vec<String>,
}

impl FilterChainConfig {
    /// Create a new chain config with the given name and filter list.
    pub fn new(name: impl Into<String>, filter_names: Vec<String>) -> Self {
        Self {
            name: name.into(),
            filter_names,
        }
    }
}
