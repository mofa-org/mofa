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
//! | [`GatewayRoute`] | A routing rule mapping a path + method to an agent |
//! | [`RouteRegistry`] | Trait for registering, looking up, and listing routes |
//! | [`RoutingContext`] | Per-request dispatch context (path, method, headers, correlation ID) |
//! | [`HttpMethod`] | HTTP method enum |
//! | [`RegistryError`] | Error type for registry operations |
//! | [`RequestEnvelope`] | Typed inbound request envelope flowing through the pipeline |
//! | [`GatewayResponse`] | Typed response for access logging, metrics, and admin API |

pub mod envelope;
pub mod error;
pub mod route;

#[cfg(test)]
mod tests;

pub use envelope::{GatewayResponse, RequestEnvelope};
pub use error::RegistryError;
pub use route::{GatewayRoute, HttpMethod, RouteRegistry, RoutingContext};
