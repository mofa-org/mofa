//! Web-based monitoring dashboard module
//!
//! Provides a web dashboard for monitoring MoFA:
//! - Real-time metrics visualization
//! - Agent status monitoring
//! - Workflow execution tracking
//! - Plugin health monitoring
//! - System resource usage
//! - REST API for integration
//! - WebSocket for live updates

mod api;
mod assets;
mod metrics;
mod server;
mod websocket;

pub use api::{
    AgentStatus, ApiError, ApiResponse, PluginStatus, SystemStatus,
};
pub use metrics::{
    AgentMetrics, Gauge, Histogram, MetricType, MetricValue, MetricsCollector,
    MetricsConfig, MetricsRegistry, MetricsSnapshot, PluginMetrics, SystemMetrics, WorkflowMetrics,
};
pub use server::{DashboardConfig, DashboardServer, ServerState};
pub use websocket::{WebSocketClient, WebSocketHandler, WebSocketMessage};
