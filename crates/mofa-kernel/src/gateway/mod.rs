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
//! | [`RouteConfig`] | Route definition used by [`GatewayRouter`] |
//! | [`GatewayRouter`] | Trait for path-based request routing |
//! | [`GatewayFilter`] | Trait for request/response filter pipeline |
//! | [`FilterOrder`] | Numeric ordering slot for filters |
//! | [`FilterAction`] | Continue / Reject / Redirect action from a filter |
//! | [`FilterChainConfig`] | Named ordered list of filter names |
//! | [`BackendKind`] | Classification of a backend service |
//! | [`BackendHealth`] | Last-known health state of a backend |
//! | [`CapabilityDescriptor`] | Full description of a registered backend |
//! | [`CapabilityRegistry`] | Trait for backend discovery and management |
//! | [`GatewayConfig`] | Top-level gateway configuration container |
//! | [`RateLimitConfig`] | Token-bucket rate-limit parameters |

pub mod error;
pub mod route;
mod config_error;
mod types;
mod router;
mod filter;
mod capability;
mod validation;

#[cfg(test)]
mod tests;

pub use error::RegistryError;
pub use route::{GatewayRoute, HttpMethod, RouteRegistry, RoutingContext};
pub use config_error::GatewayConfigError;
pub use types::{GatewayContext, GatewayRequest, GatewayResponse, RouteMatch};
pub use router::{GatewayRouter, RouteConfig};
pub use filter::{FilterAction, FilterChainConfig, FilterOrder, GatewayFilter};
pub use capability::{BackendHealth, BackendKind, CapabilityDescriptor, CapabilityRegistry};
pub use validation::{GatewayConfig, RateLimitConfig};
