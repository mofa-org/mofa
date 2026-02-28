//! Feature-gated OTLP metrics exporter wiring.
//!
//! This placeholder keeps the server/config surface stable in PR1. The native
//! OpenTelemetry exporter implementation is added in the next PR.

use crate::{CardinalityLimits, MetricsCollector};
use std::sync::Arc;
use std::time::Duration;

/// OTLP metrics exporter configuration.
#[derive(Debug, Clone)]
pub struct OtlpMetricsExporterConfig {
    /// OTLP collector endpoint.
    pub endpoint: String,
    /// Snapshot sampling interval.
    pub collect_interval: Duration,
    /// Native OTLP export interval.
    pub export_interval: Duration,
    /// Max snapshots processed in a single worker tick.
    pub batch_size: usize,
    /// Max in-memory queue size.
    pub max_queue_size: usize,
    /// OTLP export timeout.
    pub timeout: Duration,
    /// Service name attribute.
    pub service_name: String,
    /// Cardinality guard settings.
    pub cardinality: CardinalityLimits,
}

impl Default for OtlpMetricsExporterConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:4318/v1/metrics".to_string(),
            collect_interval: Duration::from_secs(1),
            export_interval: Duration::from_secs(5),
            batch_size: 64,
            max_queue_size: 256,
            timeout: Duration::from_secs(3),
            service_name: "mofa-monitoring".to_string(),
            cardinality: CardinalityLimits::default(),
        }
    }
}

#[derive(Debug)]
pub struct OtlpExporterHandles {
    pub sampler: tokio::task::JoinHandle<()>,
    pub exporter: tokio::task::JoinHandle<()>,
}

/// OTLP metrics exporter placeholder. Full implementation lands in PR2.
pub struct OtlpMetricsExporter {
    _collector: Arc<MetricsCollector>,
    _config: OtlpMetricsExporterConfig,
}

impl OtlpMetricsExporter {
    pub fn new(collector: Arc<MetricsCollector>, config: OtlpMetricsExporterConfig) -> Self {
        Self {
            _collector: collector,
            _config: config,
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<OtlpExporterHandles, String> {
        let sampler = tokio::spawn(async move {});
        let exporter = tokio::spawn(async move {});
        Ok(OtlpExporterHandles { sampler, exporter })
    }
}
