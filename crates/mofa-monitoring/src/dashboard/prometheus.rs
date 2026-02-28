//! Prometheus metrics export bridge for dashboard metrics.

use super::metrics::{MetricValue, MetricsCollector, MetricsSnapshot};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// Prometheus export configuration.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct PrometheusExportConfig;

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

/// Prometheus exporter bridge over `MetricsCollector` snapshots.
pub struct PrometheusExporter {
    collector: Arc<MetricsCollector>,
    _config: PrometheusExportConfig,
}

impl PrometheusExporter {
    pub fn new(collector: Arc<MetricsCollector>, config: PrometheusExportConfig) -> Self {
        Self {
            collector,
            _config: config,
        }
    }

    /// Kept for forward compatibility with cached exporter variants.
    pub async fn refresh_once(&self) -> Result<(), PrometheusExportError> {
        Ok(())
    }

    /// Render current snapshot into Prometheus text exposition format.
    pub async fn render_cached(&self) -> String {
        let snapshot = self.collector.current().await;
        render_snapshot(&snapshot)
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
        "gauge",
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
        "gauge",
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
        "gauge",
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
        "gauge",
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
        "gauge",
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
        "gauge",
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
        "gauge",
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
        "mofa_tool_call_count",
        "Total tool/plugin call count",
        "gauge",
    );
    for plugin in &snapshot.plugins {
        append_gauge_line(
            out,
            "mofa_tool_call_count",
            &[("tool_name".to_string(), plugin.name.clone())],
            plugin.call_count as f64,
        );
    }

    write_metric_header(
        out,
        "mofa_tool_error_count",
        "Total tool/plugin errors",
        "gauge",
    );
    for plugin in &snapshot.plugins {
        append_gauge_line(
            out,
            "mofa_tool_error_count",
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
        "gauge",
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

    write_metric_header(out, "mofa_llm_errors_total", "Total LLM errors", "gauge");
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
    for (name, value) in &snapshot.custom {
        let metric_name = sanitize_metric_name(name);
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
                let mut cumulative = Vec::with_capacity(hist.buckets.len());
                for (_, count) in &hist.buckets {
                    cumulative.push(*count);
                }
                let bounds = hist
                    .buckets
                    .iter()
                    .map(|(bound, _)| *bound)
                    .collect::<Vec<_>>();
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
        format!("{value:.6}")
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

    #[test]
    fn renders_prometheus_headers_and_labels() {
        let output = render_snapshot(&sample_snapshot());

        assert!(output.contains("# HELP mofa_agent_tasks_total"));
        assert!(output.contains("# TYPE mofa_agent_tasks_total gauge"));
        assert!(output.contains("mofa_agent_tasks_total{agent_id=\"agent-1\"} 42"));
        assert!(output.contains("# HELP mofa_system_cpu_percent"));
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
}
