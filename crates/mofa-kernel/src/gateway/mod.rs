//! Cognitive Gateway kernel contract.

mod error;
mod types;
mod router;
mod filter;
mod capability;
mod validation;

// ── Flat re-exports ────────────────────────────────────────────────────────

pub use error::GatewayError;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, HttpMethod, RouteMatch};
pub use router::{GatewayRouter, RouteConfig};
pub use filter::{FilterAction, FilterChainConfig, FilterOrder, GatewayFilter};
pub use capability::{BackendHealth, BackendKind, CapabilityDescriptor, CapabilityRegistry};
pub use validation::{GatewayConfig, RateLimitConfig};
