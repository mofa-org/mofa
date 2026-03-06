//! Serialisable response types for the admin API.

use serde::{Deserialize, Serialize};

/// A single route entry returned by `GET /admin/routes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminRouteEntry {
    /// Stable route identifier.
    pub id: String,
    /// URL path pattern.
    pub path_pattern: String,
    /// Target agent ID.
    pub agent_id: String,
    /// HTTP method string (e.g. `"POST"`).
    pub method: String,
    /// Numeric priority — higher values are evaluated first.
    pub priority: i32,
    /// Whether the route is currently enabled.
    pub enabled: bool,
    /// Requests served by this route since gateway startup.
    pub requests_served: u64,
}

/// Request body for `POST /admin/routes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRouteRequest {
    /// Unique route identifier.
    pub id: String,
    /// URL path pattern (must start with `/`).
    pub path_pattern: String,
    /// Target agent ID (must exist in the agent registry).
    pub agent_id: String,
    /// HTTP method string, e.g. `"POST"`.
    pub method: String,
    /// Numeric priority. Defaults to `0`.
    #[serde(default)]
    pub priority: i32,
    /// Whether the route starts enabled. Defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Response body for `GET /admin/health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayHealthSummary {
    /// Total uptime in seconds since the admin server started.
    pub uptime_secs: u64,
    /// Total requests served across all routes since startup.
    pub total_requests: u64,
    /// Number of currently registered routes.
    pub routes_count: usize,
    /// Whether the hot-reload file watcher is active.
    pub hot_reload_active: bool,
    /// Gateway version string.
    pub version: String,
}
