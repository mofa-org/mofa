//! Kernel-level gateway abstractions for agent request dispatch.
//!
//! | Type | Description |
//! |------|-------------|
//! | [`GatewayRoute`] | Routing rule mapping path + method to an agent |
//! | [`RouteRegistry`] | Trait for registering, looking up, and listing routes |
//! | [`RoutingContext`] | Per-request dispatch context |
//! | [`HttpMethod`] | HTTP method enum |
//! | [`RegistryError`] | Error type for registry operations |
//! | [`RequestEnvelope`] | Typed inbound request envelope |
//! | [`GatewayResponse`] | Typed response for logging and metrics |
//! | [`AuthClaims`] | Verified identity produced by any auth backend |
//! | [`AuthProvider`] | Async trait for authenticating requests |
//! | [`ApiKeyStore`] | Persistence trait for API key lifecycle |
//! | [`AuthError`] | Auth failure error enum |

pub mod auth;
pub mod envelope;
pub mod error;
pub mod route;

#[cfg(test)]
mod tests;

pub use auth::{ApiKeyStore, AuthClaims, AuthError, AuthProvider};
pub use envelope::{GatewayResponse, RequestEnvelope};
pub use error::RegistryError;
pub use route::{GatewayRoute, HttpMethod, RouteRegistry, RoutingContext};

pub mod routing;
pub use routing::RoutingStrategy;
