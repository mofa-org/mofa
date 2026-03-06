//! Cognitive Gateway kernel contract — types and errors.

mod error;
mod types;

// ── Flat re-exports ────────────────────────────────────────────────────────
// Types are exposed via curated re-exports to keep the public API surface tight.

pub use error::GatewayError;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, HttpMethod, RouteMatch};
