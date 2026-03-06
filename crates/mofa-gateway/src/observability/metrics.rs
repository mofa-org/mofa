//! Prometheus metrics collection.
//!
//! This module provides comprehensive metrics for the gateway and control plane:
//! - Request metrics (latency, throughput, errors)
//! - Node health metrics
//! - Consensus metrics (term, log index, leader elections)
//! - Agent registry metrics

use prometheus::{
    Counter, Encoder, Gauge, Histogram, HistogramOpts, Opts, Registry, TextEncoder,
};
use std::sync::Arc;
use std::time::Duration;

/// Metrics collector for the gateway and control plane.
pub struct GatewayMetrics {
    registry: Registry,

    // Request metrics
    /// Total number of requests processed by the gateway.
    pub requests_total: Counter,
    /// Histogram of request durations in milliseconds.
    pub requests_duration: Histogram,
    /// Total number of requests that resulted in errors.
    pub requests_errors_total: Counter,

    // Node metrics
    /// Total number of nodes in the cluster.
    pub nodes_total: Gauge,
    /// Number of nodes currently marked as healthy.
    pub nodes_healthy: Gauge,
    /// Number of nodes currently marked as unhealthy.
    pub nodes_unhealthy: Gauge,

    // Consensus metrics
    /// Current Raft consensus term.
    pub consensus_term: Gauge,
    /// Current Raft log index.
    pub consensus_log_index: Gauge,
    /// Total number of leader elections that have occurred.
    pub consensus_leader_elections_total: Counter,
    /// Total number of heartbeats sent by the leader.
    pub consensus_heartbeats_total: Counter,

    // Agent registry metrics
    /// Current number of registered agents.
    pub agents_registered: Gauge,
    /// Total number of agents that have been unregistered.
    pub agents_unregistered_total: Counter,

    // Load balancer metrics
    /// Total number of node selections made by the load balancer.
    pub load_balancer_selections_total: Counter,
    /// Total number of load balancer errors.
    pub load_balancer_errors_total: Counter,

    // Circuit breaker metrics
    /// Total number of times circuit breakers have opened.
    pub circuit_breaker_opens_total: Counter,
    /// Total number of times circuit breakers have closed.
    pub circuit_breaker_closes_total: Counter,

    // Health check metrics
    /// Total number of health checks performed.
    pub health_checks_total: Counter,
    /// Total number of health checks that failed.
    pub health_checks_failed_total: Counter,
}

impl GatewayMetrics {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        let registry = Registry::new();

        // Request metrics
        let requests_total = Counter::with_opts(
            Opts::new("gateway_requests_total", "Total number of requests processed"),
        )
        .unwrap();
        registry.register(Box::new(requests_total.clone())).unwrap();

        let requests_duration = Histogram::with_opts(
            HistogramOpts::new("gateway_requests_duration_seconds", "Request duration in seconds")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        )
        .unwrap();
        registry.register(Box::new(requests_duration.clone())).unwrap();

        let requests_errors_total = Counter::with_opts(
            Opts::new("gateway_requests_errors_total", "Total number of request errors"),
        )
        .unwrap();
        registry.register(Box::new(requests_errors_total.clone())).unwrap();

        // Node metrics
        let nodes_total = Gauge::with_opts(
            Opts::new("gateway_nodes_total", "Total number of nodes in cluster"),
        )
        .unwrap();
        registry.register(Box::new(nodes_total.clone())).unwrap();

        let nodes_healthy = Gauge::with_opts(
            Opts::new("gateway_nodes_healthy", "Number of healthy nodes"),
        )
        .unwrap();
        registry.register(Box::new(nodes_healthy.clone())).unwrap();

        let nodes_unhealthy = Gauge::with_opts(
            Opts::new("gateway_nodes_unhealthy", "Number of unhealthy nodes"),
        )
        .unwrap();
        registry.register(Box::new(nodes_unhealthy.clone())).unwrap();

        // Consensus metrics
        let consensus_term = Gauge::with_opts(
            Opts::new("gateway_consensus_term", "Current Raft term"),
        )
        .unwrap();
        registry.register(Box::new(consensus_term.clone())).unwrap();

        let consensus_log_index = Gauge::with_opts(
            Opts::new("gateway_consensus_log_index", "Current Raft log index"),
        )
        .unwrap();
        registry.register(Box::new(consensus_log_index.clone())).unwrap();

        let consensus_leader_elections_total = Counter::with_opts(
            Opts::new("gateway_consensus_leader_elections_total", "Total number of leader elections"),
        )
        .unwrap();
        registry.register(Box::new(consensus_leader_elections_total.clone())).unwrap();

        let consensus_heartbeats_total = Counter::with_opts(
            Opts::new("gateway_consensus_heartbeats_total", "Total number of heartbeats sent"),
        )
        .unwrap();
        registry.register(Box::new(consensus_heartbeats_total.clone())).unwrap();

        // Agent registry metrics
        let agents_registered = Gauge::with_opts(
            Opts::new("gateway_agents_registered", "Number of registered agents"),
        )
        .unwrap();
        registry.register(Box::new(agents_registered.clone())).unwrap();

        let agents_unregistered_total = Counter::with_opts(
            Opts::new("gateway_agents_unregistered_total", "Total number of agent unregistrations"),
        )
        .unwrap();
        registry.register(Box::new(agents_unregistered_total.clone())).unwrap();

