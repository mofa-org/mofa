//! Optional OTLP metrics exporter bridge.
//!
//! This exporter reuses `MetricsCollector` snapshots and pushes OTLP-like JSON
//! payloads to a collector endpoint on a periodic interval.

use crate::{CardinalityLimits, MetricsCollector, MetricsSnapshot};
use reqwest::Client;
use serde_json::json;
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::{debug, warn};

const OTHER_LABEL_VALUE: &str = "__other__";

/// OTLP metrics exporter configuration.
#[derive(Debug, Clone)]
pub struct OtlpMetricsExporterConfig {
    /// OTLP collector HTTP endpoint.
    pub endpoint: String,
    /// Snapshot sampling interval.
    pub collect_interval: Duration,
    /// Export flush interval.
    pub export_interval: Duration,
    /// Max snapshots per batch.
    pub batch_size: usize,
    /// Max in-memory queue size.
    pub max_queue_size: usize,
    /// HTTP timeout per export request.
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

/// Feature-gated OTLP metrics exporter.
pub struct OtlpMetricsExporter {
    collector: Arc<MetricsCollector>,
    config: OtlpMetricsExporterConfig,
    client: Client,
    sender: mpsc::Sender<MetricsSnapshot>,
    receiver: Mutex<Option<mpsc::Receiver<MetricsSnapshot>>>,
    dropped_snapshots: AtomicU64,
    last_error: Arc<RwLock<Option<String>>>,
}

impl OtlpMetricsExporter {
    pub fn new(collector: Arc<MetricsCollector>, mut config: OtlpMetricsExporterConfig) -> Self {
        if config.max_queue_size == 0 {
            warn!("OtlpMetricsExporterConfig.max_queue_size=0 is invalid; clamping to 1");
            config.max_queue_size = 1;
        }
        if config.batch_size == 0 {
            warn!("OtlpMetricsExporterConfig.batch_size=0 is invalid; clamping to 1");
            config.batch_size = 1;
        }

        let (sender, receiver) = mpsc::channel(config.max_queue_size);
        Self {
            collector,
            config,
            client: Client::new(),
            sender,
            receiver: Mutex::new(Some(receiver)),
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
    pub async fn start(self: Arc<Self>) -> Result<OtlpExporterHandles, String> {
        let Some(mut receiver) = self.receiver.lock().await.take() else {
            return Err("OTLP metrics exporter already started".to_string());
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
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(this.config.export_interval);
                let mut batch = Vec::with_capacity(this.config.batch_size);

                loop {
                    tokio::select! {
                        maybe_snapshot = receiver.recv() => {
                            match maybe_snapshot {
                                Some(snapshot) => {
                                    batch.push(snapshot);
                                    if batch.len() >= this.config.batch_size {
                                        flush_batch(&this, &mut batch).await;
                                    }
                                }
                                None => break,
                            }
                        }
                        _ = interval.tick() => {
                            if !batch.is_empty() {
                                flush_batch(&this, &mut batch).await;
                            }
                        }
                    }
                }

                if !batch.is_empty() {
                    flush_batch(&this, &mut batch).await;
                }
            })
        };

        Ok(OtlpExporterHandles { sampler, exporter })
    }

    async fn export_batch(&self, batch: &[MetricsSnapshot]) -> Result<(), String> {
        if batch.is_empty() {
            return Ok(());
        }

        let payload =
            build_otlp_payload(batch, &self.config.cardinality, &self.config.service_name);

        let response = self
            .client
            .post(&self.config.endpoint)
            .timeout(self.config.timeout)
            .json(&payload)
            .send()
            .await
            .map_err(|err| format!("failed to send OTLP metrics request: {err}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "OTLP metrics export failed with status {}",
                response.status()
            ));
        }

        Ok(())
    }
}

async fn flush_batch(exporter: &OtlpMetricsExporter, batch: &mut Vec<MetricsSnapshot>) {
    let payload = std::mem::take(batch);
    if let Err(err) = exporter.export_batch(&payload).await {
        warn!("otlp metrics export failed: {err}");
        *exporter.last_error.write().await = Some(err);
    } else {
        debug!(
            "exported {} snapshot(s) to OTLP metrics endpoint",
            payload.len()
        );
        *exporter.last_error.write().await = None;
    }
}

