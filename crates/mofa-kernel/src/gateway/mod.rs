//! Kernel-level gateway abstractions for agent request dispatch.
//!
//! This module defines the trait boundary between the gateway transport layer
//! (HTTP, gRPC, MQTT, …) and the agent runtime.  By keeping routing logic in
//! the kernel, alternative transports and unit tests can reason about routing
//! without depending on the full HTTP stack.
//!
//! # Types
//!
//! | Type | Description |
//! |------|-------------|
//! | [`GatewayRoute`] | A routing rule mapping a path + method to an agent |
//! | [`RouteRegistry`] | Trait for registering, looking up, and listing routes |
//! | [`RoutingContext`] | Per-request dispatch context (path, method, headers, correlation ID) |
//! | [`HttpMethod`] | HTTP method enum |
//! | [`RegistryError`] | Error type for registry operations |
//! | [`GatewayConfigError`] | Error type for gateway configuration validation |
//! | [`GatewayRequest`] | Inbound HTTP request model |
//! | [`GatewayResponse`] | Outbound HTTP response model |
//! | [`GatewayContext`] | Per-request mutable context for filter chains |
//! | [`RouteMatch`] | Result of a successful route lookup |

pub mod error;
pub mod route;
mod config_error;
mod types;

#[cfg(test)]
mod tests;

pub use error::RegistryError;
pub use route::{GatewayRoute, HttpMethod, RouteRegistry, RoutingContext};
pub use config_error::GatewayConfigError;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, RouteMatch};
