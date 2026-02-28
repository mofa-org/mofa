//! Optional native OTLP metrics exporter.
//!
//! This exporter reuses `MetricsCollector` snapshots and records them through
//! OpenTelemetry metric instruments backed by the native OTLP exporter pipeline.

use crate::{CardinalityLimits, MetricsCollector, MetricsSnapshot};
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter, MeterProvider, UpDownCounter},
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, metrics::MeterProvider as SdkMeterProvider, runtime::Tokio};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, RwLock, mpsc};
use tracing::{debug, warn};

const OTHER_LABEL_VALUE: &str = "__other__";

/// OTLP metrics exporter configuration.
#[derive(Debug, Clone)]
#[non_exhaustive]
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

impl OtlpMetricsExporterConfig {
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn with_service_name(mut self, service_name: impl Into<String>) -> Self {
        self.service_name = service_name.into();
        self
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn with_max_queue_size(mut self, max_queue_size: usize) -> Self {
        self.max_queue_size = max_queue_size;
        self
    }
}

#[derive(Debug)]
pub struct OtlpExporterHandles {
    pub sampler: tokio::task::JoinHandle<()>,
    pub exporter: tokio::task::JoinHandle<()>,
}

/// Errors returned by OTLP metrics exporter lifecycle.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OtlpMetricsExporterError {
    #[error("OTLP metrics exporter already started")]
    AlreadyStarted,
    #[error("otlp metrics exporter internal error: {0}")]
    Internal(String),
}

/// Feature-gated OTLP metrics exporter.
pub struct OtlpMetricsExporter {
    collector: Arc<MetricsCollector>,
    config: OtlpMetricsExporterConfig,
    sender: mpsc::Sender<MetricsSnapshot>,
    receiver: AsyncMutex<Option<mpsc::Receiver<MetricsSnapshot>>>,
    dropped_snapshots: AtomicU64,
    last_error: Arc<RwLock<Option<String>>>,
}

impl OtlpMetricsExporter {
    pub fn new(collector: Arc<MetricsCollector>, mut config: OtlpMetricsExporterConfig) -> Self {
        if config.endpoint.trim().is_empty() {
            warn!("OtlpMetricsExporterConfig.endpoint is empty; using default endpoint");
            config.endpoint = OtlpMetricsExporterConfig::default().endpoint;
        }
        if config.max_queue_size == 0 {
            warn!("OtlpMetricsExporterConfig.max_queue_size=0 is invalid; clamping to 1");
            config.max_queue_size = 1;
        }
        if config.batch_size == 0 {
            warn!("OtlpMetricsExporterConfig.batch_size=0 is invalid; clamping to 1");
            config.batch_size = 1;
        }
        if config.collect_interval.is_zero() {
            warn!("OtlpMetricsExporterConfig.collect_interval=0 is invalid; clamping to 1s");
            config.collect_interval = Duration::from_secs(1);
        }
        if config.export_interval.is_zero() {
            warn!("OtlpMetricsExporterConfig.export_interval=0 is invalid; clamping to 1s");
            config.export_interval = Duration::from_secs(1);
        }
        if config.timeout.is_zero() {
            warn!("OtlpMetricsExporterConfig.timeout=0 is invalid; clamping to 1s");
            config.timeout = Duration::from_secs(1);
        }

        let (sender, receiver) = mpsc::channel(config.max_queue_size);
        Self {
            collector,
            config,
            sender,
            receiver: AsyncMutex::new(Some(receiver)),
            dropped_snapshots: AtomicU64::new(0),
            last_error: Arc::new(RwLock::new(None)),
        }
    }

    pub fn dropped_snapshots(&self) -> u64 {
        self.dropped_snapshots.load(AtomicOrdering::Relaxed)
    }

    pub async fn last_error(&self) -> Option<String> {
        self.last_error.read().await.clone()
    }