        // Load balancer metrics
        let load_balancer_selections_total = Counter::with_opts(
            Opts::new("gateway_load_balancer_selections_total", "Total number of node selections by load balancer"),
        )
        .unwrap();
        registry.register(Box::new(load_balancer_selections_total.clone())).unwrap();

        let load_balancer_errors_total = Counter::with_opts(
            Opts::new("gateway_load_balancer_errors_total", "Total number of load balancer errors"),
        )
        .unwrap();
        registry.register(Box::new(load_balancer_errors_total.clone())).unwrap();

        // Circuit breaker metrics
        let circuit_breaker_opens_total = Counter::with_opts(
            Opts::new("gateway_circuit_breaker_opens_total", "Total number of circuit breaker opens"),
        )
        .unwrap();
        registry.register(Box::new(circuit_breaker_opens_total.clone())).unwrap();

        let circuit_breaker_closes_total = Counter::with_opts(
            Opts::new("gateway_circuit_breaker_closes_total", "Total number of circuit breaker closes"),
        )
        .unwrap();
        registry.register(Box::new(circuit_breaker_closes_total.clone())).unwrap();

        // Health check metrics
        let health_checks_total = Counter::with_opts(
            Opts::new("gateway_health_checks_total", "Total number of health checks performed"),
        )
        .unwrap();
        registry.register(Box::new(health_checks_total.clone())).unwrap();

        let health_checks_failed_total = Counter::with_opts(
            Opts::new("gateway_health_checks_failed_total", "Total number of failed health checks"),
        )
        .unwrap();
        registry.register(Box::new(health_checks_failed_total.clone())).unwrap();

        Self {
            registry,
            requests_total,
            requests_duration,
            requests_errors_total,
            nodes_total,
            nodes_healthy,
            nodes_unhealthy,
            consensus_term,
            consensus_log_index,
            consensus_leader_elections_total,
            consensus_heartbeats_total,
            agents_registered,
            agents_unregistered_total,
            load_balancer_selections_total,
            load_balancer_errors_total,
            circuit_breaker_opens_total,
            circuit_breaker_closes_total,
            health_checks_total,
            health_checks_failed_total,
        }
    }

    /// Get the Prometheus registry.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Export metrics in Prometheus text format.
    pub fn export(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    /// Record a request duration.
    pub fn record_request_duration(&self, duration: Duration) {
        self.requests_duration.observe(duration.as_secs_f64());
    }

    /// Increment request counter.
    pub fn increment_requests(&self) {
        self.requests_total.inc();
    }

    /// Increment error counter.
    pub fn increment_errors(&self) {
        self.requests_errors_total.inc();
    }

    /// Update node counts.
    pub fn update_node_counts(&self, total: usize, healthy: usize, unhealthy: usize) {
        self.nodes_total.set(total as f64);
        self.nodes_healthy.set(healthy as f64);
        self.nodes_unhealthy.set(unhealthy as f64);
    }

    /// Update consensus term.
    pub fn update_consensus_term(&self, term: u64) {
        self.consensus_term.set(term as f64);
    }

    /// Update consensus log index.
    pub fn update_consensus_log_index(&self, index: u64) {
        self.consensus_log_index.set(index as f64);
    }

    /// Increment leader election counter.
    pub fn increment_leader_elections(&self) {
        self.consensus_leader_elections_total.inc();
    }

    /// Increment heartbeat counter.
    pub fn increment_heartbeats(&self) {
        self.consensus_heartbeats_total.inc();
    }

    /// Update agent count.
    pub fn update_agent_count(&self, count: usize) {
        self.agents_registered.set(count as f64);
    }

    /// Increment agent unregistration counter.
    pub fn increment_agent_unregistrations(&self) {
        self.agents_unregistered_total.inc();
    }

    /// Increment load balancer selection counter.
    pub fn increment_load_balancer_selections(&self) {
        self.load_balancer_selections_total.inc();
    }

    /// Increment load balancer error counter.
    pub fn increment_load_balancer_errors(&self) {
        self.load_balancer_errors_total.inc();
    }

    /// Increment circuit breaker open counter.
    pub fn increment_circuit_breaker_opens(&self) {
        self.circuit_breaker_opens_total.inc();
    }

    /// Increment circuit breaker close counter.
    pub fn increment_circuit_breaker_closes(&self) {
        self.circuit_breaker_closes_total.inc();
    }

    /// Increment health check counter.
    pub fn increment_health_checks(&self) {
        self.health_checks_total.inc();
    }

    /// Increment failed health check counter.
    pub fn increment_health_check_failures(&self) {
        self.health_checks_failed_total.inc();
    }
}

impl Default for GatewayMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics collector wrapped in Arc for sharing.
pub type SharedMetrics = Arc<GatewayMetrics>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = GatewayMetrics::new();
        assert_eq!(metrics.nodes_total.get(), 0.0);
        assert_eq!(metrics.requests_total.get(), 0.0);
    }

    #[test]
    fn test_metrics_export() {
        let metrics = GatewayMetrics::new();
        metrics.increment_requests();
        metrics.update_node_counts(5, 4, 1);

        let exported = metrics.export().unwrap();
        assert!(exported.contains("gateway_requests_total"));
        assert!(exported.contains("gateway_nodes_total"));
    }

    #[test]
    fn test_request_duration_recording() {
        let metrics = GatewayMetrics::new();
        metrics.record_request_duration(Duration::from_millis(100));
        // Duration is recorded in histogram, can't easily assert value
        // but we can verify it doesn't panic
    }
}
