//! Prometheus metrics export bridge for dashboard metrics.

use super::metrics::{MetricValue, MetricsCollector, MetricsSnapshot};
use axum::body::Bytes;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::warn;

/// Prometheus export configuration.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PrometheusExportConfig {
    /// Refresh interval for the background cache worker.
    pub refresh_interval: Duration,
}

impl Default for PrometheusExportConfig {
    fn default() -> Self {
        Self {
            refresh_interval: Duration::from_secs(1),
        }
    }
}

impl PrometheusExportConfig {
    pub fn with_refresh_interval(mut self, refresh_interval: Duration) -> Self {
        if refresh_interval.is_zero() {
            warn!(
                "PrometheusExportConfig::with_refresh_interval received zero duration; clamping to 1ms"
            );
            self.refresh_interval = Duration::from_millis(1);
        } else {
            self.refresh_interval = refresh_interval;
        }
        self
    }
}

/// Errors returned by the Prometheus exporter lifecycle.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PrometheusExportError {
    #[error("prometheus exporter internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
struct HistogramSample {
    count: u64,
    sum: f64,
    bucket_counts: Vec<u64>,
}

#[derive(Debug)]
struct DurationHistogram {
    bounds: Vec<f64>,
    sample: HistogramSample,
}

impl DurationHistogram {
    fn new(bounds: Vec<f64>) -> Self {
        Self {
            sample: HistogramSample {
                count: 0,
                sum: 0.0,
                bucket_counts: vec![0; bounds.len()],
            },
            bounds,
        }
    }

    fn observe(&mut self, value_seconds: f64) {
        if !value_seconds.is_finite() || value_seconds < 0.0 {
            return;
        }

        self.sample.count = self.sample.count.saturating_add(1);
        self.sample.sum += value_seconds;

        for (idx, bound) in self.bounds.iter().enumerate() {
            if value_seconds <= *bound {
                self.sample.bucket_counts[idx] = self.sample.bucket_counts[idx].saturating_add(1);
            }
        }
    }
}

/// Prometheus exporter bridge over `MetricsCollector` snapshots.
pub struct PrometheusExporter {
    collector: Arc<MetricsCollector>,
    config: PrometheusExportConfig,
    cached_body: Arc<RwLock<Bytes>>,
    render_duration_histogram: Arc<RwLock<DurationHistogram>>,
    refresh_failures: AtomicU64,
}

impl PrometheusExporter {
    pub fn new(collector: Arc<MetricsCollector>, config: PrometheusExportConfig) -> Self {
        Self {
            collector,
            config,
            cached_body: Arc::new(RwLock::new(Bytes::new())),
            render_duration_histogram: Arc::new(RwLock::new(DurationHistogram::new(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
            ]))),
            refresh_failures: AtomicU64::new(0),
        }
    }

    /// Starts the background cache refresh worker.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let refresh_interval = self.config.refresh_interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(refresh_interval);
            loop {
                ticker.tick().await;
                if let Err(err) = self.refresh_once().await {
                    self.refresh_failures.fetch_add(1, AtomicOrdering::Relaxed);
                    warn!("prometheus exporter refresh failed: {err}");
                }
            }
        })
    }

    pub async fn refresh_once(&self) -> Result<(), PrometheusExportError> {
        let snapshot = self.collector.current().await;

        let render_start = Instant::now();
        let mut body = render_snapshot(&snapshot);
        self.append_exporter_internal_metrics(
            &mut body,
            render_start.elapsed().as_secs_f64(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        )
        .await;

        *self.cached_body.write().await = Bytes::from(body);

        Ok(())
    }

    /// Returns the current Prometheus payload from cache.
    pub async fn render_cached(&self) -> Bytes {
        self.cached_body.read().await.clone()
    }

    async fn append_exporter_internal_metrics(
        &self,
        out: &mut String,
        render_duration: f64,
        last_refresh_unix_seconds: f64,
    ) {
        {
            let mut render_hist = self.render_duration_histogram.write().await;
            render_hist.observe(render_duration);

            write_metric_header(
                out,
                "mofa_exporter_render_duration_seconds",
                "Distribution of Prometheus payload render duration",
                "histogram",
            );
            append_histogram_lines(
                out,
                "mofa_exporter_render_duration_seconds",
                &[],
                &render_hist.bounds,
                &render_hist.sample,
            );
        }

        write_metric_header(
            out,
            "mofa_exporter_refresh_failures_total",
            "Total background refresh failures for the Prometheus exporter",
            "counter",
        );
        append_gauge_line(
            out,
            "mofa_exporter_refresh_failures_total",
            &[],
            self.refresh_failures.load(AtomicOrdering::Relaxed) as f64,
        );

        write_metric_header(
            out,
            "mofa_exporter_last_refresh_timestamp_seconds",
            "Unix timestamp of the last successful /metrics payload refresh",
            "gauge",
        );
        append_gauge_line(
            out,
            "mofa_exporter_last_refresh_timestamp_seconds",
            &[],
            last_refresh_unix_seconds,
        );
    }
}