    /// Start sampler and exporter workers.
    pub async fn start(self: Arc<Self>) -> Result<OtlpExporterHandles, OtlpMetricsExporterError> {
        let Some(mut receiver) = self.receiver.lock().await.take() else {
            let err = OtlpMetricsExporterError::AlreadyStarted;
            *self.last_error.write().await = Some(err.to_string());
            return Err(err);
        };

        let recorder = match OtlpRecorder::new(&self.config) {
            Ok(recorder) => Arc::new(recorder),
            Err(err) => {
                *self.last_error.write().await = Some(err.to_string());
                return Err(err);
            }
        };

        let sampler = {
            let this = self.clone();
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(this.config.collect_interval);
                loop {
                    ticker.tick().await;
                    let snapshot = this.collector.current().await;
                    if let Err(err) = this.sender.try_send(snapshot) {
                        match err {
                            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                                this.dropped_snapshots.fetch_add(1, AtomicOrdering::Relaxed);
                            }
                            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                                warn!("otlp sampler queue closed");
                                break;
                            }
                        }
                    }
                }
            })
        };

        let exporter = {
            let this = self.clone();
            let recorder = recorder.clone();
            tokio::spawn(async move {
                let mut batch = Vec::with_capacity(this.config.batch_size);

                while let Some(snapshot) = receiver.recv().await {
                    batch.push(snapshot);
                    while batch.len() < this.config.batch_size {
                        match receiver.try_recv() {
                            Ok(snapshot) => batch.push(snapshot),
                            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                        }
                    }

                    flush_batch(&this, &recorder, &mut batch).await;
                }

                if !batch.is_empty() {
                    flush_batch(&this, &recorder, &mut batch).await;
                }
            })
        };

        Ok(OtlpExporterHandles { sampler, exporter })
    }
}

struct OtlpRecorder {
    // Keep provider alive for background periodic export.
    _meter_provider: SdkMeterProvider,
    instruments: OtlpInstruments,
    cardinality: CardinalityLimits,
    last_values: StdMutex<LastSeriesState>,
}

