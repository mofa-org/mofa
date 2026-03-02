//! REST API endpoints for the dashboard
//!
//! Provides REST API for monitoring data access

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::metrics::{
    AgentMetrics, LLMMetrics, MetricsCollector, MetricsSnapshot, PluginMetrics, WorkflowMetrics,
};

use mofa_kernel::workflow::telemetry::{DebugEvent, SessionRecorder};

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: u64,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn error(message: &str) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message.to_string()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// API error type
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(ApiResponse::<()>::error(&message));
        (status, body).into_response()
    }
}

/// Agent status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub name: String,
    pub state: String,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tasks_in_progress: u32,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub health: String,
    pub last_activity: u64,
}

impl From<AgentMetrics> for AgentStatus {
    fn from(m: AgentMetrics) -> Self {
        let health = if m.tasks_failed == 0 {
            "healthy"
        } else if m.tasks_failed as f64 / (m.tasks_completed.max(1) as f64) < 0.1 {
            "degraded"
        } else {
            "unhealthy"
        };

        Self {
            agent_id: m.agent_id,
            name: m.name,
            state: m.state,
            tasks_completed: m.tasks_completed,
            tasks_failed: m.tasks_failed,
            tasks_in_progress: m.tasks_in_progress,
            messages_sent: m.messages_sent,
            messages_received: m.messages_received,
            health: health.to_string(),
            last_activity: m.last_activity,
        }
    }
}

/// Workflow status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub workflow_id: String,
    pub name: String,
    pub status: String,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub success_rate: f64,
    pub avg_execution_time_ms: f64,
    pub running_instances: u32,
}

impl From<WorkflowMetrics> for WorkflowStatus {
    fn from(m: WorkflowMetrics) -> Self {
        let success_rate = if m.total_executions > 0 {
            (m.successful_executions as f64 / m.total_executions as f64) * 100.0
        } else {
            0.0
        };

        Self {
            workflow_id: m.workflow_id,
            name: m.name,
            status: m.status,
            total_executions: m.total_executions,
            successful_executions: m.successful_executions,
            failed_executions: m.failed_executions,
            success_rate,
            avg_execution_time_ms: m.avg_execution_time_ms,
            running_instances: m.running_instances,
        }
    }
}

/// Plugin status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStatus {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub state: String,
    pub call_count: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub avg_response_time_ms: f64,
    pub reload_count: u32,
}

impl From<PluginMetrics> for PluginStatus {
    fn from(m: PluginMetrics) -> Self {
        let error_rate = if m.call_count > 0 {
            (m.error_count as f64 / m.call_count as f64) * 100.0
        } else {
            0.0
        };

        Self {
            plugin_id: m.plugin_id,
            name: m.name,
            version: m.version,
            state: m.state,
            call_count: m.call_count,
            error_count: m.error_count,
            error_rate,
            avg_response_time_ms: m.avg_response_time_ms,
            reload_count: m.reload_count,
        }
    }
}

/// LLM status response - specialized for model inference metrics
///
/// Separate from PluginStatus because LLM metrics include model-specific
/// inference data (tokens/s, TTFT, token counts) that don't apply to
/// generic plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMStatus {
    pub plugin_id: String,
    pub provider_name: String,
    pub model_name: String,
    pub state: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub total_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub avg_latency_ms: f64,
    pub tokens_per_second: Option<f64>,
    pub time_to_first_token_ms: Option<f64>,
    pub requests_per_minute: f64,
    pub error_rate: f64,
}

