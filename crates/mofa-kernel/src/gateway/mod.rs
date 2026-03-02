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
//!
//! # Quick start
//!
//! ```rust,no_run
//! use mofa_kernel::gateway::{
//!     GatewayConfig, RouteConfig, CapabilityDescriptor, BackendKind,
//!     FilterChainConfig,
//! };
//!
//! let config = GatewayConfig::new("my-gateway")
//!     .with_backend(CapabilityDescriptor::new(
//!         "openai",
//!         BackendKind::LlmOpenAI,
//!         "https://api.openai.com",
//!     ))
//!     .with_route(RouteConfig::new(
//!         "chat",
//!         "/v1/chat/completions",
//!         "openai",
//!     ));
//!
//! config.validate().expect("gateway config is valid");
//! ```

pub mod capability;
pub mod error;
pub mod filter;
pub mod router;
pub mod validation;

// ── Flat re-exports ────────────────────────────────────────────────────────

pub use capability::{BackendHealth, BackendKind, CapabilityDescriptor, CapabilityRegistry};
pub use error::GatewayError;
pub use filter::{FilterAction, FilterChainConfig, FilterOrder, GatewayFilter};
pub use router::{GatewayRouter, RouteConfig};
pub use validation::{GatewayConfig, RateLimitConfig};

// types module is pub so implementors in mofa-gateway can use the structs
pub mod types;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, HttpMethod, RouteMatch};
