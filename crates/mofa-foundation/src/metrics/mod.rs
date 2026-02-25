//! Metrics and Telemetry Module
//!
//! This module provides comprehensive metrics and telemetry for tracking agent execution,
//! including execution time, latency percentiles, token usage, tool success/failure rates,
//! memory and CPU utilization, workflow step timing, and custom business metrics.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Agent execution metrics
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

/// Token usage tracking
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost_estimate: f64,
}

/// Latency percentiles (p50, p90, p95, p99)
#[derive(Debug, Clone, Default)]
pub struct LatencyPercentiles {
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

/// Tool execution metrics
#[derive(Debug, Clone)]
pub struct ToolMetrics {
    pub tool_name: String,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub average_execution_time_ms: f64,
    pub total_execution_time_ms: u64,
}

/// Workflow execution metrics
#[derive(Debug, Clone)]
pub struct WorkflowMetrics {
    pub workflow_id: String,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub step_timings: Vec<StepTiming>,
    pub total_duration_ms: u64,
}

/// Timing for individual workflow steps
#[derive(Debug, Clone)]
pub struct StepTiming {
    pub step_name: String,
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub status: StepStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Routing metrics for tracking local vs cloud decisions
#[derive(Debug, Clone, Default)]
pub struct RoutingMetrics {
    pub total_routing_decisions: u64,
    pub local_routing_count: u64,
    pub cloud_routing_count: u64,
    pub fallback_count: u64,
}

/// Model pool metrics for load/eviction events
#[derive(Debug, Clone, Default)]
pub struct ModelPoolMetrics {
    pub total_models_loaded: u64,
    pub total_models_evicted: u64,
    pub current_load: u64,
    pub max_capacity: u64,
    pub eviction_count: u64,
}

/// Circuit breaker event metrics
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub circuit_breaker_id: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub rejected_requests: u64,
    pub state_changes: u64,
    pub current_state: CircuitBreakerState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Scheduler admission metrics
#[derive(Debug, Clone, Default)]
pub struct SchedulerMetrics {
    pub total_admission_requests: u64,
    pub admitted_count: u64,
    pub rejected_count: u64,
    pub queue_wait_time_ms: u64,
}

/// Retry metrics
#[derive(Debug, Clone)]
pub struct RetryMetrics {
    pub total_retries: u64,
    pub successful_retries: u64,
    pub exhausted_retries: u64,
    pub total_backoff_time_ms: u64,
}

/// Custom business metrics via callbacks
#[derive(Debug, Clone)]
pub struct BusinessMetrics {
    pub metric_name: String,
    pub metric_value: f64,
    pub tags: HashMap<String, String>,
    pub timestamp_ms: u64,
}

/// Pluggable metrics backend trait
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

/// In-memory metrics collector
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

    /// Record agent execution metrics
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
        entry.total_execution_time_ms += duration.as_millis() as u64;
        
        if success {
            entry.successful_executions += 1;
        } else {
            entry.failed_executions += 1;
        }

        if let Some(token_usage) = tokens {
            entry.token_usage.prompt_tokens += token_usage.prompt_tokens;
            entry.token_usage.completion_tokens += token_usage.completion_tokens;
            entry.token_usage.total_tokens += token_usage.total_tokens;
            entry.token_usage.cost_estimate += token_usage.cost_estimate;
        }
    }

    /// Record tool execution metrics
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
        entry.total_execution_time_ms += duration.as_millis() as u64;
        
        if success {
            entry.successful_calls += 1;
        } else {
            entry.failed_calls += 1;
        }

