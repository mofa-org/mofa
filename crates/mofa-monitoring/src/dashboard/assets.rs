//! Embedded dashboard assets
//!
//! Contains the HTML, CSS, and JavaScript for the dashboard UI

use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    response::IntoResponse,
};
use rust_embed::RustEmbed;

/// Embedded dashboard assets
#[derive(RustEmbed)]
#[folder = "src/dashboard/static"]
#[prefix = ""]
pub struct DashboardAssets;

impl DashboardAssets {
    /// Get asset by path
    pub fn get_asset(path: &str) -> Option<rust_embed::EmbeddedFile> {
        Self::get(path)
    }

    /// Get index.html
    pub fn index_html() -> &'static str {
        INDEX_HTML
    }

    /// Get styles.css
    pub fn styles_css() -> &'static str {
        STYLES_CSS
    }

    /// Get app.js
    pub fn app_js() -> &'static str {
        APP_JS
    }
}

/// Serve embedded asset or return 404
pub async fn serve_asset(path: String) -> impl IntoResponse {
    let path = if path.is_empty() || path == "/" {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };

    // Check embedded assets first
    if let Some(file) = DashboardAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(file.data.into_owned()))
            .unwrap();
    }

    // Fallback to hardcoded assets
    match path {
        "index.html" | "" => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(INDEX_HTML))
            .unwrap(),
        "styles.css" => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/css")
            .body(Body::from(STYLES_CSS))
            .unwrap(),
        "app.js" => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/javascript")
            .body(Body::from(APP_JS))
            .unwrap(),
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}

/// Index HTML
pub const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>MoFA Monitoring Dashboard</title>
    <link rel="stylesheet" href="/styles.css">
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
</head>
<body>
    <div class="dashboard">
        <!-- Header -->
        <header class="header">
            <div class="logo">
                <h1>🤖 MoFA Dashboard</h1>
            </div>
            <div class="header-info">
                <span id="connection-status" class="status-badge disconnected">Disconnected</span>
                <span id="last-update">Last update: --</span>
            </div>
        </header>

        <!-- Main Content -->
        <main class="main-content">
            <!-- System Overview Cards -->
            <section class="overview-section">
                <h2>System Overview</h2>
                <div class="cards-grid">
                    <div class="card">
                        <div class="card-icon">⏱️</div>
                        <div class="card-content">
                            <h3>Uptime</h3>
                            <p id="uptime" class="metric-value">--</p>
                        </div>
                    </div>
                    <div class="card">
                        <div class="card-icon">🤖</div>
                        <div class="card-content">
                            <h3>Agents</h3>
                            <p id="agent-count" class="metric-value">--</p>
                            <span id="agent-status" class="metric-sub">-- running</span>
                        </div>
                    </div>
                    <div class="card">
                        <div class="card-icon">📊</div>
                        <div class="card-content">
                            <h3>Workflows</h3>
                            <p id="workflow-count" class="metric-value">--</p>
                            <span id="workflow-status" class="metric-sub">-- active</span>
                        </div>
                    </div>
                    <div class="card">
                        <div class="card-icon">🔌</div>
                        <div class="card-content">
                            <h3>Plugins</h3>
                            <p id="plugin-count" class="metric-value">--</p>
                            <span id="plugin-status" class="metric-sub">-- loaded</span>
                        </div>
                    </div>
                </div>
            </section>

            <!-- Charts Section -->
            <section class="charts-section">
                <div class="chart-container">
                    <h3>Task Activity</h3>
                    <canvas id="tasks-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Message Flow</h3>
                    <canvas id="messages-chart"></canvas>
                </div>
            </section>

            <!-- Agents Table -->
            <section class="table-section">
                <h2>Agents</h2>
                <table class="data-table" id="agents-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Name</th>
                            <th>State</th>
                            <th>Tasks Completed</th>
                            <th>Tasks Failed</th>
                            <th>Messages</th>
                            <th>Health</th>
                        </tr>
                    </thead>
                    <tbody id="agents-tbody">
                        <tr><td colspan="7" class="loading">Loading...</td></tr>
                    </tbody>
                </table>
            </section>

            <!-- Workflows Table -->
            <section class="table-section">
                <h2>Workflows</h2>
                <table class="data-table" id="workflows-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Name</th>
                            <th>Status</th>
                            <th>Executions</th>
                            <th>Success Rate</th>
                            <th>Avg Time</th>
                            <th>Running</th>
                        </tr>
                    </thead>
                    <tbody id="workflows-tbody">
                        <tr><td colspan="7" class="loading">Loading...</td></tr>
                    </tbody>
                </table>
            </section>

            <!-- Plugins Table -->
            <section class="table-section">
                <h2>Plugins</h2>
                <table class="data-table" id="plugins-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Name</th>
                            <th>Version</th>
                            <th>State</th>
                            <th>Calls</th>
                            <th>Error Rate</th>
                            <th>Reloads</th>
                        </tr>
                    </thead>
                    <tbody id="plugins-tbody">
                        <tr><td colspan="7" class="loading">Loading...</td></tr>
                    </tbody>
                </table>
            </section>
        </main>

        <!-- Footer -->
        <footer class="footer">
            <p>MoFA Monitoring Dashboard v0.1.0 | <a href="/api/health">API Health</a></p>
        </footer>
    </div>

    <script src="/app.js"></script>