impl OtlpRecorder {
    fn new(config: &OtlpMetricsExporterConfig) -> Result<Self, OtlpMetricsExporterError> {
        let exporter = opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(config.endpoint.clone())
            .with_timeout(config.timeout);

        let meter_provider = opentelemetry_otlp::new_pipeline()
            .metrics(Tokio)
            .with_exporter(exporter)
            .with_period(config.export_interval)
            .with_timeout(config.timeout)
            .with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                config.service_name.clone(),
            )]))
            .build()
            .map_err(|err| {
                OtlpMetricsExporterError::Internal(format!(
                    "failed to build native OTLP meter provider: {err}"
                ))
            })?;

        let meter = meter_provider.meter("mofa-monitoring.metrics-exporter");
        let instruments = OtlpInstruments::new(&meter);

        Ok(Self {
            _meter_provider: meter_provider,
            instruments,
            cardinality: config.cardinality.clone(),
            last_values: StdMutex::new(LastSeriesState::default()),
        })
    }

    fn record_snapshot(&self, snapshot: &MetricsSnapshot) {
        let mut dropped = DroppedSeriesCounters::default();
        let mut state = self
            .last_values
            .lock()
            .expect("otlp metrics exporter state mutex poisoned");

        self.apply_labeled_values(
            &self.instruments.system_cpu_percent,
            vec![LabeledPoint {
                labels: vec![],
                rank: snapshot.system.cpu_usage,
                value: snapshot.system.cpu_usage,
            }],
            &mut state.system_cpu_percent,
        );

        self.apply_labeled_values(
            &self.instruments.system_memory_bytes,
            vec![LabeledPoint {
                labels: vec![],
                rank: snapshot.system.memory_used as f64,
                value: snapshot.system.memory_used as f64,
            }],
            &mut state.system_memory_bytes,
        );

        let (agent_values, dropped_agents) = cap_points(
            snapshot
                .agents
                .iter()
                .map(|agent| LabeledPoint {
                    labels: vec![("agent_id".to_string(), agent.agent_id.clone())],
                    rank: agent.tasks_completed as f64,
                    value: agent.tasks_completed as f64,
                })
                .collect(),
            self.cardinality.agent_id,
        );
        dropped.agent_id = dropped_agents;
        self.apply_labeled_values(
            &self.instruments.agent_tasks_total,
            agent_values,
            &mut state.agent_tasks_total,
        );

        let (workflow_values, dropped_workflows) = cap_points(
            snapshot
                .workflows
                .iter()
                .map(|workflow| LabeledPoint {
                    labels: vec![("workflow_id".to_string(), workflow.workflow_id.clone())],
                    rank: workflow.total_executions as f64,
                    value: workflow.total_executions as f64,
                })
                .collect(),
            self.cardinality.workflow_id,
        );
        dropped.workflow_id = dropped_workflows;
        self.apply_labeled_values(
            &self.instruments.workflow_executions_total,
            workflow_values,
            &mut state.workflow_executions_total,
        );

        let (tool_values, dropped_tools) = cap_points(
            snapshot
                .plugins
                .iter()
                .map(|plugin| LabeledPoint {
                    labels: vec![("tool_name".to_string(), plugin.name.clone())],
                    rank: plugin.call_count as f64,
                    value: plugin.call_count as f64,
                })
                .collect(),
            self.cardinality.plugin_or_tool,
        );
        dropped.plugin_or_tool = dropped_tools;
        self.apply_labeled_values(
            &self.instruments.tool_call_count,
            tool_values,
            &mut state.tool_call_count,
        );

        let (llm_values, dropped_provider_model) = cap_points(
            snapshot
                .llm_metrics
                .iter()
                .map(|llm| LabeledPoint {
                    labels: vec![
                        ("provider".to_string(), llm.provider_name.clone()),
                        ("model".to_string(), llm.model_name.clone()),
                    ],
                    rank: llm.total_requests as f64,
                    value: llm.total_requests as f64,
                })
                .collect(),
            self.cardinality.provider_model,
        );
        dropped.provider_model = dropped_provider_model;
        self.apply_labeled_values(
            &self.instruments.llm_requests_total,
            llm_values,
            &mut state.llm_requests_total,
        );

        self.record_dropped_series(dropped);
    }

    fn apply_labeled_values(
        &self,
        instrument: &UpDownCounter<f64>,
        values: Vec<LabeledPoint>,
        state: &mut HashMap<String, SeriesValue>,
    ) {
        let mut next = HashMap::with_capacity(values.len());

        for value in values {
            let key = label_key(&value.labels);
            next.insert(
                key,
                SeriesValue {
                    labels: value.labels,
                    value: value.value,
                },
            );
        }

        for (key, series) in &next {
            let prev = state.get(key).map(|old| old.value).unwrap_or(0.0);
            let delta = series.value - prev;
            if is_non_zero(delta) {
                let attrs = to_attributes(&series.labels);
                instrument.add(delta, &attrs);
            }
        }

        for (key, series) in state.iter() {
            if !next.contains_key(key) && is_non_zero(series.value) {
                let attrs = to_attributes(&series.labels);
                instrument.add(-series.value, &attrs);
            }
        }

        *state = next;
    }

    fn record_dropped_series(&self, dropped: DroppedSeriesCounters) {
        if dropped.agent_id > 0 {
            self.instruments.dropped_series_total.add(
                dropped.agent_id as u64,
                &[KeyValue::new("label", "agent_id")],
            );
        }
        if dropped.workflow_id > 0 {
            self.instruments.dropped_series_total.add(
                dropped.workflow_id as u64,
                &[KeyValue::new("label", "workflow_id")],
            );
        }
        if dropped.plugin_or_tool > 0 {
            self.instruments.dropped_series_total.add(
                dropped.plugin_or_tool as u64,
                &[KeyValue::new("label", "plugin_or_tool")],
            );
        }
        if dropped.provider_model > 0 {
            self.instruments.dropped_series_total.add(
                dropped.provider_model as u64,
                &[KeyValue::new("label", "provider_model")],
            );
        }
    }
}

struct OtlpInstruments {
    system_cpu_percent: UpDownCounter<f64>,
    system_memory_bytes: UpDownCounter<f64>,
    agent_tasks_total: UpDownCounter<f64>,
    workflow_executions_total: UpDownCounter<f64>,
    tool_call_count: UpDownCounter<f64>,
    llm_requests_total: UpDownCounter<f64>,
    dropped_series_total: Counter<u64>,
}

