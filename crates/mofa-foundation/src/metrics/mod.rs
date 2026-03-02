//! Metrics and Telemetry Module
//!
//! Provides comprehensive metrics and telemetry for tracking agent execution,
//! including execution time, latency percentiles, token usage, tool success/failure rates,
//! memory and CPU utilization, workflow step timing, and custom business metrics.
//!
//! # Architecture
//!
//! The module is structured around three concepts:
//!
//! 1. **Metric data types** — lightweight structs that capture counters, timings, and gauges
//!    for agents, tools, workflows, routing decisions, model pool lifecycle, circuit breakers,
//!    scheduler admission, and retries.
//! 2. **`MetricsBackend` trait** — a pluggable sink so that callers can forward metrics to
//!    Prometheus, OpenTelemetry, or any custom backend.
//! 3. **`MetricsCollector`** — an in-memory, async-safe default implementation that stores
//!    metrics behind `tokio::sync::RwLock` maps.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_foundation::metrics::{MetricsCollector, TokenUsage};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let collector = MetricsCollector::new();
//!
//!     collector.record_agent_execution(
//!         "agent-1",
//!         Duration::from_millis(120),
//!         true,
//!         Some(TokenUsage {
//!             prompt_tokens: 100,
//!             completion_tokens: 50,
//!             total_tokens: 150,
//!             cost_estimate: 0.002,
//!         }),
//!     ).await;
//!
//!     let metrics = collector.get_agent_metrics().await;
//!     println!("Agent executions: {}", metrics[0].total_executions);
//! }
//! ```

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Agent metrics
// ---------------------------------------------------------------------------

/// Aggregated execution metrics for a single agent.
#[derive(Debug, Clone)]
pub struct AgentMetrics {
    pub agent_id: String,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub total_execution_time_ms: u64,
    pub latency_percentiles: LatencyPercentiles,
    pub token_usage: TokenUsage,
    pub memory_usage_bytes: u64,
    pub cpu_usage_percent: f64,
}

/// Token usage tracking for LLM calls.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost_estimate: f64,
}

/// Latency percentiles (p50, p90, p95, p99).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LatencyPercentiles {
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

// ---------------------------------------------------------------------------
// Tool metrics
// ---------------------------------------------------------------------------

/// Aggregated execution metrics for a single tool.
#[derive(Debug, Clone)]
pub struct ToolMetrics {
    pub tool_name: String,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub average_execution_time_ms: f64,
    pub total_execution_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Workflow metrics
// ---------------------------------------------------------------------------

/// Aggregated execution metrics for a workflow.
#[derive(Debug, Clone)]
pub struct WorkflowMetrics {
    pub workflow_id: String,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub step_timings: Vec<StepTiming>,
    pub total_duration_ms: u64,
}

/// Timing for an individual workflow step.
#[derive(Debug, Clone)]
pub struct StepTiming {
    pub step_name: String,
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub status: StepStatus,
}

/// Status of a workflow step.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

// ---------------------------------------------------------------------------
// Routing metrics (forward-looking: records routing decisions when available)
// ---------------------------------------------------------------------------

/// Metrics for local-vs-cloud routing decisions.
#[derive(Debug, Clone, Default)]
pub struct RoutingMetrics {
    pub total_routing_decisions: u64,
    pub local_routing_count: u64,
    pub cloud_routing_count: u64,
    pub fallback_count: u64,
}

// ---------------------------------------------------------------------------
// Model pool metrics
// ---------------------------------------------------------------------------

/// Metrics for model pool load/eviction events.
#[derive(Debug, Clone, Default)]
pub struct ModelPoolMetrics {
    pub total_models_loaded: u64,
    pub total_models_evicted: u64,
    pub current_load: u64,
    pub max_capacity: u64,
    pub eviction_count: u64,
}

/// Events emitted by a model pool.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ModelPoolEvent {
    ModelLoaded,
    ModelEvicted,
    CapacitySet(u64),
}

// ---------------------------------------------------------------------------
// Circuit breaker metrics
// ---------------------------------------------------------------------------

/// Metrics for a single circuit breaker instance.
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub circuit_breaker_id: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub rejected_requests: u64,
    pub state_changes: u64,
    pub current_state: CircuitBreakerState,
}

/// Possible states of a circuit breaker (metrics-level view).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Events emitted by a circuit breaker.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CircuitBreakerEvent {
    RequestAttempt,
    RequestSuccess,
    RequestRejected,
    StateChange(CircuitBreakerState),
}