fn render_snapshot(snapshot: &MetricsSnapshot) -> String {
    let mut out = String::with_capacity(16 * 1024);

    render_agent_metrics(&mut out, snapshot);
    render_workflow_metrics(&mut out, snapshot);
    render_plugin_metrics(&mut out, snapshot);
    render_llm_metrics(&mut out, snapshot);
    render_system_metrics(&mut out, snapshot);
    render_custom_metrics(&mut out, snapshot);

    out
}

fn render_agent_metrics(out: &mut String, snapshot: &MetricsSnapshot) {
    write_metric_header(
        out,
        "mofa_agent_tasks_total",
        "Total tasks completed by agent",
        "counter",
    );
    for agent in &snapshot.agents {
        append_gauge_line(
            out,
            "mofa_agent_tasks_total",
            &[("agent_id".to_string(), agent.agent_id.clone())],
            agent.tasks_completed as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_agent_tasks_failed_total",
        "Total failed tasks by agent",
        "counter",
    );
    for agent in &snapshot.agents {
        append_gauge_line(
            out,
            "mofa_agent_tasks_failed_total",
            &[("agent_id".to_string(), agent.agent_id.clone())],
            agent.tasks_failed as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_agent_tasks_in_progress",
        "Current in-progress tasks by agent",
        "gauge",
    );
    for agent in &snapshot.agents {
        append_gauge_line(
            out,
            "mofa_agent_tasks_in_progress",
            &[("agent_id".to_string(), agent.agent_id.clone())],
            agent.tasks_in_progress as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_agent_response_time_seconds",
        "Average task duration by agent in seconds",
        "gauge",
    );
    for agent in &snapshot.agents {
        append_gauge_line(
            out,
            "mofa_agent_response_time_seconds",
            &[("agent_id".to_string(), agent.agent_id.clone())],
            agent.avg_task_duration_ms / 1000.0,
        );
    }

    write_metric_header(
        out,
        "mofa_agent_messages_sent_total",
        "Total messages sent by agent",
        "counter",
    );
    for agent in &snapshot.agents {
        append_gauge_line(
            out,
            "mofa_agent_messages_sent_total",
            &[("agent_id".to_string(), agent.agent_id.clone())],
            agent.messages_sent as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_agent_messages_received_total",
        "Total messages received by agent",
        "counter",
    );
    for agent in &snapshot.agents {
        append_gauge_line(
            out,
            "mofa_agent_messages_received_total",
            &[("agent_id".to_string(), agent.agent_id.clone())],
            agent.messages_received as f64,
        );
    }
}

fn render_workflow_metrics(out: &mut String, snapshot: &MetricsSnapshot) {
    write_metric_header(
        out,
        "mofa_workflow_executions_total",
        "Total workflow executions",
        "counter",
    );
    for workflow in &snapshot.workflows {
        append_gauge_line(
            out,
            "mofa_workflow_executions_total",
            &[("workflow_id".to_string(), workflow.workflow_id.clone())],
            workflow.total_executions as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_workflow_executions_success_total",
        "Total successful workflow executions",
        "counter",
    );
    for workflow in &snapshot.workflows {
        append_gauge_line(
            out,
            "mofa_workflow_executions_success_total",
            &[("workflow_id".to_string(), workflow.workflow_id.clone())],
            workflow.successful_executions as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_workflow_executions_failed_total",
        "Total failed workflow executions",
        "counter",
    );
    for workflow in &snapshot.workflows {
        append_gauge_line(
            out,
            "mofa_workflow_executions_failed_total",
            &[("workflow_id".to_string(), workflow.workflow_id.clone())],
            workflow.failed_executions as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_workflow_duration_seconds",
        "Average workflow execution duration in seconds",
        "gauge",
    );
    for workflow in &snapshot.workflows {
        append_gauge_line(
            out,
            "mofa_workflow_duration_seconds",
            &[("workflow_id".to_string(), workflow.workflow_id.clone())],
            workflow.avg_execution_time_ms / 1000.0,
        );
    }

    write_metric_header(
        out,
        "mofa_workflow_active",
        "Currently running workflow instances",
        "gauge",
    );
    for workflow in &snapshot.workflows {
        append_gauge_line(
            out,
            "mofa_workflow_active",
            &[("workflow_id".to_string(), workflow.workflow_id.clone())],
            workflow.running_instances as f64,
        );
    }
}

fn render_plugin_metrics(out: &mut String, snapshot: &MetricsSnapshot) {
    write_metric_header(
        out,
        "mofa_tool_calls_total",
        "Total tool/plugin call count",
        "counter",
    );
    for plugin in &snapshot.plugins {
        append_gauge_line(
            out,
            "mofa_tool_calls_total",
            &[("tool_name".to_string(), plugin.name.clone())],
            plugin.call_count as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_tool_errors_total",
        "Total tool/plugin errors",
        "counter",
    );
    for plugin in &snapshot.plugins {
        append_gauge_line(
            out,
            "mofa_tool_errors_total",
            &[("tool_name".to_string(), plugin.name.clone())],
            plugin.error_count as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_tool_response_time_seconds",
        "Average tool/plugin response duration in seconds",
        "gauge",
    );
    for plugin in &snapshot.plugins {
        append_gauge_line(
            out,
            "mofa_tool_response_time_seconds",
            &[("tool_name".to_string(), plugin.name.clone())],
            plugin.avg_response_time_ms / 1000.0,
        );
    }
}

fn render_llm_metrics(out: &mut String, snapshot: &MetricsSnapshot) {
    write_metric_header(
        out,
        "mofa_llm_requests_total",
        "Total LLM requests",
        "counter",
    );
    for llm in &snapshot.llm_metrics {
        append_gauge_line(
            out,
            "mofa_llm_requests_total",
            &[
                ("provider".to_string(), llm.provider_name.clone()),
                ("model".to_string(), llm.model_name.clone()),
            ],
            llm.total_requests as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_llm_tokens_per_second",
        "LLM generation speed in tokens per second",
        "gauge",
    );
    for llm in &snapshot.llm_metrics {
        append_gauge_line(
            out,
            "mofa_llm_tokens_per_second",
            &[
                ("provider".to_string(), llm.provider_name.clone()),
                ("model".to_string(), llm.model_name.clone()),
            ],
            llm.tokens_per_second.unwrap_or_default(),
        );
    }

    write_metric_header(out, "mofa_llm_errors_total", "Total LLM errors", "counter");
    for llm in &snapshot.llm_metrics {
        append_gauge_line(
            out,
            "mofa_llm_errors_total",
            &[
                ("provider".to_string(), llm.provider_name.clone()),
                ("model".to_string(), llm.model_name.clone()),
            ],
            llm.failed_requests as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_llm_latency_seconds",
        "Average LLM request latency in seconds",
        "gauge",
    );
    for llm in &snapshot.llm_metrics {
        append_gauge_line(
            out,
            "mofa_llm_latency_seconds",
            &[
                ("provider".to_string(), llm.provider_name.clone()),
                ("model".to_string(), llm.model_name.clone()),
            ],
            llm.avg_latency_ms / 1000.0,
        );
    }
}

fn render_system_metrics(out: &mut String, snapshot: &MetricsSnapshot) {
    write_metric_header(
        out,
        "mofa_system_cpu_percent",
        "System CPU usage percentage",
        "gauge",
    );
    append_gauge_line(
        out,
        "mofa_system_cpu_percent",
        &[],
        snapshot.system.cpu_usage,
    );

    write_metric_header(
        out,
        "mofa_system_memory_bytes",
        "System memory used in bytes",
        "gauge",
    );
    append_gauge_line(
        out,
        "mofa_system_memory_bytes",
        &[],
        snapshot.system.memory_used as f64,
    );

    write_metric_header(
        out,
        "mofa_system_memory_total_bytes",
        "System total memory in bytes",
        "gauge",
    );
    append_gauge_line(
        out,
        "mofa_system_memory_total_bytes",
        &[],
        snapshot.system.memory_total as f64,
    );

    write_metric_header(
        out,
        "mofa_system_uptime_seconds",
        "System/process uptime in seconds",
        "gauge",
    );
    append_gauge_line(
        out,
        "mofa_system_uptime_seconds",
        &[],
        snapshot.system.uptime_secs as f64,
    );

    write_metric_header(
        out,
        "mofa_system_thread_count",
        "System thread count",
        "gauge",
    );
    append_gauge_line(
        out,
        "mofa_system_thread_count",
        &[],
        snapshot.system.thread_count as f64,
    );
}

fn render_custom_metrics(out: &mut String, snapshot: &MetricsSnapshot) {
    let mut custom = snapshot.custom.iter().collect::<Vec<_>>();
    custom.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut collision_counts: HashMap<String, usize> = HashMap::new();

    for (name, value) in custom {
        let sanitized = sanitize_metric_name(name);
        let next = collision_counts.entry(sanitized.clone()).or_insert(0);
        let metric_name = if *next == 0 {
            sanitized
        } else {
            format!("{sanitized}_{}", *next)
        };
        *next += 1;

        match value {
            MetricValue::Integer(v) => {
                write_metric_header(
                    out,
                    &metric_name,
                    "Custom metric exported from MetricsRegistry",
                    "gauge",
                );
                append_gauge_line(out, &metric_name, &[], *v as f64);
            }
            MetricValue::Float(v) => {
                write_metric_header(
                    out,
                    &metric_name,
                    "Custom metric exported from MetricsRegistry",
                    "gauge",
                );
                append_gauge_line(out, &metric_name, &[], *v);
            }
            MetricValue::Histogram(hist) => {
                write_metric_header(
                    out,
                    &metric_name,
                    "Custom histogram metric exported from MetricsRegistry",
                    "histogram",
                );
                let mut sorted = hist.buckets.clone();
                sorted.sort_by(|(left, _), (right, _)| {
                    left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
                });
                let mut cumulative = Vec::with_capacity(sorted.len());
                for (_, count) in &sorted {
                    cumulative.push(*count);
                }
                let bounds = sorted.iter().map(|(bound, _)| *bound).collect::<Vec<_>>();
                let sample = HistogramSample {
                    count: hist.count,
                    sum: hist.sum,
                    bucket_counts: cumulative,
                };
                append_histogram_lines(out, &metric_name, &[], &bounds, &sample);
            }
        }
    }
}

fn sanitize_metric_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        return "mofa_custom_metric".to_string();
    }

    // Prometheus requires the first character to match [a-zA-Z_:].
    if let Some(first) = out.chars().next()
        && !(first.is_ascii_alphabetic() || first == '_' || first == ':')
    {
        return format!("mofa_custom_{out}");
    }

    out
}

fn write_metric_header(out: &mut String, name: &str, help: &str, metric_type: &str) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} {metric_type}");
}

fn append_gauge_line(out: &mut String, name: &str, labels: &[(String, String)], value: f64) {
    if !value.is_finite() {
        return;
    }

    if labels.is_empty() {
        let _ = writeln!(out, "{name} {}", format_float(value));
        return;
    }

    let rendered_labels = labels
        .iter()
        .map(|(k, v)| {
            let escaped = escape_label_value(v);
            format!("{k}=\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(",");
    let _ = writeln!(out, "{name}{{{rendered_labels}}} {}", format_float(value));
}

fn append_histogram_lines(
    out: &mut String,
    base_name: &str,
    labels: &[(String, String)],
    bounds: &[f64],
    sample: &HistogramSample,
) {
    for (idx, bound) in bounds.iter().enumerate() {
        let mut with_le = labels.to_vec();
        with_le.push(("le".to_string(), format_float(*bound)));
        append_gauge_line(
            out,
            &format!("{base_name}_bucket"),
            &with_le,
            sample.bucket_counts.get(idx).copied().unwrap_or_default() as f64,
        );
    }

    let mut with_inf = labels.to_vec();
    with_inf.push(("le".to_string(), "+Inf".to_string()));
    append_gauge_line(
        out,
        &format!("{base_name}_bucket"),
        &with_inf,
        sample.count as f64,
    );
    append_gauge_line(out, &format!("{base_name}_sum"), labels, sample.sum);
    append_gauge_line(
        out,
        &format!("{base_name}_count"),
        labels,
        sample.count as f64,
    );
}

fn format_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, http::StatusCode, routing::get};
    use tokio::time::{Duration, timeout};
    use tower::ServiceExt;

    fn sample_snapshot() -> MetricsSnapshot {
        MetricsSnapshot {
            system: super::super::metrics::SystemMetrics {
                cpu_usage: 40.5,
                memory_used: 1024,
                memory_total: 4096,
                uptime_secs: 77,
                thread_count: 5,
                timestamp: 1,
            },
            agents: vec![super::super::metrics::AgentMetrics {
                agent_id: "agent-1".to_string(),
                tasks_completed: 42,
                tasks_failed: 1,
                tasks_in_progress: 2,
                avg_task_duration_ms: 120.0,
                messages_sent: 9,
                messages_received: 8,
                ..Default::default()
            }],
            workflows: vec![],
            plugins: vec![],
            llm_metrics: vec![],
            timestamp: 2,
            custom: HashMap::new(),
        }
    }

    async fn seed_collector_from_snapshot(collector: &MetricsCollector, snapshot: MetricsSnapshot) {
        for agent in snapshot.agents {
            collector.update_agent(agent).await;
        }
        for workflow in snapshot.workflows {
            collector.update_workflow(workflow).await;
        }
        for plugin in snapshot.plugins {
            collector.update_plugin(plugin).await;
        }
        for llm in snapshot.llm_metrics {
            collector.update_llm(llm).await;
        }
        let _ = collector.collect().await;
    }

    #[tokio::test]
    async fn renders_prometheus_headers_and_labels() {
        let collector = Arc::new(MetricsCollector::new(Default::default()));
        seed_collector_from_snapshot(&collector, sample_snapshot()).await;

        let exporter = PrometheusExporter::new(collector, PrometheusExportConfig::default());
        exporter.refresh_once().await.expect("refresh");
        let output = exporter.render_cached().await;
        let output = std::str::from_utf8(output.as_ref()).expect("utf8");

        assert!(output.contains("# HELP mofa_agent_tasks_total"));
        assert!(output.contains("# TYPE mofa_agent_tasks_total counter"));
        assert!(output.contains("mofa_agent_tasks_total{agent_id=\"agent-1\"} 42"));
        assert!(output.contains("# HELP mofa_system_cpu_percent"));
    }

    #[tokio::test]
    async fn serves_metrics_route() {
        let collector = Arc::new(MetricsCollector::new(Default::default()));
        seed_collector_from_snapshot(&collector, sample_snapshot()).await;

        let exporter = Arc::new(PrometheusExporter::new(
            collector,
            PrometheusExportConfig::default(),
        ));
        exporter.refresh_once().await.expect("refresh");

        let app = Router::new().route(
            "/metrics",
            get({
                let exporter = exporter.clone();
                move || {
                    let exporter = exporter.clone();
                    async move {
                        (
                            [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
                            exporter.render_cached().await,
                        )
                    }
                }
            }),
        );

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/metrics")
                    .body(axum::body::Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn concurrent_scrapes_with_refresh_worker_complete() {
        let collector = Arc::new(MetricsCollector::new(Default::default()));
        let exporter = Arc::new(PrometheusExporter::new(
            collector.clone(),
            PrometheusExportConfig::default().with_refresh_interval(Duration::from_millis(20)),
        ));

        exporter.refresh_once().await.expect("initial refresh");
        let worker = exporter.clone().start();

        let updater = {
            let collector = collector.clone();
            tokio::spawn(async move {
                for idx in 0..100u64 {
                    collector
                        .update_agent(super::super::metrics::AgentMetrics {
                            agent_id: format!("agent-{idx}"),
                            tasks_completed: idx,
                            avg_task_duration_ms: idx as f64,
                            ..Default::default()
                        })
                        .await;
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            })
        };

        let mut scrapers = Vec::new();
        for _ in 0..20 {
            let exporter = exporter.clone();
            scrapers.push(tokio::spawn(async move {
                for _ in 0..20 {
                    let payload = exporter.render_cached().await;
                    let payload = std::str::from_utf8(payload.as_ref()).expect("utf8");
                    assert!(payload.contains("mofa_exporter_last_refresh_timestamp_seconds"));
                }
            }));
        }

        timeout(Duration::from_secs(8), async {
            let _ = updater.await;
            for scraper in scrapers {
                let _ = scraper.await;
            }
        })
        .await
        .expect("concurrency test timed out");

        worker.abort();
    }

    #[test]
    fn escapes_label_values() {
        let escaped = escape_label_value("a\"b\\c\n");
        assert_eq!(escaped, "a\\\"b\\\\c\\n");
    }

    #[test]
    fn sanitizes_metric_names_with_invalid_first_char() {
        assert_eq!(sanitize_metric_name("1foo"), "mofa_custom_1foo");
        assert_eq!(sanitize_metric_name(""), "mofa_custom_metric");
    }

    #[test]
    fn zero_refresh_interval_is_clamped() {
        let config = PrometheusExportConfig::default().with_refresh_interval(Duration::ZERO);
        assert_eq!(config.refresh_interval, Duration::from_millis(1));
    }

    #[test]
    fn custom_metric_name_collisions_are_disambiguated() {
        let mut snapshot = sample_snapshot();
        snapshot
            .custom
            .insert("foo-bar".to_string(), MetricValue::Integer(1));
        snapshot
            .custom
            .insert("foo_bar".to_string(), MetricValue::Integer(2));
        let output = render_snapshot(&snapshot);

        assert!(output.contains("# HELP foo_bar "));
        assert!(output.contains("# HELP foo_bar_1 "));
    }
}