impl From<LLMMetrics> for LLMStatus {
    fn from(m: LLMMetrics) -> Self {
        let success_rate = if m.total_requests > 0 {
            (m.successful_requests as f64 / m.total_requests as f64) * 100.0
        } else {
            0.0
        };

        Self {
            plugin_id: m.plugin_id,
            provider_name: m.provider_name,
            model_name: m.model_name,
            state: m.state,
            total_requests: m.total_requests,
            successful_requests: m.successful_requests,
            failed_requests: m.failed_requests,
            success_rate,
            total_tokens: m.total_tokens,
            prompt_tokens: m.prompt_tokens,
            completion_tokens: m.completion_tokens,
            avg_latency_ms: m.avg_latency_ms,
            tokens_per_second: m.tokens_per_second,
            time_to_first_token_ms: m.time_to_first_token_ms,
            requests_per_minute: m.requests_per_minute,
            error_rate: m.error_rate,
        }
    }
}

/// System status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub status: String,
    pub uptime_secs: u64,
    pub cpu_usage: f64,
    pub memory_usage_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub agent_count: usize,
    pub workflow_count: usize,
    pub plugin_count: usize,
    pub healthy_agents: usize,
    pub running_workflows: usize,
}

/// Dashboard overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardOverview {
    pub system: SystemStatus,
    pub agents_summary: AgentsSummary,
    pub workflows_summary: WorkflowsSummary,
    pub plugins_summary: PluginsSummary,
    pub llm_summary: LLMSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsSummary {
    pub total: usize,
    pub running: usize,
    pub idle: usize,
    pub error: usize,
    pub total_tasks_completed: u64,
    pub total_messages: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowsSummary {
    pub total: usize,
    pub running: usize,
    pub total_executions: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsSummary {
    pub total: usize,
    pub loaded: usize,
    pub failed: usize,
    pub total_calls: u64,
}

/// LLM summary for dashboard overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMSummary {
    pub total_plugins: usize,
    pub active_models: usize,
    pub total_requests: u64,
    pub total_tokens: u64,
    pub avg_latency_ms: f64,
    pub avg_tokens_per_second: f64,
    pub total_errors: u64,
}

/// Query parameters for history endpoint
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
    pub from: Option<u64>,
    pub to: Option<u64>,
}

/// API state
pub struct ApiState {
    pub collector: Arc<MetricsCollector>,
    /// Optional session recorder for debug sessions
    pub session_recorder: Option<Arc<dyn SessionRecorder>>,
}

/// Create API router
pub fn create_api_router(
    collector: Arc<MetricsCollector>,
    session_recorder: Option<Arc<dyn SessionRecorder>>,
) -> Router {
    let state = Arc::new(ApiState {
        collector,
        session_recorder,
    });

    Router::new()
        // Overview
        .route("/overview", get(get_overview))
        // Metrics
        .route("/metrics", get(get_metrics))
        .route("/metrics/history", get(get_metrics_history))
        .route("/metrics/custom", get(get_custom_metrics))
        // Agents
        .route("/agents", get(get_agents))
        .route("/agents/{id}", get(get_agent))
        // Workflows
        .route("/workflows", get(get_workflows))
        .route("/workflows/{id}", get(get_workflow))
        // Plugins
        .route("/plugins", get(get_plugins))
        .route("/plugins/{id}", get(get_plugin))
        // Debug sessions
        .route("/debug/sessions", get(get_debug_sessions))
        .route("/debug/sessions/{id}", get(get_debug_session))
        .route("/debug/sessions/{id}/events", get(get_debug_session_events))
        // LLM
        .route("/llm", get(get_llm_metrics))
        .route("/llm/{id}", get(get_llm_plugin))
        // System
        .route("/system", get(get_system_status))
        .route("/health", get(health_check))
        .with_state(state)
}

