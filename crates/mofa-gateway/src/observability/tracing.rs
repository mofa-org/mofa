//! OpenTelemetry distributed tracing integration.
//!
//! This module provides OpenTelemetry tracing support for distributed request
//! tracing across the gateway and control plane.
//!
//! # Usage
//!
//! Enable the `monitoring` feature and initialize tracing:
//!
//! ```rust,ignore
//! use mofa_gateway::observability::init_tracing;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     init_tracing("my-gateway", "http://localhost:4317")?;
//!     Ok(())
//! }
//! ```

#[cfg(feature = "monitoring")]
use opentelemetry::global;
#[cfg(feature = "monitoring")]
use opentelemetry::trace::TracerProvider as _;
#[cfg(feature = "monitoring")]
use opentelemetry_sdk::trace::TracerProvider;
#[cfg(feature = "monitoring")]
use opentelemetry_sdk::Resource;
#[cfg(feature = "monitoring")]
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
#[cfg(feature = "monitoring")]
use tracing_opentelemetry::OpenTelemetryLayer;
#[cfg(feature = "monitoring")]
use tracing_subscriber::layer::SubscriberExt;
#[cfg(feature = "monitoring")]
use tracing_subscriber::Registry;

use crate::error::GatewayResult;
use tracing::{error, info};

/// Initialize OpenTelemetry tracing.
///
/// # Arguments
///
/// * `service_name` - Name of the service (e.g., "mofa-gateway")
/// * `otlp_endpoint` - OTLP endpoint URL (e.g., "http://localhost:4317")
///
/// # Returns
///
/// Returns an error if initialization fails.
#[cfg(feature = "monitoring")]
pub fn init_tracing(service_name: &str, otlp_endpoint: &str) -> GatewayResult<()> {
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::BatchSpanProcessor;

    info!("Initializing OpenTelemetry tracing for service: {}", service_name);

    let service_name_owned = service_name.to_string();

    // Create OTLP exporter
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .build()
        .map_err(|e| crate::error::GatewayError::Internal(format!("Failed to create OTLP exporter: {}", e)))?;

    // Create batch span processor
    let span_processor = BatchSpanProcessor::builder(exporter, opentelemetry_sdk::runtime::Tokio)
        .build();

    // Create tracer provider
    let tracer_provider = TracerProvider::builder()
        .with_span_processor(span_processor)
        .with_resource(Resource::new(vec![opentelemetry::KeyValue::new(SERVICE_NAME, service_name_owned.clone())]))
        .build();

    // Get tracer before setting global provider (to get concrete type)
    let tracer = tracer_provider.tracer(service_name_owned);

    // Set global tracer provider
    global::set_tracer_provider(tracer_provider);

    // Create OpenTelemetry layer with concrete tracer type
    let telemetry_layer = OpenTelemetryLayer::new(tracer);

    // Initialize tracing subscriber with OpenTelemetry layer
    let subscriber = Registry::default().with(telemetry_layer);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| crate::error::GatewayError::Internal(format!("Failed to set tracing subscriber: {}", e)))?;

    info!("OpenTelemetry tracing initialized successfully");
    Ok(())
}

/// Initialize basic tracing without OpenTelemetry (fallback).
///
/// This is used when the `monitoring` feature is not enabled.
pub fn init_basic_tracing(service_name: &str) -> GatewayResult<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("{}={}", service_name, tracing::Level::INFO).parse().unwrap_or_else(|_| {
                    tracing::Level::INFO.into()
                })),
        )
        .init();

    info!("Basic tracing initialized for service: {}", service_name);
    Ok(())
}

/// Shutdown OpenTelemetry tracing.
#[cfg(feature = "monitoring")]
pub fn shutdown_tracing() {
    global::shutdown_tracer_provider();
}

/// Shutdown tracing (no-op when monitoring feature is disabled).
#[cfg(not(feature = "monitoring"))]
pub fn shutdown_tracing() {
    // No-op when monitoring feature is disabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tracing_init() {
        // This test just verifies the function doesn't panic
        // In a real test, we'd need to handle the global subscriber
        let _ = init_basic_tracing("test-service");
    }
}
