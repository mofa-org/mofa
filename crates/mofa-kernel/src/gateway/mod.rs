//! Cognitive Gateway kernel contract.

mod error;
mod types;
mod router;
mod filter;

// ── Flat re-exports ────────────────────────────────────────────────────────
// Types are exposed via curated re-exports to keep the public API surface tight.

pub use error::GatewayError;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, HttpMethod, RouteMatch};
pub use router::{GatewayRouter, RouteConfig};
pub use filter::{FilterAction, FilterChainConfig, FilterOrder, GatewayFilter};