impl OtlpInstruments {
    fn new(meter: &Meter) -> Self {
        Self {
            system_cpu_percent: meter
                .f64_up_down_counter("mofa.system.cpu.percent")
                .with_description("System CPU usage percentage")
                .init(),
            system_memory_bytes: meter
                .f64_up_down_counter("mofa.system.memory.bytes")
                .with_description("System memory usage in bytes")
                .init(),
            agent_tasks_total: meter
                .f64_up_down_counter("mofa.agent.tasks.total")
                .with_description("Total tasks completed by agent")
                .init(),
            workflow_executions_total: meter
                .f64_up_down_counter("mofa.workflow.executions.total")
                .with_description("Total workflow executions")
                .init(),
            tool_call_count: meter
                .f64_up_down_counter("mofa.tool.calls.total")
                .with_description("Total tool or plugin call count")
                .init(),
            llm_requests_total: meter
                .f64_up_down_counter("mofa.llm.requests.total")
                .with_description("Total LLM requests")
                .init(),
            dropped_series_total: meter
                .u64_counter("mofa.exporter.dropped_series.total")
                .with_description("Total dropped metric series due to cardinality limits")
                .init(),
        }
    }
}

#[derive(Default)]
struct LastSeriesState {
    system_cpu_percent: HashMap<String, SeriesValue>,
    system_memory_bytes: HashMap<String, SeriesValue>,
    agent_tasks_total: HashMap<String, SeriesValue>,
    workflow_executions_total: HashMap<String, SeriesValue>,
    tool_call_count: HashMap<String, SeriesValue>,
    llm_requests_total: HashMap<String, SeriesValue>,
}

#[derive(Clone)]
struct SeriesValue {
    labels: Vec<(String, String)>,
    value: f64,
}

async fn flush_batch(
    exporter: &OtlpMetricsExporter,
    recorder: &OtlpRecorder,
    batch: &mut Vec<MetricsSnapshot>,
) {
    let snapshots = std::mem::take(batch);
    for snapshot in snapshots {
        recorder.record_snapshot(&snapshot);
    }

    debug!("recorded snapshot batch into native OTLP meter provider");
    *exporter.last_error.write().await = None;
}

#[derive(Default, Debug, Clone)]
struct DroppedSeriesCounters {
    agent_id: usize,
    workflow_id: usize,
    plugin_or_tool: usize,
    provider_model: usize,
}

#[derive(Clone)]
struct LabeledPoint {
    labels: Vec<(String, String)>,
    rank: f64,
    value: f64,
}

fn to_attributes(labels: &[(String, String)]) -> Vec<KeyValue> {
    labels
        .iter()
        .map(|(k, v)| KeyValue::new(k.clone(), v.clone()))
        .collect()
}

fn cap_points(mut points: Vec<LabeledPoint>, limit: usize) -> (Vec<LabeledPoint>, usize) {
    if points.len() <= limit {
        points.sort_by(|a, b| compare_labels(&a.labels, &b.labels));
        return (points, 0);
    }

    points.sort_by(|a, b| {
        b.rank
            .partial_cmp(&a.rank)
            .unwrap_or(Ordering::Equal)
            .then_with(|| compare_labels(&a.labels, &b.labels))
    });

    let mut kept = points.drain(..limit.min(points.len())).collect::<Vec<_>>();
    let overflow = points;
    let dropped_count = overflow.len();

    let overflow_value = overflow
        .into_iter()
        .map(|entry| entry.value)
        .fold(0.0, |acc, v| acc + v);

    let labels = if let Some(first) = kept.first() {
        first
            .labels
            .iter()
            .map(|(k, _)| (k.clone(), OTHER_LABEL_VALUE.to_string()))
            .collect::<Vec<_>>()
    } else {
        vec![("label".to_string(), OTHER_LABEL_VALUE.to_string())]
    };

    kept.push(LabeledPoint {
        labels,
        rank: overflow_value,
        value: overflow_value,
    });
    kept.sort_by(|a, b| compare_labels(&a.labels, &b.labels));
    (kept, dropped_count)
}