</body>
</html>
"#;

/// CSS Styles
pub const STYLES_CSS: &str = r#"
:root {
    --bg-primary: #1a1a2e;
    --bg-secondary: #16213e;
    --bg-card: #0f3460;
    --text-primary: #eaeaea;
    --text-secondary: #a0a0a0;
    --accent-primary: #e94560;
    --accent-secondary: #0f9b0f;
    --accent-warning: #f39c12;
    --border-color: #2a2a4a;
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
    background-color: var(--bg-primary);
    color: var(--text-primary);
    line-height: 1.6;
}

.dashboard {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
}

/* Header */
.header {
    background-color: var(--bg-secondary);
    padding: 1rem 2rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-color);
}

.logo h1 {
    font-size: 1.5rem;
    font-weight: 600;
}

.header-info {
    display: flex;
    align-items: center;
    gap: 1rem;
}

.status-badge {
    padding: 0.25rem 0.75rem;
    border-radius: 1rem;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
}

.status-badge.connected {
    background-color: var(--accent-secondary);
    color: white;
}

.status-badge.disconnected {
    background-color: var(--accent-primary);
    color: white;
}

#last-update {
    color: var(--text-secondary);
    font-size: 0.875rem;
}

/* Main Content */
.main-content {
    flex: 1;
    padding: 2rem;
    max-width: 1600px;
    margin: 0 auto;
    width: 100%;
}

/* Section Titles */
section h2 {
    margin-bottom: 1rem;
    font-size: 1.25rem;
    color: var(--text-primary);
}

/* Cards Grid */
.cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1.5rem;
    margin-bottom: 2rem;
}

.card {
    background-color: var(--bg-card);
    border-radius: 0.75rem;
    padding: 1.5rem;
    display: flex;
    align-items: center;
    gap: 1rem;
    border: 1px solid var(--border-color);
    transition: transform 0.2s, box-shadow 0.2s;
}

.card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.card-icon {
    font-size: 2rem;
}

.card-content h3 {
    font-size: 0.875rem;
    color: var(--text-secondary);
    margin-bottom: 0.25rem;
}

.metric-value {
    font-size: 1.75rem;
    font-weight: 700;
    color: var(--text-primary);
}

.metric-sub {
    font-size: 0.75rem;
    color: var(--text-secondary);
}

/* Charts Section */
.charts-section {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
    gap: 1.5rem;
    margin-bottom: 2rem;
}

.chart-container {
    background-color: var(--bg-card);
    border-radius: 0.75rem;
    padding: 1.5rem;
    border: 1px solid var(--border-color);
}

.chart-container h3 {
    font-size: 1rem;
    margin-bottom: 1rem;
    color: var(--text-secondary);
}

/* Tables */
.table-section {
    margin-bottom: 2rem;
}

.data-table {
    width: 100%;
    border-collapse: collapse;
    background-color: var(--bg-card);
    border-radius: 0.75rem;
    overflow: hidden;
    border: 1px solid var(--border-color);
}

.data-table th,
.data-table td {
    padding: 1rem;
    text-align: left;
    border-bottom: 1px solid var(--border-color);
}

.data-table th {
    background-color: var(--bg-secondary);
    font-weight: 600;
    color: var(--text-secondary);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}

