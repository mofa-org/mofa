//! Prometheus-format metrics exporter for the swarm scheduler layer.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Mutex;

use crate::swarm::{SchedulerSummary, SwarmMetrics};

const BUCKETS: &[f64] = &[0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0];

#[derive(Debug, Default)]
struct PatternCounters {
    runs: u64,
    succeeded: u64,
    failed: u64,
    skipped: u64,
}

#[derive(Debug)]
struct HistogramData {
    /// count of observations falling in each bucket (parallel to BUCKETS)
    buckets: Vec<u64>,
    count: u64,
    sum_secs: f64,
}

impl HistogramData {
    fn new() -> Self {
        Self {
            buckets: vec![0; BUCKETS.len()],
            count: 0,
            sum_secs: 0.0,
        }
    }

    fn observe(&mut self, secs: f64) {
        self.count += 1;
        self.sum_secs += secs;
        for (i, &le) in BUCKETS.iter().enumerate() {
            if secs <= le {
                self.buckets[i] += 1;
            }
        }
    }
}

#[derive(Debug, Default)]
struct ExporterInner {
    patterns: HashMap<String, PatternCounters>,
    histograms: HashMap<String, HistogramData>,
    hitl_total: u64,
    tokens_total: u64,
}

/// Thread-safe Prometheus metrics exporter for swarm scheduler runs.
///
/// Records per-pattern counters and duration histograms, then renders
/// valid Prometheus text-format output via `render()`.
///
/// # Example
/// ```rust,ignore
/// let exporter = SwarmMetricsExporter::new();
/// exporter.record_scheduler_run(&summary);
/// println!("{}", exporter.render());
/// ```
#[derive(Debug, Default)]
pub struct SwarmMetricsExporter {
    inner: Mutex<ExporterInner>,
}

impl SwarmMetricsExporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the outcome of one scheduler execution.
    pub fn record_scheduler_run(&self, summary: &SchedulerSummary) {
        let pattern = summary.pattern.to_string();
        let secs = summary.total_wall_time.as_secs_f64();
        let mut g = self.inner.lock().expect("metrics lock poisoned");
        let pc = g.patterns.entry(pattern.clone()).or_default();
        pc.runs += 1;
        pc.succeeded += u64::try_from(summary.succeeded).unwrap_or(u64::MAX);
        pc.failed += u64::try_from(summary.failed).unwrap_or(u64::MAX);
        pc.skipped += u64::try_from(summary.skipped).unwrap_or(u64::MAX);
        g.histograms
            .entry(pattern)
            .or_insert_with(HistogramData::new)
            .observe(secs);
    }

    /// Record token and HITL counts from a completed swarm result.
    pub fn record_swarm_result(&self, metrics: &SwarmMetrics) {
        let mut g = self.inner.lock().expect("metrics lock poisoned");
        g.hitl_total += u64::try_from(metrics.hitl_interventions).unwrap_or(u64::MAX);
        g.tokens_total += metrics.total_tokens;
    }

    /// Reset all counters and histograms to zero.
    pub fn reset(&self) {
        let mut g = self.inner.lock().expect("metrics lock poisoned");
        *g = ExporterInner::default();
    }

    /// Render Prometheus text-format exposition.
    ///
    /// Returns an empty string if no runs have been recorded yet.
    pub fn render(&self) -> String {
        let g = self.inner.lock().expect("metrics lock poisoned");
        if g.patterns.is_empty() && g.hitl_total == 0 && g.tokens_total == 0 {
            return String::new();
        }

        let mut out = String::new();
        let mut patterns: Vec<&str> = g.patterns.keys().map(|s| s.as_str()).collect();
        patterns.sort_unstable();

        out.push_str("# HELP mofa_swarm_scheduler_runs_total total scheduler executions per pattern\n");
        out.push_str("# TYPE mofa_swarm_scheduler_runs_total counter\n");
        for p in &patterns {
            let pc = &g.patterns[*p];
            let _ = writeln!(out, "mofa_swarm_scheduler_runs_total{{pattern=\"{p}\"}} {}", pc.runs);
        }

        out.push_str("# HELP mofa_swarm_tasks_total total subtasks by pattern and status\n");
        out.push_str("# TYPE mofa_swarm_tasks_total counter\n");
        for p in &patterns {
            let pc = &g.patterns[*p];
            for (status, val) in [("succeeded", pc.succeeded), ("failed", pc.failed), ("skipped", pc.skipped)] {
                let _ = writeln!(out, "mofa_swarm_tasks_total{{pattern=\"{p}\",status=\"{status}\"}} {val}");
            }
        }

        out.push_str("# HELP mofa_swarm_scheduler_duration_seconds wall time per scheduler run in seconds\n");
        out.push_str("# TYPE mofa_swarm_scheduler_duration_seconds histogram\n");
        for p in &patterns {
            if let Some(h) = g.histograms.get(*p) {
                for (i, &le) in BUCKETS.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "mofa_swarm_scheduler_duration_seconds_bucket{{pattern=\"{p}\",le=\"{le}\"}} {}",
                        h.buckets[i]
                    );
                }
                let _ = writeln!(
                    out,
                    "mofa_swarm_scheduler_duration_seconds_bucket{{pattern=\"{p}\",le=\"+Inf\"}} {}",
                    h.count
                );
                let _ = writeln!(out, "mofa_swarm_scheduler_duration_seconds_sum{{pattern=\"{p}\"}} {:.6}", h.sum_secs);
                let _ = writeln!(out, "mofa_swarm_scheduler_duration_seconds_count{{pattern=\"{p}\"}} {}", h.count);
            }
        }

        out.push_str("# HELP mofa_swarm_hitl_interventions_total total HITL interventions recorded\n");
        out.push_str("# TYPE mofa_swarm_hitl_interventions_total counter\n");
        let _ = writeln!(out, "mofa_swarm_hitl_interventions_total {}", g.hitl_total);

        out.push_str("# HELP mofa_swarm_tokens_total total LLM tokens consumed across all swarm runs\n");
        out.push_str("# TYPE mofa_swarm_tokens_total counter\n");
        let _ = writeln!(out, "mofa_swarm_tokens_total {}", g.tokens_total);

        out
    }
}
