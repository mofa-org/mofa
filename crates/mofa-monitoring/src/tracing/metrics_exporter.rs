#[cfg(feature = "otlp-metrics")]
use opentelemetry::{
    metrics::{Meter, MeterProvider as _, Unit},
    KeyValue,
};
#[cfg(feature = "otlp-metrics")]
use opentelemetry_otlp::{WithExportConfig};
#[cfg(feature = "otlp-metrics")]
use opentelemetry_sdk::{
    metrics::{PeriodicReader, SdkMeterProvider},
    runtime, Resource,
};

#[cfg(feature = "otlp-metrics")]
use crate::dashboard::{AgentMetrics, LLMMetrics, MetricsSnapshot, WorkflowMetrics};

#[cfg(not(feature = "otlp-metrics"))]
use crate::MetricsCollector;

#[cfg(feature = "otlp-metrics")]
use crate::MetricsCollector;

use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Duration;
use tracing::{debug, error, info, warn};

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

/// Cardinality guard configuration for OTLP label dimensions.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CardinalityLimits {
    pub agent_id: usize,
    pub workflow_id: usize,
    pub plugin_or_tool: usize,
    pub provider_model: usize,
}

impl Default for CardinalityLimits {
    fn default() -> Self {
        Self {
            agent_id: 100,
            workflow_id: 100,
            plugin_or_tool: 100,
            provider_model: 50,
        }
    }
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

/// OTLP metrics exporter.
///
/// Samples snapshots from `MetricsCollector` and exports them via OTLP.
pub struct OtlpMetricsExporter {
    collector: Arc<MetricsCollector>,
    config: OtlpMetricsExporterConfig,
    #[cfg(feature = "otlp-metrics")]
    state_cache: Arc<StdRwLock<MetricsSnapshot>>,
}

impl OtlpMetricsExporter {
    pub fn new(collector: Arc<MetricsCollector>, config: OtlpMetricsExporterConfig) -> Self {
        Self {
            collector,
            config,
            #[cfg(feature = "otlp-metrics")]
            state_cache: Arc::new(StdRwLock::new(MetricsSnapshot::default())),
        }
    }

    #[cfg(feature = "otlp-metrics")]
    pub async fn start(self: Arc<Self>) -> Result<OtlpExporterHandles, String> {
        info!(
            "Starting OTLP metrics exporter for service '{}' to {}",
            self.config.service_name, self.config.endpoint
        );

        let exporter = opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(&self.config.endpoint)
            .with_timeout(self.config.timeout);

        let provider = SdkMeterProvider::builder()
            .with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                self.config.service_name.clone(),
            )]))
            .with_reader(
                PeriodicReader::builder(exporter, runtime::Tokio)
                    .with_interval(self.config.export_interval)
                    .build(),
            )
            .build();

        let meter = provider.meter("mofa-monitoring");
        
        // Register observable gauges once
        self.register_instruments(&meter);

        // Spawn sampling task to update the sync cache
        let collector = self.collector.clone();
        let cache = self.state_cache.clone();
        let collect_interval = self.config.collect_interval;

        let sampler = tokio::spawn(async move {
            let mut interval = tokio::time::interval(collect_interval);
            loop {
                interval.tick().await;
                let snapshot = collector.current().await;
                if let Ok(mut cache_guard) = cache.write() {
                    *cache_guard = snapshot;
                }
            }
        });

        let exporter_task = tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            if let Err(e) = provider.shutdown() {
                error!("Error shutting down OTLP exporter: {}", e);
            }
        });

        Ok(OtlpExporterHandles {
            sampler,
            exporter: exporter_task,
        })
    }

    #[cfg(feature = "otlp-metrics")]
    fn register_instruments(&self, meter: &Meter) {
        let cache = self.state_cache.clone();
        let limits = self.config.cardinality.clone();

        // System Gauges
        let sys_cache = cache.clone();
        meter.f64_observable_gauge("system.cpu_usage")
            .with_description("CPU usage percentage")
            .with_unit(Unit::new("1"))
            .with_callback(move |observer| {
                if let Ok(snapshot) = sys_cache.read() {
                    observer.observe(snapshot.system.cpu_usage, &[]);
                }
            })
            .build();

        let mem_cache = cache.clone();
        meter.u64_observable_gauge("system.memory_used")
            .with_description("Memory usage in bytes")
            .with_unit(Unit::new("By"))
            .with_callback(move |observer| {
                if let Ok(snapshot) = mem_cache.read() {
                    observer.observe(snapshot.system.memory_used, &[]);
                }
            })
            .build();

        // Agent Gauges
        let agent_cache = cache.clone();
        let agent_limits = limits.clone();
        meter.u64_observable_gauge("agent.tasks_completed")
            .with_description("Total tasks completed by agent")
            .with_callback(move |observer| {
                if let Ok(snapshot) = agent_cache.read() {
                    for (i, agent) in snapshot.agents.iter().enumerate() {
                        if i >= agent_limits.agent_id { break; }
                        let labels = [
                            KeyValue::new("agent_id", agent.agent_id.clone()),
                            KeyValue::new("agent_name", agent.name.clone()),
                            KeyValue::new("state", agent.state.clone()),
                        ];
                        observer.observe(agent.tasks_completed, &labels);
                    }
                }
            })
            .build();

        // LLM Gauges
        let llm_cache = cache.clone();
        let llm_limits = limits.clone();
        meter.u64_observable_gauge("llm.total_tokens")
            .with_description("Total tokens processed by LLM plugin")
            .with_callback(move |observer| {
                if let Ok(snapshot) = llm_cache.read() {
                    for (i, llm) in snapshot.llm_metrics.iter().enumerate() {
                        if i >= llm_limits.provider_model { break; }
                        let labels = [
                            KeyValue::new("plugin_id", llm.plugin_id.clone()),
                            KeyValue::new("provider", llm.provider_name.clone()),
                            KeyValue::new("model", llm.model_name.clone()),
                        ];
                        observer.observe(llm.total_tokens, &labels);
                    }
                }
            })
            .build();
    }

    #[cfg(not(feature = "otlp-metrics"))]
    pub async fn start(self: Arc<Self>) -> Result<OtlpExporterHandles, String> {
        warn!("OTLP metrics exporter requested but 'otlp-metrics' feature is disabled");
        let sampler = tokio::spawn(async move {});
        let exporter = tokio::spawn(async move {});
        Ok(OtlpExporterHandles { sampler, exporter })
    }
}