.data-table tbody tr:hover {
    background-color: rgba(255, 255, 255, 0.05);
}

.data-table .loading {
    text-align: center;
    color: var(--text-secondary);
    padding: 2rem;
}

/* Status Indicators */
.status-running, .status-healthy {
    color: var(--accent-secondary);
}

.status-idle, .status-loaded {
    color: var(--accent-warning);
}

.status-error, .status-failed {
    color: var(--accent-primary);
}

/* Health Badge */
.health-badge {
    display: inline-block;
    padding: 0.25rem 0.5rem;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 600;
}

.health-badge.healthy {
    background-color: rgba(15, 155, 15, 0.2);
    color: var(--accent-secondary);
}

.health-badge.degraded {
    background-color: rgba(243, 156, 18, 0.2);
    color: var(--accent-warning);
}

.health-badge.unhealthy {
    background-color: rgba(233, 69, 96, 0.2);
    color: var(--accent-primary);
}

/* Footer */
.footer {
    background-color: var(--bg-secondary);
    padding: 1rem 2rem;
    text-align: center;
    border-top: 1px solid var(--border-color);
    color: var(--text-secondary);
    font-size: 0.875rem;
}

.footer a {
    color: var(--accent-primary);
    text-decoration: none;
}

.footer a:hover {
    text-decoration: underline;
}

/* Responsive */
@media (max-width: 768px) {
    .header {
        flex-direction: column;
        gap: 1rem;
    }

    .main-content {
        padding: 1rem;
    }

    .charts-section {
        grid-template-columns: 1fr;
    }

    .data-table {
        font-size: 0.875rem;
    }

    .data-table th,
    .data-table td {
        padding: 0.75rem 0.5rem;
    }
}
"#;

/// JavaScript Application
pub const APP_JS: &str = r#"
// MoFA Dashboard Application

class Dashboard {
    constructor() {
        this.ws = null;
        this.charts = {};
        this.metricsHistory = [];
        this.maxHistoryLength = 60;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;

        this.init();
    }

    init() {
        this.initCharts();
        this.connectWebSocket();
        this.fetchInitialData();

        // Reconnect on visibility change
        document.addEventListener('visibilitychange', () => {
            if (document.visibilityState === 'visible' && !this.ws) {
                this.connectWebSocket();
            }
        });
    }