#[derive(Clone)]
struct LabeledPoint {
    labels: Vec<(String, String)>,
    rank: f64,
    value: f64,
}

fn build_otlp_payload(
    batch: &[MetricsSnapshot],
    limits: &CardinalityLimits,
    service_name: &str,
) -> serde_json::Value {
    if batch.is_empty() {
        return json!({"resourceMetrics": []});
    }

    let mut system_cpu_points = Vec::with_capacity(batch.len());
    let mut system_memory_points = Vec::with_capacity(batch.len());
    let mut agent_points = Vec::new();
    let mut workflow_points = Vec::new();
    let mut tool_points = Vec::new();
    let mut llm_points = Vec::new();

    for snapshot in batch {
        let ts_nanos = snapshot_time_nanos(snapshot);
        system_cpu_points.push(point(ts_nanos, vec![], snapshot.system.cpu_usage));
        system_memory_points.push(point(ts_nanos, vec![], snapshot.system.memory_used as f64));

        agent_points.extend(otlp_points(
            ts_nanos,
            cap_points(
                snapshot
                    .agents
                    .iter()
                    .map(|agent| LabeledPoint {
                        labels: vec![("agent_id".to_string(), agent.agent_id.clone())],
                        rank: agent.tasks_completed as f64,
                        value: agent.tasks_completed as f64,
                    })
                    .collect(),
                limits.agent_id,
            ),
        ));

        workflow_points.extend(otlp_points(
            ts_nanos,
            cap_points(
                snapshot
                    .workflows
                    .iter()
                    .map(|workflow| LabeledPoint {
                        labels: vec![("workflow_id".to_string(), workflow.workflow_id.clone())],
                        rank: workflow.total_executions as f64,
                        value: workflow.total_executions as f64,
                    })
                    .collect(),
                limits.workflow_id,
            ),
        ));

        tool_points.extend(otlp_points(
            ts_nanos,
            cap_points(
                snapshot
                    .plugins
                    .iter()
                    .map(|plugin| LabeledPoint {
                        labels: vec![("tool_name".to_string(), plugin.name.clone())],
                        rank: plugin.call_count as f64,
                        value: plugin.call_count as f64,
                    })
                    .collect(),
                limits.plugin_or_tool,
            ),
        ));

        llm_points.extend(otlp_points(
            ts_nanos,
            cap_points(
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
                limits.provider_model,
            ),
        ));
    }

    json!({
        "resourceMetrics": [{
            "resource": {
                "attributes": [
                    {
                        "key": "service.name",
                        "value": { "stringValue": service_name }
                    }
                ]
            },
            "scopeMetrics": [{
                "scope": { "name": "mofa-monitoring.metrics-exporter" },
                "metrics": [
                    metric_gauge("mofa.system.cpu.percent", system_cpu_points),
                    metric_gauge("mofa.system.memory.bytes", system_memory_points),
                    metric_gauge("mofa.agent.tasks.total", agent_points),
                    metric_gauge("mofa.workflow.executions.total", workflow_points),
                    metric_gauge("mofa.tool.calls.total", tool_points),
                    metric_gauge("mofa.llm.requests.total", llm_points),
                ]
            }]
        }]
    })
}

fn snapshot_time_nanos(snapshot: &MetricsSnapshot) -> u64 {
    if snapshot.timestamp > 0 {
        return snapshot.timestamp.saturating_mul(1_000_000_000);
    }

    let now_nanos_u128 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    u64::try_from(now_nanos_u128).unwrap_or(u64::MAX)
}

fn cap_points(mut points: Vec<LabeledPoint>, limit: usize) -> Vec<LabeledPoint> {
    if points.len() <= limit {
        points.sort_by(|a, b| compare_labels(&a.labels, &b.labels));
        return points;
    }

    points.sort_by(|a, b| {
        b.rank
            .partial_cmp(&a.rank)
            .unwrap_or(Ordering::Equal)
            .then_with(|| compare_labels(&a.labels, &b.labels))
    });

    let mut kept = points.drain(..limit).collect::<Vec<_>>();
    let overflow = points;

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
    kept
}

