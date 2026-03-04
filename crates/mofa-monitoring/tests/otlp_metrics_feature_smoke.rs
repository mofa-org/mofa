#![cfg(feature = "otlp-metrics")]

use std::sync::Arc;

use mofa_monitoring::{
    MetricsCollector, MetricsConfig,
    tracing::{OtlpMetricsExporter, OtlpMetricsExporterConfig},
};

#[test]
fn otlp_config_defaults_are_safe() {
    let config = OtlpMetricsExporterConfig::default();

    assert!(!config.endpoint.is_empty());
    assert!(!config.collect_interval.is_zero());
    assert!(!config.export_interval.is_zero());
    assert!(!config.timeout.is_zero());
    assert!(config.batch_size > 0);
    assert!(config.max_queue_size > 0);
}

#[tokio::test]
async fn otlp_exporter_start_returns_join_handles() {
    let collector = Arc::new(MetricsCollector::new(MetricsConfig::default()));
    let exporter = Arc::new(OtlpMetricsExporter::new(
        collector,
        OtlpMetricsExporterConfig::default(),
    ));

    let handles = exporter.start().await.expect("exporter should start");
    let sampler_result = handles.sampler.await;
    let exporter_result = handles.exporter.await;

    assert!(sampler_result.is_ok());
    assert!(exporter_result.is_ok());
}
