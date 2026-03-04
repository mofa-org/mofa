#![cfg(feature = "otlp-metrics")]

use std::sync::Arc;
use std::time::Duration;

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

// macOS can panic inside system proxy discovery used by the OTLP HTTP stack in
// sandboxed CI/local environments ("Attempted to create a NULL object."). This
// behavior is outside exporter logic, so we run lifecycle coverage on non-macOS.
#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn otlp_exporter_start_returns_join_handles() {
    let collector = Arc::new(MetricsCollector::new(MetricsConfig::default()));
    let exporter = Arc::new(OtlpMetricsExporter::new(
        collector,
        OtlpMetricsExporterConfig::default(),
    ));

    let handles = exporter.start().await.expect("exporter should start");

    // Workers are long-running loops by design; verify they start, then stop them.
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(!handles.sampler.is_finished());
    assert!(!handles.exporter.is_finished());

    handles.sampler.abort();
    handles.exporter.abort();
    let sampler_result = handles.sampler.await;
    let exporter_result = handles.exporter.await;

    assert!(sampler_result.is_err());
    assert!(exporter_result.is_err());
}
