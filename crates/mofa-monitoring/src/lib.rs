//! MoFA Monitoring - Web-based dashboard and metrics collection
//!
//! This crate provides monitoring capabilities for MoFA agents:
//! - Web dashboard for real-time visualization
//! - Metrics collection and aggregation
//! - REST API for integration
//! - WebSocket for live updates
//!
//! # Example
//!
//! ```rust,no_run
//! use mofa_monitoring::{DashboardServer, DashboardConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = DashboardConfig::new()
//!     .with_port(8080);
//!
//! let server = DashboardServer::new(config);
//! server.start().await?;
//! # Ok(())
//! # }
//! ```

mod dashboard;
pub mod tracing;

pub use dashboard::{
    AgentMetrics, AgentStatus, ApiError, ApiResponse, DashboardConfig, DashboardServer, Gauge,
    Histogram, MetricType, MetricValue, MetricsCollector, MetricsConfig, MetricsRegistry,
    MetricsSnapshot, PluginMetrics, PluginStatus, ServerState, SystemMetrics, SystemStatus,
    WebSocketClient, WebSocketHandler, WebSocketMessage, WorkflowMetrics,
};