// ---------------------------------------------------------------------------
// Scheduler metrics
// ---------------------------------------------------------------------------

/// Metrics for scheduler admission decisions.
#[derive(Debug, Clone, Default)]
pub struct SchedulerMetrics {
    pub total_admission_requests: u64,
    pub admitted_count: u64,
    pub rejected_count: u64,
    pub queue_wait_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Retry metrics
// ---------------------------------------------------------------------------

/// Metrics for retry operations.
#[derive(Debug, Clone)]
pub struct RetryMetrics {
    pub total_retries: u64,
    pub successful_retries: u64,
    pub exhausted_retries: u64,
    pub total_backoff_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Custom business metrics
// ---------------------------------------------------------------------------

/// A user-defined business metric with arbitrary tags.
#[derive(Debug, Clone)]
pub struct BusinessMetrics {
    pub metric_name: String,
    pub metric_value: f64,
    pub tags: HashMap<String, String>,
    pub timestamp_ms: u64,
}

// ---------------------------------------------------------------------------
// Pluggable backend trait
// ---------------------------------------------------------------------------

/// Trait for pluggable metrics sinks (Prometheus, OpenTelemetry, logging, etc.).
pub trait MetricsBackend: Send + Sync {
    fn record_agent_metrics(&self, metrics: &AgentMetrics);
    fn record_tool_metrics(&self, metrics: &ToolMetrics);
    fn record_workflow_metrics(&self, metrics: &WorkflowMetrics);
    fn record_routing_metrics(&self, metrics: &RoutingMetrics);
    fn record_model_pool_metrics(&self, metrics: &ModelPoolMetrics);
    fn record_circuit_breaker_metrics(&self, metrics: &CircuitBreakerMetrics);
    fn record_scheduler_metrics(&self, metrics: &SchedulerMetrics);
    fn record_retry_metrics(&self, metrics: &RetryMetrics);
    fn record_business_metric(&self, metric: &BusinessMetrics);
}

// ---------------------------------------------------------------------------
// Builder helper
// ---------------------------------------------------------------------------

/// Convenience builder for tagging metric records.
pub struct MetricBuilder {
    agent_id: Option<String>,
    tool_name: Option<String>,
    tags: HashMap<String, String>,
}

impl MetricBuilder {
    pub fn new() -> Self {
        Self {
            agent_id: None,
            tool_name: None,
            tags: HashMap::new(),
        }
    }

    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.agent_id = Some(agent_id.to_string());
        self
    }

    pub fn with_tool(mut self, tool_name: &str) -> Self {
        self.tool_name = Some(tool_name.to_string());
        self
    }

    pub fn with_tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }

    pub fn build(self) -> (Option<String>, Option<String>, HashMap<String, String>) {
        (self.agent_id, self.tool_name, self.tags)
    }
}

impl Default for MetricBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// In-memory collector
// ---------------------------------------------------------------------------

