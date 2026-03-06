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
//! | [`RouteDeadline`] | Per-route deadline policy (request, connect, idle timeouts) |
//! | [`RouteRegistry`] | Trait for registering, looking up, and listing routes |
//! | [`RoutingContext`] | Per-request dispatch context (path, method, headers, correlation ID) |
//! | [`HttpMethod`] | HTTP method enum |
//! | [`RegistryError`] | Error type for registry operations |
//! | [`RequestEnvelope`] | Admitted request with computed deadline `Instant` |
//! | [`GatewayResponse`] | Typed gateway response (status + JSON body) |

pub mod envelope;
pub mod error;
pub mod route;

#[cfg(test)]
mod tests;

pub use error::RegistryError;
pub use route::{GatewayRoute, HttpMethod, RouteDeadline, RouteRegistry, RoutingContext};
pub use envelope::{GatewayResponse, RequestEnvelope};
