//! Framework-level gateway kernel contract.
//!
//! This module defines the *trait interfaces and configuration types* for the
//! MoFA Cognitive Gateway.  No concrete implementations live here — those
//! belong in `mofa-gateway` (runtime) and `mofa-plugins` (adapters).
//!
//! # Architecture mapping
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │              mofa-kernel  (this module)                     │
//! │  GatewayRouter trait    CapabilityRegistry trait            │
//! │  GatewayFilter trait    GatewayConfig + validate()          │
//! │  GatewayRequest/Response/Context  GatewayError              │
//! └──────────────────────────┬──────────────────────────────────┘
//!                            │  depends on
//! ┌──────────────────────────▼──────────────────────────────────┐
//! │              mofa-gateway  (runtime crate)                  │
//! │  TrieRouter: impl GatewayRouter                             │
//! │  InMemoryCapabilityRegistry: impl CapabilityRegistry        │
//! │  ApiKeyFilter / RateLimitFilter / LoggingFilter             │
//! │  GatewayServer  (axum HTTP server)                          │
//! │  OpenAiBackend  (reqwest proxy)                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod error;

// ── Flat re-exports ────────────────────────────────────────────────────────

pub use error::GatewayError;

// types module is pub so implementors in mofa-gateway can use the structs
pub mod types;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, HttpMethod, RouteMatch};