/// Async-safe, in-memory metrics collector backed by `tokio::sync::RwLock`.
pub struct MetricsCollector {
    agent_metrics: RwLock<HashMap<String, AgentMetrics>>,
    tool_metrics: RwLock<HashMap<String, ToolMetrics>>,
    workflow_metrics: RwLock<HashMap<String, WorkflowMetrics>>,
    routing_metrics: RwLock<RoutingMetrics>,
    model_pool_metrics: RwLock<ModelPoolMetrics>,
    circuit_breaker_metrics: RwLock<HashMap<String, CircuitBreakerMetrics>>,
    scheduler_metrics: RwLock<SchedulerMetrics>,
    retry_metrics: RwLock<HashMap<String, RetryMetrics>>,
    business_metrics: RwLock<Vec<BusinessMetrics>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            agent_metrics: RwLock::new(HashMap::new()),
            tool_metrics: RwLock::new(HashMap::new()),
            workflow_metrics: RwLock::new(HashMap::new()),
            routing_metrics: RwLock::new(RoutingMetrics::default()),
            model_pool_metrics: RwLock::new(ModelPoolMetrics::default()),
            circuit_breaker_metrics: RwLock::new(HashMap::new()),
            scheduler_metrics: RwLock::new(SchedulerMetrics::default()),
            retry_metrics: RwLock::new(HashMap::new()),
            business_metrics: RwLock::new(Vec::new()),
        }
    }

    // -- Agent ---------------------------------------------------------------

    /// Record one agent execution (success or failure) with optional token usage.
    pub async fn record_agent_execution(
        &self,
        agent_id: &str,
        duration: Duration,
        success: bool,
        tokens: Option<TokenUsage>,
    ) {
        let mut metrics = self.agent_metrics.write().await;
        let entry = metrics.entry(agent_id.to_string()).or_insert_with(|| AgentMetrics {
            agent_id: agent_id.to_string(),
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            total_execution_time_ms: 0,
            latency_percentiles: LatencyPercentiles::default(),
            token_usage: TokenUsage::default(),
            memory_usage_bytes: 0,
            cpu_usage_percent: 0.0,
        });

        entry.total_executions += 1;
        entry.total_execution_time_ms +=
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);

        if success {
            entry.successful_executions += 1;
        } else {
            entry.failed_executions += 1;
        }

        if let Some(tu) = tokens {
            entry.token_usage.prompt_tokens += tu.prompt_tokens;
            entry.token_usage.completion_tokens += tu.completion_tokens;
            entry.token_usage.total_tokens += tu.total_tokens;
            entry.token_usage.cost_estimate += tu.cost_estimate;
        }
    }

    /// Return a snapshot of all agent metrics.
    pub async fn get_agent_metrics(&self) -> Vec<AgentMetrics> {
        self.agent_metrics.read().await.values().cloned().collect()
    }

    // -- Tool ----------------------------------------------------------------

    /// Record one tool invocation.
    pub async fn record_tool_execution(
        &self,
        tool_name: &str,
        duration: Duration,
        success: bool,
    ) {
        let mut metrics = self.tool_metrics.write().await;
        let entry = metrics.entry(tool_name.to_string()).or_insert_with(|| ToolMetrics {
            tool_name: tool_name.to_string(),
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            average_execution_time_ms: 0.0,
            total_execution_time_ms: 0,
        });

        entry.total_calls += 1;
        entry.total_execution_time_ms +=
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);

        if success {
            entry.successful_calls += 1;
        } else {
            entry.failed_calls += 1;
        }

        entry.average_execution_time_ms =
            entry.total_execution_time_ms as f64 / entry.total_calls as f64;
    }

    /// Return a snapshot of all tool metrics.
    pub async fn get_tool_metrics(&self) -> Vec<ToolMetrics> {
        self.tool_metrics.read().await.values().cloned().collect()
    }

    // -- Routing -------------------------------------------------------------

    /// Record a single routing decision (local vs cloud).
    pub async fn record_routing_decision(&self, is_local: bool) {
        let mut metrics = self.routing_metrics.write().await;
        metrics.total_routing_decisions += 1;
        if is_local {
            metrics.local_routing_count += 1;
        } else {
            metrics.cloud_routing_count += 1;
        }
    }

    /// Return a snapshot of routing metrics.
    pub async fn get_routing_metrics(&self) -> RoutingMetrics {
        self.routing_metrics.read().await.clone()
    }

    // -- Model pool ----------------------------------------------------------

    /// Record a model pool lifecycle event.
    pub async fn record_model_pool_event(&self, event: ModelPoolEvent) {
        let mut metrics = self.model_pool_metrics.write().await;
        match event {
            ModelPoolEvent::ModelLoaded => {
                metrics.total_models_loaded += 1;
                metrics.current_load += 1;
            }
            ModelPoolEvent::ModelEvicted => {
                metrics.total_models_evicted += 1;
                metrics.current_load = metrics.current_load.saturating_sub(1);
                metrics.eviction_count += 1;
            }
            ModelPoolEvent::CapacitySet(capacity) => {
                metrics.max_capacity = capacity;
            }
        }
    }

    /// Return a snapshot of model pool metrics.
    pub async fn get_model_pool_metrics(&self) -> ModelPoolMetrics {
        self.model_pool_metrics.read().await.clone()
    }

    // -- Circuit breaker -----------------------------------------------------

    /// Record a circuit breaker event.
    pub async fn record_circuit_breaker_event(
        &self,
        cb_id: &str,
        event: CircuitBreakerEvent,
    ) {
        let mut metrics = self.circuit_breaker_metrics.write().await;
        let entry = metrics
            .entry(cb_id.to_string())
            .or_insert_with(|| CircuitBreakerMetrics {
                circuit_breaker_id: cb_id.to_string(),
                total_requests: 0,
                successful_requests: 0,
                rejected_requests: 0,
                state_changes: 0,
                current_state: CircuitBreakerState::Closed,
            });

        match event {
            CircuitBreakerEvent::RequestAttempt => entry.total_requests += 1,
            CircuitBreakerEvent::RequestSuccess => entry.successful_requests += 1,
            CircuitBreakerEvent::RequestRejected => entry.rejected_requests += 1,
            CircuitBreakerEvent::StateChange(state) => {
                entry.state_changes += 1;
                entry.current_state = state;
            }
        }
    }

    // -- Scheduler -----------------------------------------------------------

    /// Record a scheduler admission decision.
    pub async fn record_scheduler_decision(&self, admitted: bool, wait_time: Duration) {
        let mut metrics = self.scheduler_metrics.write().await;
        metrics.total_admission_requests += 1;
        if admitted {
            metrics.admitted_count += 1;
        } else {
            metrics.rejected_count += 1;
        }
        metrics.queue_wait_time_ms +=
            u64::try_from(wait_time.as_millis()).unwrap_or(u64::MAX);
    }

    /// Return a snapshot of scheduler metrics.
    pub async fn get_scheduler_metrics(&self) -> SchedulerMetrics {
        self.scheduler_metrics.read().await.clone()
    }

    // -- Retry ---------------------------------------------------------------

    /// Record a retry attempt for a given operation.
    pub async fn record_retry_attempt(
        &self,
        operation_id: &str,
        backoff_time: Duration,
        success: bool,
    ) {
        let mut metrics = self.retry_metrics.write().await;
        let entry = metrics
            .entry(operation_id.to_string())
            .or_insert_with(|| RetryMetrics {
                total_retries: 0,
                successful_retries: 0,
                exhausted_retries: 0,
                total_backoff_time_ms: 0,
            });

        entry.total_retries += 1;
        if success {
            entry.successful_retries += 1;
        } else {
            entry.exhausted_retries += 1;
        }
        entry.total_backoff_time_ms +=
            u64::try_from(backoff_time.as_millis()).unwrap_or(u64::MAX);
    }

    // -- Business metrics ----------------------------------------------------

    /// Record a custom business metric with arbitrary tags.
    pub async fn record_business_metric(
        &self,
        name: impl Into<String>,
        value: f64,
        tags: HashMap<String, String>,
    ) {
        let mut metrics = self.business_metrics.write().await;
        let timestamp_ms = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(u64::MAX);

        metrics.push(BusinessMetrics {
            metric_name: name.into(),
            metric_value: value,
            tags,
            timestamp_ms,
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_metrics_recording() {
        let collector = MetricsCollector::new();

        collector
            .record_agent_execution(
                "agent-1",
                Duration::from_millis(100),
                true,
                Some(TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                    cost_estimate: 0.001,
                }),
            )
            .await;

        let metrics = collector.get_agent_metrics().await;
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].total_executions, 1);
        assert_eq!(metrics[0].successful_executions, 1);
        assert_eq!(metrics[0].failed_executions, 0);
        assert_eq!(metrics[0].token_usage.total_tokens, 150);
    }

    #[tokio::test]
    async fn test_agent_metrics_failure() {
        let collector = MetricsCollector::new();

        collector
            .record_agent_execution("agent-fail", Duration::from_millis(50), false, None)
            .await;

        let metrics = collector.get_agent_metrics().await;
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].failed_executions, 1);
        assert_eq!(metrics[0].successful_executions, 0);
    }

    #[tokio::test]
    async fn test_tool_metrics_recording() {
        let collector = MetricsCollector::new();

        collector
            .record_tool_execution("http_fetch", Duration::from_millis(50), true)
            .await;

        let metrics = collector.get_tool_metrics().await;
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].total_calls, 1);
        assert_eq!(metrics[0].successful_calls, 1);
    }

    #[tokio::test]
    async fn test_tool_metrics_average() {
        let collector = MetricsCollector::new();

        collector
            .record_tool_execution("search", Duration::from_millis(100), true)
            .await;
        collector
            .record_tool_execution("search", Duration::from_millis(200), true)
            .await;

        let metrics = collector.get_tool_metrics().await;
        assert_eq!(metrics[0].total_calls, 2);
        assert!((metrics[0].average_execution_time_ms - 150.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_routing_metrics() {
        let collector = MetricsCollector::new();

        collector.record_routing_decision(true).await;
        collector.record_routing_decision(false).await;
        collector.record_routing_decision(true).await;

        let metrics = collector.get_routing_metrics().await;
        assert_eq!(metrics.total_routing_decisions, 3);
        assert_eq!(metrics.local_routing_count, 2);
        assert_eq!(metrics.cloud_routing_count, 1);
    }

    #[tokio::test]
    async fn test_model_pool_metrics() {
        let collector = MetricsCollector::new();

        collector
            .record_model_pool_event(ModelPoolEvent::CapacitySet(3))
            .await;
        collector
            .record_model_pool_event(ModelPoolEvent::ModelLoaded)
            .await;
        collector
            .record_model_pool_event(ModelPoolEvent::ModelLoaded)
            .await;
        collector
            .record_model_pool_event(ModelPoolEvent::ModelEvicted)
            .await;

        let m = collector.get_model_pool_metrics().await;
        assert_eq!(m.max_capacity, 3);
        assert_eq!(m.total_models_loaded, 2);
        assert_eq!(m.total_models_evicted, 1);
        assert_eq!(m.current_load, 1);
        assert_eq!(m.eviction_count, 1);
    }

    #[tokio::test]
    async fn test_scheduler_metrics() {
        let collector = MetricsCollector::new();

        collector
            .record_scheduler_decision(true, Duration::from_millis(10))
            .await;
        collector
            .record_scheduler_decision(false, Duration::from_millis(50))
            .await;

        let m = collector.get_scheduler_metrics().await;
        assert_eq!(m.total_admission_requests, 2);
        assert_eq!(m.admitted_count, 1);
        assert_eq!(m.rejected_count, 1);
        assert_eq!(m.queue_wait_time_ms, 60);
    }

    #[tokio::test]
    async fn test_retry_metrics() {
        let collector = MetricsCollector::new();

        collector
            .record_retry_attempt("op-1", Duration::from_millis(100), false)
            .await;
        collector
            .record_retry_attempt("op-1", Duration::from_millis(200), true)
            .await;

        let metrics = collector.retry_metrics.read().await;
        let m = metrics.get("op-1").unwrap();
        assert_eq!(m.total_retries, 2);
        assert_eq!(m.successful_retries, 1);
        assert_eq!(m.exhausted_retries, 1);
        assert_eq!(m.total_backoff_time_ms, 300);
    }

    #[tokio::test]
    async fn test_business_metric() {
        let collector = MetricsCollector::new();

        let mut tags = HashMap::new();
        tags.insert("region".to_string(), "us-east".to_string());

        collector
            .record_business_metric("custom_score", 42.5, tags)
            .await;

        let all = collector.business_metrics.read().await;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].metric_name, "custom_score");
        assert!((all[0].metric_value - 42.5).abs() < f64::EPSILON);
        assert_eq!(all[0].tags.get("region").unwrap(), "us-east");
        assert!(all[0].timestamp_ms > 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_metrics() {
        let collector = MetricsCollector::new();

        collector
            .record_circuit_breaker_event("cb-1", CircuitBreakerEvent::RequestAttempt)
            .await;
        collector
            .record_circuit_breaker_event("cb-1", CircuitBreakerEvent::RequestSuccess)
            .await;
        collector
            .record_circuit_breaker_event(
                "cb-1",
                CircuitBreakerEvent::StateChange(CircuitBreakerState::Open),
            )
            .await;

        let metrics = collector.circuit_breaker_metrics.read().await;
        let m = metrics.get("cb-1").unwrap();
        assert_eq!(m.total_requests, 1);
        assert_eq!(m.successful_requests, 1);
        assert_eq!(m.state_changes, 1);
        assert_eq!(m.current_state, CircuitBreakerState::Open);
    }

    #[tokio::test]
    async fn test_default_collector() {
        let collector = MetricsCollector::default();
        let metrics = collector.get_agent_metrics().await;
        assert!(metrics.is_empty());
    }

    #[tokio::test]
    async fn test_metric_builder() {
        let (agent, tool, tags) = MetricBuilder::new()
            .with_agent("agent-1")
            .with_tool("search")
            .with_tag("env", "prod")
            .build();

        assert_eq!(agent, Some("agent-1".to_string()));
        assert_eq!(tool, Some("search".to_string()));
        assert_eq!(tags.get("env").unwrap(), "prod");
    }
}
