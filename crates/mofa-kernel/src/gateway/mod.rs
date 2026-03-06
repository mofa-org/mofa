//! Kernel-level gateway abstractions for agent request dispatch.
//!
//! This module defines the trait boundary between the gateway transport layer
//! (HTTP, gRPC, MQTT, …) and the agent runtime.  By keeping routing and
//! envelope logic in the kernel, alternative transports and unit tests can
//! reason about the full request lifecycle without depending on the full HTTP
//! stack.
//!
//! # Types
//!
//! | Type | Description |
//! |------|-------------|
//! | [`GatewayRoute`] | Routing rule mapping path + method to an agent |
//! | [`RouteRegistry`] | Trait for registering, looking up, and listing routes |
//! | [`RoutingContext`] | Per-request dispatch context |
//! | [`HttpMethod`] | HTTP method enum |
//! | [`RegistryError`] | Error type for registry operations |
//! | [`GatewayConfigError`] | Error type for gateway configuration validation |
//! | [`GatewayRequest`] | Inbound HTTP request model |
//! | [`GatewayResponse`] | Outbound HTTP response model |
//! | [`GatewayContext`] | Per-request mutable context for filter chains |
//! | [`RouteMatch`] | Result of a successful route lookup |
//! | [`RequestEnvelope`] | Typed inbound request envelope flowing through the pipeline |
//! | [`AgentResponse`] | Typed agent response for access logging, metrics, and admin API |
//! | [`AuthClaims`] | Verified identity produced by any auth backend |
//! | [`AuthProvider`] | Async trait for authenticating requests |
//! | [`ApiKeyStore`] | Persistence trait for API key lifecycle |
//! | [`AuthError`] | Auth failure error enum |

pub mod auth;
pub mod envelope;
pub mod error;
pub mod route;
mod config_error;
mod types;

#[cfg(test)]
mod tests;

pub use auth::{ApiKeyStore, AuthClaims, AuthError, AuthProvider};
pub use envelope::{AgentResponse, RequestEnvelope};
pub use error::RegistryError;
pub use route::{GatewayRoute, HttpMethod, RouteRegistry, RoutingContext};
pub use config_error::GatewayConfigError;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, RouteMatch};