fn compare_labels(a: &[(String, String)], b: &[(String, String)]) -> Ordering {
    let a_key = a
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("|");
    let b_key = b
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("|");
    a_key.cmp(&b_key)
}

fn point(timestamp_unix_nano: u64, labels: Vec<(String, String)>, value: f64) -> serde_json::Value {
    let attributes = labels
        .into_iter()
        .map(|(k, v)| {
            json!({
                "key": k,
                "value": { "stringValue": v }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "timeUnixNano": timestamp_unix_nano,
        "asDouble": value,
        "attributes": attributes,
    })
}

fn otlp_points(timestamp_unix_nano: u64, points: Vec<LabeledPoint>) -> Vec<serde_json::Value> {
    points
        .into_iter()
        .map(|point_data| point(timestamp_unix_nano, point_data.labels, point_data.value))
        .collect()
}

fn metric_gauge(name: &str, data_points: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "name": name,
        "unit": "1",
        "gauge": {
            "dataPoints": data_points
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> MetricsSnapshot {
        MetricsSnapshot {
            system: crate::SystemMetrics {
                cpu_usage: 20.0,
                memory_used: 100,
                memory_total: 200,
                uptime_secs: 10,
                thread_count: 3,
                timestamp: 1,
            },
            agents: vec![crate::AgentMetrics {
                agent_id: "agent-1".to_string(),
                tasks_completed: 8,
                ..Default::default()
            }],
            workflows: vec![crate::WorkflowMetrics {
                workflow_id: "wf-1".to_string(),
                total_executions: 4,
                ..Default::default()
            }],
            plugins: vec![crate::PluginMetrics {
                name: "search".to_string(),
                call_count: 11,
                ..Default::default()
            }],
            llm_metrics: vec![crate::LLMMetrics {
                provider_name: "openai".to_string(),
                model_name: "gpt-4o-mini".to_string(),
                total_requests: 6,
                ..Default::default()
            }],
            timestamp: 1,
            custom: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn payload_contains_core_metrics() {
        let snapshot = sample_snapshot();
        let payload = build_otlp_payload(&[snapshot], &CardinalityLimits::default(), "mofa");

        let payload_str = payload.to_string();
        assert!(payload_str.contains("mofa.agent.tasks.total"));
        assert!(payload_str.contains("mofa.workflow.executions.total"));
        assert!(payload_str.contains("mofa.tool.calls.total"));
        assert!(payload_str.contains("mofa.llm.requests.total"));
    }

    #[test]
    fn payload_caps_cardinality_with_other_bucket() {
        let mut snapshot = sample_snapshot();
        snapshot.agents = (0..110)
            .map(|idx| crate::AgentMetrics {
                agent_id: format!("agent-{idx}"),
                tasks_completed: idx as u64,
                ..Default::default()
            })
            .collect();

        let limits = CardinalityLimits {
            agent_id: 3,
            ..Default::default()
        };
        let payload = build_otlp_payload(&[snapshot], &limits, "mofa");
        let payload_str = payload.to_string();

        assert!(payload_str.contains("__other__"));
    }

    #[test]
    fn payload_includes_all_batch_snapshots() {
        let mut first = sample_snapshot();
        first.timestamp = 1;
        first.system.cpu_usage = 10.0;
        let mut second = sample_snapshot();
        second.timestamp = 2;
        second.system.cpu_usage = 20.0;

        let payload = build_otlp_payload(&[first, second], &CardinalityLimits::default(), "mofa");
        let payload_str = payload.to_string();

        assert!(payload_str.contains("\"timeUnixNano\":1000000000"));
        assert!(payload_str.contains("\"timeUnixNano\":2000000000"));
    }

    // macOS CI/sandbox intermittently fails constructing network-backed HTTP client stacks
    // in this failure-path test; keep coverage on other targets where behavior is stable.
    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn export_failure_surfaces_error_without_panic() {
        let collector = Arc::new(MetricsCollector::new(Default::default()));

        let exporter = OtlpMetricsExporter::new(
            collector.clone(),
            OtlpMetricsExporterConfig::default().with_endpoint("http://127.0.0.1:1/v1/metrics"),
        );

        let result = exporter.export_batch(&[sample_snapshot()]).await;
        assert!(result.is_err());
    }
}