    initCharts() {
        // Task Activity Chart
        const tasksCtx = document.getElementById('tasks-chart').getContext('2d');
        this.charts.tasks = new Chart(tasksCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Completed',
                    data: [],
                    borderColor: '#0f9b0f',
                    backgroundColor: 'rgba(15, 155, 15, 0.1)',
                    fill: true,
                    tension: 0.4
                }, {
                    label: 'Failed',
                    data: [],
                    borderColor: '#e94560',
                    backgroundColor: 'rgba(233, 69, 96, 0.1)',
                    fill: true,
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: { beginAtZero: true, grid: { color: 'rgba(255,255,255,0.1)' } },
                    x: { grid: { color: 'rgba(255,255,255,0.1)' } }
                },
                plugins: { legend: { labels: { color: '#eaeaea' } } }
            }
        });

        // Messages Chart
        const messagesCtx = document.getElementById('messages-chart').getContext('2d');
        this.charts.messages = new Chart(messagesCtx, {
            type: 'bar',
            data: {
                labels: [],
                datasets: [{
                    label: 'Sent',
                    data: [],
                    backgroundColor: 'rgba(15, 155, 15, 0.7)',
                }, {
                    label: 'Received',
                    data: [],
                    backgroundColor: 'rgba(15, 99, 132, 0.7)',
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: { beginAtZero: true, grid: { color: 'rgba(255,255,255,0.1)' } },
                    x: { grid: { color: 'rgba(255,255,255,0.1)' } }
                },
                plugins: { legend: { labels: { color: '#eaeaea' } } }
            }
        });
    }

    connectWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws`;

        console.log('Connecting to WebSocket:', wsUrl);

        this.ws = new WebSocket(wsUrl);

        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.updateConnectionStatus(true);
            this.reconnectAttempts = 0;

            // Subscribe to all updates
            this.ws.send(JSON.stringify({
                type: 'subscribe',
                data: { topics: ['metrics', 'alerts', '*'] }
            }));
        };

        this.ws.onmessage = (event) => {
            try {
                const message = JSON.parse(event.data);
                this.handleMessage(message);
            } catch (e) {
                console.error('Failed to parse message:', e);
            }
        };

        this.ws.onclose = () => {
            console.log('WebSocket disconnected');
            this.updateConnectionStatus(false);
            this.ws = null;
            this.scheduleReconnect();
        };

        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    }

    scheduleReconnect() {
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
            const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
            this.reconnectAttempts++;
            console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
            setTimeout(() => this.connectWebSocket(), delay);
        }
    }

    handleMessage(message) {
        switch (message.type) {
            case 'metrics':
                this.updateMetrics(message.data);
                break;
            case 'agent_update':
                this.handleAgentUpdate(message.data);
                break;
            case 'workflow_update':
                this.handleWorkflowUpdate(message.data);
                break;
            case 'alert':
                this.showAlert(message.data);
                break;
            default:
                console.log('Unknown message type:', message.type);
        }
    }

    updateMetrics(data) {
        // Store in history
        this.metricsHistory.push(data);
        if (this.metricsHistory.length > this.maxHistoryLength) {
            this.metricsHistory.shift();
        }

        // Update overview cards
        this.updateOverview(data);

        // Update tables
        this.updateAgentsTable(data.agents || []);
        this.updateWorkflowsTable(data.workflows || []);
        this.updatePluginsTable(data.plugins || []);

        // Update charts
        this.updateCharts(data);

        // Update last update time
        document.getElementById('last-update').textContent =
            'Last update: ' + new Date().toLocaleTimeString();
    }

    updateOverview(data) {
        // Uptime
        const uptime = data.system?.uptime_secs || 0;
        document.getElementById('uptime').textContent = this.formatUptime(uptime);

        // Agents
        const agents = data.agents || [];
        const runningAgents = agents.filter(a => a.state === 'running').length;
        document.getElementById('agent-count').textContent = agents.length;
        document.getElementById('agent-status').textContent = `${runningAgents} running`;

        // Workflows
        const workflows = data.workflows || [];
        const runningWorkflows = workflows.filter(w => w.status === 'running').length;
        document.getElementById('workflow-count').textContent = workflows.length;
        document.getElementById('workflow-status').textContent = `${runningWorkflows} active`;

        // Plugins
        const plugins = data.plugins || [];
        const loadedPlugins = plugins.filter(p => p.state === 'running' || p.state === 'loaded').length;
        document.getElementById('plugin-count').textContent = plugins.length;
        document.getElementById('plugin-status').textContent = `${loadedPlugins} loaded`;
    }

    updateAgentsTable(agents) {
        const tbody = document.getElementById('agents-tbody');
        if (agents.length === 0) {
            tbody.innerHTML = '<tr><td colspan="7" class="loading">No agents found</td></tr>';
            return;
        }

        tbody.innerHTML = agents.map(agent => `
            <tr>
                <td><code>${agent.agent_id}</code></td>
                <td>${agent.name}</td>
                <td class="status-${agent.state}">${agent.state}</td>
                <td>${agent.tasks_completed}</td>
                <td>${agent.tasks_failed}</td>
                <td>${agent.messages_sent + agent.messages_received}</td>
                <td><span class="health-badge ${this.getHealthClass(agent)}">${this.getHealth(agent)}</span></td>
            </tr>
        `).join('');
    }

    updateWorkflowsTable(workflows) {
        const tbody = document.getElementById('workflows-tbody');
        if (workflows.length === 0) {
            tbody.innerHTML = '<tr><td colspan="7" class="loading">No workflows found</td></tr>';
            return;
        }

        tbody.innerHTML = workflows.map(wf => `
            <tr>
                <td><code>${wf.workflow_id}</code></td>
                <td>${wf.name}</td>
                <td class="status-${wf.status}">${wf.status}</td>
                <td>${wf.total_executions}</td>
                <td>${this.calculateSuccessRate(wf)}%</td>
                <td>${wf.avg_execution_time_ms?.toFixed(1) || 0}ms</td>
                <td>${wf.running_instances}</td>
            </tr>
        `).join('');
    }

    updatePluginsTable(plugins) {
        const tbody = document.getElementById('plugins-tbody');
        if (plugins.length === 0) {
            tbody.innerHTML = '<tr><td colspan="7" class="loading">No plugins found</td></tr>';
            return;
        }

        tbody.innerHTML = plugins.map(plugin => `
            <tr>
                <td><code>${plugin.plugin_id}</code></td>
                <td>${plugin.name}</td>
                <td>${plugin.version}</td>
                <td class="status-${plugin.state}">${plugin.state}</td>
                <td>${plugin.call_count}</td>
                <td>${this.calculateErrorRate(plugin)}%</td>
                <td>${plugin.reload_count}</td>
            </tr>
        `).join('');
    }

    updateCharts(data) {
        const agents = data.agents || [];
        const time = new Date().toLocaleTimeString();

        // Tasks chart
        const totalCompleted = agents.reduce((sum, a) => sum + (a.tasks_completed || 0), 0);
        const totalFailed = agents.reduce((sum, a) => sum + (a.tasks_failed || 0), 0);

        this.addChartData(this.charts.tasks, time, [totalCompleted, totalFailed]);

        // Messages chart - show per agent
        const labels = agents.map(a => a.name || a.agent_id).slice(0, 5);
        const sent = agents.map(a => a.messages_sent || 0).slice(0, 5);
        const received = agents.map(a => a.messages_received || 0).slice(0, 5);

        this.charts.messages.data.labels = labels;
        this.charts.messages.data.datasets[0].data = sent;
        this.charts.messages.data.datasets[1].data = received;
        this.charts.messages.update('none');
    }

    addChartData(chart, label, values) {
        chart.data.labels.push(label);
        values.forEach((value, index) => {
            chart.data.datasets[index].data.push(value);
        });

        // Keep only last 20 points
        if (chart.data.labels.length > 20) {
            chart.data.labels.shift();
            chart.data.datasets.forEach(ds => ds.data.shift());
        }

        chart.update('none');
    }

    async fetchInitialData() {
        try {
            const response = await fetch('/api/metrics');
            const result = await response.json();
            if (result.success && result.data) {
                this.updateMetrics(result.data);
            }
        } catch (e) {
            console.error('Failed to fetch initial data:', e);
        }
    }

    updateConnectionStatus(connected) {
        const badge = document.getElementById('connection-status');
        if (connected) {
            badge.textContent = 'Connected';
            badge.className = 'status-badge connected';
        } else {
            badge.textContent = 'Disconnected';
            badge.className = 'status-badge disconnected';
        }
    }

    formatUptime(seconds) {
        const days = Math.floor(seconds / 86400);
        const hours = Math.floor((seconds % 86400) / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);

        if (days > 0) return `${days}d ${hours}h`;
        if (hours > 0) return `${hours}h ${minutes}m`;
        return `${minutes}m`;
    }

    getHealth(agent) {
        if (agent.tasks_failed === 0) return 'healthy';
        const errorRate = agent.tasks_failed / Math.max(agent.tasks_completed, 1);
        if (errorRate < 0.1) return 'degraded';
        return 'unhealthy';
    }

    getHealthClass(agent) {
        return this.getHealth(agent);
    }

    calculateSuccessRate(workflow) {
        if (!workflow.total_executions) return 0;
        return ((workflow.successful_executions / workflow.total_executions) * 100).toFixed(1);
    }

    calculateErrorRate(plugin) {
        if (!plugin.call_count) return 0;
        return ((plugin.error_count / plugin.call_count) * 100).toFixed(1);
    }

    showAlert(data) {
        console.log('Alert:', data);
        // Could show a toast notification here
    }

    handleAgentUpdate(data) {
        console.log('Agent update:', data);
        this.fetchInitialData(); // Refresh data
    }

    handleWorkflowUpdate(data) {
        console.log('Workflow update:', data);
        this.fetchInitialData(); // Refresh data
    }
}

// Initialize dashboard when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.dashboard = new Dashboard();
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_html_exists() {
        assert!(!INDEX_HTML.is_empty());
        assert!(INDEX_HTML.contains("MoFA"));
    }

    #[test]
    fn test_styles_exist() {
        assert!(!STYLES_CSS.is_empty());
        assert!(STYLES_CSS.contains(".dashboard"));
    }

    #[test]
    fn test_app_js_exists() {
        assert!(!APP_JS.is_empty());
        assert!(APP_JS.contains("Dashboard"));
    }
}