/// Get dashboard overview
async fn get_overview(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<DashboardOverview>>, ApiError> {
    let snapshot = state.collector.current().await;

    // Calculate summaries
    let agents_summary = AgentsSummary {
        total: snapshot.agents.len(),
        running: snapshot
            .agents
            .iter()
            .filter(|a| a.state == "running")
            .count(),
        idle: snapshot.agents.iter().filter(|a| a.state == "idle").count(),
        error: snapshot
            .agents
            .iter()
            .filter(|a| a.state == "error")
            .count(),
        total_tasks_completed: snapshot.agents.iter().map(|a| a.tasks_completed).sum(),
        total_messages: snapshot
            .agents
            .iter()
            .map(|a| a.messages_sent + a.messages_received)
            .sum(),
    };

    let workflows_summary = WorkflowsSummary {
        total: snapshot.workflows.len(),
        running: snapshot
            .workflows
            .iter()
            .filter(|w| w.status == "running")
            .count(),
        total_executions: snapshot.workflows.iter().map(|w| w.total_executions).sum(),
        success_rate: {
            let total: u64 = snapshot.workflows.iter().map(|w| w.total_executions).sum();
            let success: u64 = snapshot
                .workflows
                .iter()
                .map(|w| w.successful_executions)
                .sum();
            if total > 0 {
                (success as f64 / total as f64) * 100.0
            } else {
                0.0
            }
        },
    };

    let plugins_summary = PluginsSummary {
        total: snapshot.plugins.len(),
        loaded: snapshot
            .plugins
            .iter()
            .filter(|p| p.state == "running" || p.state == "loaded")
            .count(),
        failed: snapshot
            .plugins
            .iter()
            .filter(|p| p.state.starts_with("failed"))
            .count(),
        total_calls: snapshot.plugins.iter().map(|p| p.call_count).sum(),
    };

    // Calculate LLM summary
    let llm_summary = {
        let total_requests: u64 = snapshot.llm_metrics.iter().map(|l| l.total_requests).sum();
        let total_tokens: u64 = snapshot.llm_metrics.iter().map(|l| l.total_tokens).sum();
        let total_errors: u64 = snapshot.llm_metrics.iter().map(|l| l.failed_requests).sum();
        let avg_latency = if !snapshot.llm_metrics.is_empty() {
            snapshot
                .llm_metrics
                .iter()
                .map(|l| l.avg_latency_ms)
                .sum::<f64>()
                / snapshot.llm_metrics.len() as f64
        } else {
            0.0
        };
        let avg_tps = if !snapshot.llm_metrics.is_empty() {
            snapshot
                .llm_metrics
                .iter()
                .filter_map(|l| l.tokens_per_second)
                .sum::<f64>()
                / snapshot
                    .llm_metrics
                    .iter()
                    .filter(|l| l.tokens_per_second.is_some())
                    .count()
                    .max(1) as f64
        } else {
            0.0
        };

        LLMSummary {
            total_plugins: snapshot.llm_metrics.len(),
            active_models: snapshot
                .llm_metrics
                .iter()
                .filter(|l| l.state == "running")
                .count(),
            total_requests,
            total_tokens,
            avg_latency_ms: avg_latency,
            avg_tokens_per_second: avg_tps,
            total_errors,
        }
    };

    let system = SystemStatus {
        status: "operational".to_string(),
        uptime_secs: snapshot.system.uptime_secs,
        cpu_usage: snapshot.system.cpu_usage,
        memory_usage_percent: if snapshot.system.memory_total > 0 {
            (snapshot.system.memory_used as f64 / snapshot.system.memory_total as f64) * 100.0
        } else {
            0.0
        },
        memory_used_bytes: snapshot.system.memory_used,
        memory_total_bytes: snapshot.system.memory_total,
        agent_count: snapshot.agents.len(),
        workflow_count: snapshot.workflows.len(),
        plugin_count: snapshot.plugins.len(),
        healthy_agents: agents_summary.running + agents_summary.idle,
        running_workflows: workflows_summary.running,
    };

    let overview = DashboardOverview {
        system,
        agents_summary,
        workflows_summary,
        plugins_summary,
        llm_summary,
    };

    Ok(Json(ApiResponse::success(overview)))
}

/// Get current metrics
async fn get_metrics(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<MetricsSnapshot>>, ApiError> {
    let snapshot = state.collector.current().await;
    Ok(Json(ApiResponse::success(snapshot)))
}

/// Get metrics history
async fn get_metrics_history(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<ApiResponse<Vec<MetricsSnapshot>>>, ApiError> {
    let mut history = state.collector.history(params.limit).await;

    // Filter by time range if specified
    if let Some(from) = params.from {
        history.retain(|s| s.timestamp >= from);
    }
    if let Some(to) = params.to {
        history.retain(|s| s.timestamp <= to);
    }

    Ok(Json(ApiResponse::success(history)))
}

/// Get custom metrics
async fn get_custom_metrics(
    State(state): State<Arc<ApiState>>,
) -> Result<
    Json<ApiResponse<std::collections::HashMap<String, super::metrics::MetricValue>>>,
    ApiError,
> {
    let snapshot = state.collector.current().await;
    Ok(Json(ApiResponse::success(snapshot.custom)))
}

/// Get all agents
async fn get_agents(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<Vec<AgentStatus>>>, ApiError> {
    let snapshot = state.collector.current().await;
    let agents: Vec<AgentStatus> = snapshot.agents.into_iter().map(|a| a.into()).collect();
    Ok(Json(ApiResponse::success(agents)))
}

/// Get single agent
async fn get_agent(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<AgentStatus>>, ApiError> {
    let snapshot = state.collector.current().await;
    let agent = snapshot
        .agents
        .into_iter()
        .find(|a| a.agent_id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Agent {} not found", id)))?;

    Ok(Json(ApiResponse::success(agent.into())))
}

/// Get all workflows
async fn get_workflows(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<Vec<WorkflowStatus>>>, ApiError> {
    let snapshot = state.collector.current().await;
    let workflows: Vec<WorkflowStatus> = snapshot.workflows.into_iter().map(|w| w.into()).collect();
    Ok(Json(ApiResponse::success(workflows)))
}

/// Get single workflow
async fn get_workflow(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<WorkflowStatus>>, ApiError> {
    let snapshot = state.collector.current().await;
    let workflow = snapshot
        .workflows
        .into_iter()
        .find(|w| w.workflow_id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Workflow {} not found", id)))?;

    Ok(Json(ApiResponse::success(workflow.into())))
}

/// Get all plugins
async fn get_plugins(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<Vec<PluginStatus>>>, ApiError> {
    let snapshot = state.collector.current().await;
    let plugins: Vec<PluginStatus> = snapshot.plugins.into_iter().map(|p| p.into()).collect();
    Ok(Json(ApiResponse::success(plugins)))
}

/// Get single plugin
async fn get_plugin(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<PluginStatus>>, ApiError> {
    let snapshot = state.collector.current().await;
    let plugin = snapshot
        .plugins
        .into_iter()
        .find(|p| p.plugin_id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Plugin {} not found", id)))?;

    Ok(Json(ApiResponse::success(plugin.into())))
}

/// Get all LLM metrics
async fn get_llm_metrics(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<Vec<LLMStatus>>>, ApiError> {
    let snapshot = state.collector.current().await;
    let llm: Vec<LLMStatus> = snapshot.llm_metrics.into_iter().map(|l| l.into()).collect();
    Ok(Json(ApiResponse::success(llm)))
}

/// Get single LLM plugin metrics
async fn get_llm_plugin(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<LLMStatus>>, ApiError> {
    let snapshot = state.collector.current().await;
    let llm = snapshot
        .llm_metrics
        .into_iter()
        .find(|l| l.plugin_id == id)
        .ok_or_else(|| ApiError::NotFound(format!("LLM plugin {} not found", id)))?;

    Ok(Json(ApiResponse::success(llm.into())))
}

/// Get system status
async fn get_system_status(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<SystemStatus>>, ApiError> {
    let snapshot = state.collector.current().await;

    let status = SystemStatus {
        status: "operational".to_string(),
        uptime_secs: snapshot.system.uptime_secs,
        cpu_usage: snapshot.system.cpu_usage,
        memory_usage_percent: if snapshot.system.memory_total > 0 {
            (snapshot.system.memory_used as f64 / snapshot.system.memory_total as f64) * 100.0
        } else {
            0.0
        },
        memory_used_bytes: snapshot.system.memory_used,
        memory_total_bytes: snapshot.system.memory_total,
        agent_count: snapshot.agents.len(),
        workflow_count: snapshot.workflows.len(),
        plugin_count: snapshot.plugins.len(),
        healthy_agents: snapshot
            .agents
            .iter()
            .filter(|a| a.state != "error")
            .count(),
        running_workflows: snapshot
            .workflows
            .iter()
            .filter(|w| w.status == "running")
            .count(),
    };

    Ok(Json(ApiResponse::success(status)))
}

/// Health check endpoint
async fn health_check() -> Result<Json<ApiResponse<HealthStatus>>, ApiError> {
    Ok(Json(ApiResponse::success(HealthStatus {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })))
}

#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
}

// ============================================================================
// Debug Session API Handlers
// ============================================================================

/// Get all debug sessions
async fn get_debug_sessions(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiResponse<Vec<DebugSessionResponse>>>, ApiError> {
    let recorder = state.session_recorder.as_ref().ok_or_else(|| {
        ApiError::BadRequest("Debug session recording is not enabled".to_string())
    })?;

    let sessions = recorder
        .list_sessions()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response: Vec<DebugSessionResponse> = sessions
        .into_iter()
        .map(|s| DebugSessionResponse {
            session_id: s.session_id,
            workflow_id: s.workflow_id,
            execution_id: s.execution_id,
            started_at: s.started_at,
            ended_at: s.ended_at,
            status: s.status,
            event_count: s.event_count,
        })
        .collect();

    Ok(Json(ApiResponse::success(response)))
}

/// Get a specific debug session by ID
async fn get_debug_session(
    State(state): State<Arc<ApiState>>,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<DebugSessionResponse>>, ApiError> {
    let recorder = state.session_recorder.as_ref().ok_or_else(|| {
        ApiError::BadRequest("Debug session recording is not enabled".to_string())
    })?;

    let session = recorder
        .get_session(&session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("Session not found: {}", session_id)))?;

    let response = DebugSessionResponse {
        session_id: session.session_id,
        workflow_id: session.workflow_id,
        execution_id: session.execution_id,
        started_at: session.started_at,
        ended_at: session.ended_at,
        status: session.status,
        event_count: session.event_count,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Get all events for a debug session
async fn get_debug_session_events(
    State(state): State<Arc<ApiState>>,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<DebugEvent>>>, ApiError> {
    let recorder = state.session_recorder.as_ref().ok_or_else(|| {
        ApiError::BadRequest("Debug session recording is not enabled".to_string())
    })?;

    // First check if session exists
    let _session = recorder
        .get_session(&session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("Session not found: {}", session_id)))?;

    let events = recorder
        .get_events(&session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ApiResponse::success(events)))
}

/// Debug session response type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSessionResponse {
    pub session_id: String,
    pub workflow_id: String,
    pub execution_id: String,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub status: String,
    pub event_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_from_metrics() {
        let metrics = AgentMetrics {
            agent_id: "agent-1".to_string(),
            name: "Test Agent".to_string(),
            state: "running".to_string(),
            tasks_completed: 100,
            tasks_failed: 5,
            ..Default::default()
        };

        let status: AgentStatus = metrics.into();
        assert_eq!(status.agent_id, "agent-1");
        assert_eq!(status.health, "degraded"); // 5% error rate
    }

    #[test]
    fn test_workflow_status_success_rate() {
        let metrics = WorkflowMetrics {
            workflow_id: "wf-1".to_string(),
            total_executions: 100,
            successful_executions: 95,
            failed_executions: 5,
            ..Default::default()
        };

        let status: WorkflowStatus = metrics.into();
        assert_eq!(status.success_rate, 95.0);
    }
}
