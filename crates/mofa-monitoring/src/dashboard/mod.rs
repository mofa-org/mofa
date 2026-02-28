//! Web-based monitoring dashboard module
//!
//! Provides a web dashboard for monitoring MoFA:
//! - Real-time metrics visualization
//! - Agent status monitoring
//! - Workflow execution tracking
//! - Plugin health monitoring
//! - LLM model inference metrics (per-model monitoring for dashboard)
//! - System resource usage
//! - REST API for integration
//! - WebSocket for live updates

mod api;
mod assets;
pub mod auth;
mod metrics;
mod server;
mod websocket;

pub use api::{
    AgentStatus, ApiError, ApiResponse, DebugSessionResponse, LLMStatus, LLMSummary, PluginStatus,
    SystemStatus,
};
pub use auth::{AuthInfo, AuthProvider, NoopAuthProvider, TokenAuthProvider};
pub use metrics::{
    AgentMetrics, Gauge, Histogram, LLMMetrics, MetricType, MetricValue, MetricsCollector,
    MetricsConfig, MetricsRegistry, MetricsSnapshot, PluginMetrics, SystemMetrics, WorkflowMetrics,
};
pub use server::{DashboardConfig, DashboardServer, ServerState};
pub use websocket::{WebSocketClient, WebSocketHandler, WebSocketMessage};