fn compare_labels(a: &[(String, String)], b: &[(String, String)]) -> Ordering {
    label_key(a).cmp(&label_key(b))
}

fn label_key(labels: &[(String, String)]) -> String {
    let mut key = String::new();
    for (k, v) in labels {
        let _ = write!(&mut key, "{}:{}:{}:{};", k.len(), k, v.len(), v);
    }
    key
}

fn is_non_zero(value: f64) -> bool {
    value.abs() > f64::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_points_adds_other_bucket_when_limit_exceeded() {
        let points = vec![
            LabeledPoint {
                labels: vec![("agent_id".to_string(), "a".to_string())],
                rank: 10.0,
                value: 10.0,
            },
            LabeledPoint {
                labels: vec![("agent_id".to_string(), "b".to_string())],
                rank: 8.0,
                value: 8.0,
            },
            LabeledPoint {
                labels: vec![("agent_id".to_string(), "c".to_string())],
                rank: 5.0,
                value: 5.0,
            },
        ];

        let (capped, dropped) = cap_points(points, 2);
        assert_eq!(dropped, 1);
        assert_eq!(capped.len(), 3);

        let has_other = capped.iter().any(|entry| {
            entry
                .labels
                .iter()
                .any(|(k, v)| k == "agent_id" && v == "__other__")
        });
        assert!(has_other);
    }

    #[test]
    fn cap_points_keeps_deterministic_order() {
        let points = vec![
            LabeledPoint {
                labels: vec![("k".to_string(), "b".to_string())],
                rank: 1.0,
                value: 1.0,
            },
            LabeledPoint {
                labels: vec![("k".to_string(), "a".to_string())],
                rank: 1.0,
                value: 1.0,
            },
        ];

        let (capped, dropped) = cap_points(points, 10);
        assert_eq!(dropped, 0);
        assert_eq!(capped.len(), 2);
        assert_eq!(capped[0].labels[0].1, "a");
        assert_eq!(capped[1].labels[0].1, "b");
    }

    #[test]
    fn config_is_hardened_for_invalid_inputs() {
        let collector = Arc::new(MetricsCollector::new(Default::default()));
        let exporter = OtlpMetricsExporter::new(
            collector,
            OtlpMetricsExporterConfig {
                endpoint: "   ".to_string(),
                collect_interval: Duration::ZERO,
                export_interval: Duration::ZERO,
                batch_size: 0,
                max_queue_size: 0,
                timeout: Duration::ZERO,
                ..Default::default()
            },
        );

        assert_eq!(exporter.config.endpoint, "http://127.0.0.1:4318/v1/metrics");
        assert_eq!(exporter.config.collect_interval, Duration::from_secs(1));
        assert_eq!(exporter.config.export_interval, Duration::from_secs(1));
        assert_eq!(exporter.config.timeout, Duration::from_secs(1));
        assert_eq!(exporter.config.batch_size, 1);
        assert_eq!(exporter.config.max_queue_size, 1);
    }

    #[test]
    fn label_key_uses_collision_safe_encoding() {
        let a = vec![
            ("k|1".to_string(), "v=1".to_string()),
            ("x".to_string(), "y".to_string()),
        ];
        let b = vec![
            ("k".to_string(), "1|v=1".to_string()),
            ("x".to_string(), "y".to_string()),
        ];
        assert_ne!(label_key(&a), label_key(&b));
    }

    #[tokio::test]
    async fn start_reports_already_started_error_and_sets_last_error() {
        let collector = Arc::new(MetricsCollector::new(Default::default()));
        let exporter = Arc::new(OtlpMetricsExporter::new(collector, Default::default()));

        let _ = exporter.receiver.lock().await.take();
        let err = exporter
            .clone()
            .start()
            .await
            .expect_err("second start should fail");
        assert!(matches!(err, OtlpMetricsExporterError::AlreadyStarted));
        let expected = err.to_string();
        assert_eq!(
            exporter.last_error().await.as_deref(),
            Some(expected.as_str())
        );
    }
}
