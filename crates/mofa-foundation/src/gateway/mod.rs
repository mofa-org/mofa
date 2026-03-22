//! Foundation-layer gateway implementations.
//!
//! This module contains concrete implementations of the kernel-level gateway
//! traits. Kernel traits live in `mofa-kernel::gateway`; implementations live
//! here so the kernel stays free of runtime dependencies.

pub mod capability;
pub mod http_capabilities;
pub mod registry_builder;
pub mod rate_limiter;

pub use capability::{
    CapabilityRequest, CapabilityResponse, GatewayCapability, GatewayCapabilityRegistry,
};
pub use http_capabilities::{
    DuckDuckGoSearchCapability, HttpFetchCapability, ReadSensorCapability,
    WebhookNotificationCapability,
};
pub use registry_builder::{
    GatewayCapabilityRegistryConfig, built_in_capability_registry_from_env,
};
pub use rate_limiter::TokenBucketRateLimiter;