        entry.average_execution_time_ms = 
            entry.total_execution_time_ms as f64 / entry.total_calls as f64;
    }

    /// Record routing decision
    pub async fn record_routing_decision(&self, is_local: bool) {
        let mut metrics = self.routing_metrics.write().await;
        metrics.total_routing_decisions += 1;
        if is_local {
            metrics.local_routing_count += 1;
        } else {
            metrics.cloud_routing_count += 1;
        }
    }

    /// Record model pool event
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

    /// Record circuit breaker state change
    pub async fn record_circuit_breaker_event(
        &self,
        cb_id: &str,
        event: CircuitBreakerEvent,
    ) {
        let mut metrics = self.circuit_breaker_metrics.write().await;
        let entry = metrics.entry(cb_id.to_string()).or_insert_with(|| CircuitBreakerMetrics {
            circuit_breaker_id: cb_id.to_string(),
            total_requests: 0,
            successful_requests: 0,
            rejected_requests: 0,
            state_changes: 0,
            current_state: CircuitBreakerState::Closed,
        });

        match event {
            CircuitBreakerEvent::RequestAttempt => {
                entry.total_requests += 1;
            }
            CircuitBreakerEvent::RequestSuccess => {
                entry.successful_requests += 1;
            }
            CircuitBreakerEvent::RequestRejected => {
                entry.rejected_requests += 1;
            }
            CircuitBreakerEvent::StateChange(state) => {
                entry.state_changes += 1;
                entry.current_state = state;
            }
        }
    }

    /// Record scheduler admission decision
    pub async fn record_scheduler_decision(&self, admitted: bool, wait_time: Duration) {
        let mut metrics = self.scheduler_metrics.write().await;
        metrics.total_admission_requests += 1;
        if admitted {
            metrics.admitted_count += 1;
        } else {
            metrics.rejected_count += 1;
        }
        metrics.queue_wait_time_ms += wait_time.as_millis() as u64;
    }

    /// Record retry attempt
    pub async fn record_retry_attempt(
        &self,
        operation_id: &str,
        backoff_time: Duration,
        success: bool,
    ) {
        let mut metrics = self.retry_metrics.write().await;
        let entry = metrics.entry(operation_id.to_string()).or_insert_with(|| RetryMetrics {
            total_retries: 0,
            successful_retries: 0,
            exhausted_retries: 0,
            total_backoff_time_ms: 0,
        });

        if success {
            entry.successful_retries += 1;
        } else {
            entry.exhausted_retries += 1;
        }
        entry.total_retries += 1;
        entry.total_backoff_time_ms += backoff_time.as_millis() as u64;
    }

    /// Record custom business metric
    pub async fn record_business_metric(&self, name: String, value: f64, tags: HashMap<String, String>) {
        let mut metrics = self.business_metrics.write().await;
        metrics.push(BusinessMetrics {
            metric_name: name,
            metric_value: value,
            tags,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        });
    }

    /// Get all agent metrics
    pub async fn get_agent_metrics(&self) -> Vec<AgentMetrics> {
        let metrics = self.agent_metrics.read().await;
        metrics.values().cloned().collect()
    }

    /// Get all tool metrics
    pub async fn get_tool_metrics(&self) -> Vec<ToolMetrics> {
        let metrics = self.tool_metrics.read().await;
        metrics.values().cloned().collect()
    }

    /// Get routing metrics
    pub async fn get_routing_metrics(&self) -> RoutingMetrics {
        self.routing_metrics.read().await.clone()
    }

    /// Get model pool metrics
    pub async fn get_model_pool_metrics(&self) -> ModelPoolMetrics {
        self.model_pool_metrics.read().await.clone()
    }

    /// Get scheduler metrics
    pub async fn get_scheduler_metrics(&self) -> SchedulerMetrics {
        self.scheduler_metrics.read().await.clone()
    }
}

#[derive(Debug, Clone)]
pub enum ModelPoolEvent {
    ModelLoaded,
    ModelEvicted,
    CapacitySet(u64),
}

#[derive(Debug, Clone)]
pub enum CircuitBreakerEvent {
    RequestAttempt,
    RequestSuccess,
    RequestRejected,
    StateChange(CircuitBreakerState),
}

/// Builder for creating metric records
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_metrics_recording() {
        let collector = MetricsCollector::new();
        
        collector.record_agent_execution(
            "agent-1",
            Duration::from_millis(100),
            true,
            Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                cost_estimate: 0.001,
            }),
        ).await;

        let metrics = collector.get_agent_metrics().await;
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].total_executions, 1);
        assert_eq!(metrics[0].successful_executions, 1);
        assert_eq!(metrics[0].token_usage.total_tokens, 150);
    }

    #[tokio::test]
    async fn test_tool_metrics_recording() {
        let collector = MetricsCollector::new();
        
        collector.record_tool_execution(
            "http_fetch",
            Duration::from_millis(50),
            true,
        ).await;

        let metrics = collector.get_tool_metrics().await;
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].total_calls, 1);
        assert_eq!(metrics[0].successful_calls, 1);
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
}
