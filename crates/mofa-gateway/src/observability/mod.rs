//! Observability module for metrics and tracing.
//!
//! This module provides:
//! - Prometheus metrics collection
//! - OpenTelemetry distributed tracing
//! - Structured logging integration
//!
//! # Features
//!
//! Enable the `monitoring` feature to use OpenTelemetry tracing:
//! ```toml
//! [dependencies]
//! mofa-gateway = { version = "0.1", features = ["monitoring"] }
//! ```

pub mod metrics;
pub mod tracing;

pub use metrics::*;
pub use tracing::*;
