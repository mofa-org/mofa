//! Embedded dashboard assets
//!
//! Contains the HTML, CSS, and JavaScript for the dashboard UI

use axum::{
    body::Body,
    http::{Response, StatusCode, header},
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

    /// Get debugger.html
    pub fn debugger_html() -> &'static str {
        DEBUGGER_HTML
    }

    /// Get debugger.js
    pub fn debugger_js() -> &'static str {
        DEBUGGER_JS
    }
}

/// Debugger HTML Page - with inline JavaScript
pub const DEBUGGER_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>MoFA Visual Debugger</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <div class="debugger">
        <header class="header">
            <div class="logo">
                <h1>🔍 MoFA Visual Debugger</h1>
            </div>
            <nav class="nav">
                <a href="/" class="nav-link">Dashboard</a>
                <a href="/debugger" class="nav-link active">Debugger</a>
            </nav>
            <div class="header-info">
                <span id="connection-status" class="status-badge disconnected">Disconnected</span>
            </div>
        </header>

        <main class="debugger-content">
            <aside class="sessions-sidebar">
                <h2>Sessions</h2>
                <button id="refresh-sessions" class="btn btn-secondary">Refresh</button>
                <div id="sessions-list" class="sessions-list">
                    <div class="loading">Loading sessions...</div>
                </div>
            </aside>

            <div class="debugger-main">
                <section class="graph-section">
                    <h2>Workflow Graph</h2>
                    <div id="workflow-graph" class="workflow-graph">
                        <div class="placeholder">Select a session to view workflow graph</div>
                    </div>
                </section>

                <section class="timeline-section">
                    <div class="timeline-controls">
                        <button id="btn-first" class="btn btn-small" disabled>⏮ First</button>
                        <button id="btn-prev" class="btn btn-small" disabled>◀ Prev</button>
                        <span id="event-index" class="event-index">- / -</span>
                        <button id="btn-next" class="btn btn-small" disabled>Next ▶</button>
                        <button id="btn-last" class="btn btn-small" disabled>⏭ Last</button>
                        <button id="btn-realtime" class="btn btn-small btn-primary">▶ Real-time</button>
                    </div>
                    <div id="event-timeline" class="event-timeline">
                        <div class="placeholder">No events</div>
                    </div>
                </section>

                <section class="state-section">
                    <h2>State Inspector</h2>
                    <div id="state-inspector" class="state-inspector">
                        <div class="placeholder">Select an event to inspect state</div>
                    </div>
                </section>
            </div>
        </main>
    </div>
    <script>
// MoFA Visual Debugger Application
(function() {
    var ws = null;
    var sessions = [];
    var currentSession = null;
    var events = [];
    var currentEventIndex = -1;
    var isRealtime = false;
    var realtimeInterval = null;

    function init() {
        connectWebSocket();
        fetchSessions();
        bindEvents();
    }

    function bindEvents() {
        document.getElementById('refresh-sessions').onclick = function() {
            fetchSessions();
        };
        document.getElementById('btn-first').onclick = function() { goToEvent(0); };
        document.getElementById('btn-prev').onclick = function() { 
            if (currentEventIndex > 0) goToEvent(currentEventIndex - 1); 
        };
        document.getElementById('btn-next').onclick = function() { 
            if (currentEventIndex < events.length - 1) goToEvent(currentEventIndex + 1); 
        };
        document.getElementById('btn-last').onclick = function() { goToEvent(events.length - 1); };
        document.getElementById('btn-realtime').onclick = function() { toggleRealtime(); };
    }

    function connectWebSocket() {
        var protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        var wsUrl = protocol + '//' + window.location.host + '/ws';
        
        ws = new WebSocket(wsUrl);
        
        ws.onopen = function() {
            updateConnectionStatus(true);
            ws.send(JSON.stringify({
                type: 'subscribe',
                data: { topics: ['debug'] }
            }));
        };
        
        ws.onclose = function() {
            updateConnectionStatus(false);
            ws = null;
            setTimeout(function() { connectWebSocket(); }, 3000);
        };
        
        ws.onmessage = function(event) {
            try {
                var msg = JSON.parse(event.data);
                handleMessage(msg);
            } catch (e) {
                console.error('Failed to parse message:', e);
            }
        };
    }

    function handleMessage(msg) {
        if (msg.type === 'debug' && isRealtime && currentSession) {
            events.push(msg.data);
            updateTimeline();
            goToEvent(events.length - 1);
        }
    }

    function updateConnectionStatus(connected) {
        var statusEl = document.getElementById('connection-status');
        if (connected) {
            statusEl.textContent = 'Connected';
            statusEl.className = 'status-badge connected';
        } else {
            statusEl.textContent = 'Disconnected';
            statusEl.className = 'status-badge disconnected';
        }
    }

    async function fetchSessions() {
        try {
            var response = await fetch('/api/debug/sessions');
            var result = await response.json();
            
            if (result.success) {
                sessions = result.data || [];
                renderSessionsList();
            }
        } catch (e) {
            console.error('Error fetching sessions:', e);
        }
    }

    function renderSessionsList() {
        var container = document.getElementById('sessions-list');
        
        if (sessions.length === 0) {
            container.innerHTML = '<div class="empty">No sessions found</div>';
            return;
        }

        var html = '';
        for (var i = 0; i < sessions.length; i++) {
            var session = sessions[i];
            var isActive = currentSession && currentSession.session_id === session.session_id;
            var activeClass = isActive ? ' active' : '';
            var statusClass = ' status-' + session.status;
            var sessionTime = new Date(session.started_at).toLocaleTimeString();
            
            html += '<div class="session-card' + activeClass + '" ' + 
                 'data-session-id="' + session.session_id + '">' +
                '<div class="session-id">' + session.session_id.substring(0, 8) + '...</div>' +
                '<div class="session-status' + statusClass + '">' + session.status + '</div>' +
                '<div class="session-events">' + session.event_count + ' events</div>' +
                '<div class="session-time">' + sessionTime + '</div>' +
            '</div>';
        }
        container.innerHTML = html;

        var cards = container.querySelectorAll('.session-card');
        for (var i = 0; i < cards.length; i++) {
            cards[i].onclick = (function(card) {
                return function() { selectSession(card.dataset.sessionId); };
            })(cards[i]);
        }
    }

    async function selectSession(sessionId) {
        try {
            var response = await fetch('/api/debug/sessions/' + sessionId + '/events');
            var result = await response.json();
            
            if (result.success) {
                currentSession = null;
                for (var i = 0; i < sessions.length; i++) {
                    if (sessions[i].session_id === sessionId) {
                        currentSession = sessions[i];
                        break;
                    }
                }
                events = result.data || [];
                currentEventIndex = -1;
                
                renderSessionsList();
                renderWorkflowGraph();
                updateTimeline();
                updateControls();
                
                if (events.length > 0) {
                    goToEvent(0);
                }
            }
        } catch (e) {
            console.error('Error fetching session events:', e);
        }
    }

    function renderWorkflowGraph() {
        var container = document.getElementById('workflow-graph');
        
        if (!currentSession || events.length === 0) {
            container.innerHTML = '<div class="placeholder">Select a session to view workflow graph</div>';
            return;
        }

        var nodes = new Map();
        var nodeStates = new Map();
        
        for (var i = 0; i < events.length; i++) {
            var event = events[i];
            var eventType = event.type;
            var nodeId = event.data && event.data.node_id;
            
            if (nodeId) {
                if (!nodes.has(nodeId)) {
                    nodes.set(nodeId, { id: nodeId, events: [] });
                }
                nodes.get(nodeId).events.push({ event: event, index: i });
                
                if (eventType === 'node_start') {
                    nodeStates.set(nodeId, 'running');
                } else if (eventType === 'node_end') {
                    nodeStates.set(nodeId, 'completed');
                } else if (eventType === 'error' && event.data && event.data.node_id === nodeId) {
                    nodeStates.set(nodeId, 'errored');
                }
            }
        }

        var width = container.clientWidth || 600;
        var height = 300;
        var nodeSpacing = 120;
        var nodeRadius = 30;
        
        var nodeArray = [];
        nodes.forEach(function(v) { nodeArray.push(v); });
        var totalWidth = nodeArray.length * nodeSpacing;
        var startX = (width - totalWidth) / 2 + nodeSpacing / 2;
        
        var svgContent = '<svg width="' + width + '" height="' + height + '">';
        
        for (var i = 0; i < nodeArray.length; i++) {
            var node = nodeArray[i];
            var x = startX + i * nodeSpacing;
            var y = height / 2;
            var state = nodeStates.get(node.id) || 'pending';
            var activeEvent = events[currentEventIndex];
            var isActive = activeEvent && activeEvent.data && activeEvent.data.node_id === node.id;
            
            var fillColor = '#666';
            if (state === 'running') fillColor = '#2196f3';
            else if (state === 'completed') fillColor = '#4caf50';
            else if (state === 'errored') fillColor = '#f44336';
            
            if (isActive) fillColor = '#ff9800';
            
            var strokeWidth = isActive ? 3 : 0;
            var strokeColor = isActive ? '#fff' : 'none';
            var activeClass = isActive ? ' active' : '';
            
            svgContent += '<circle cx="' + x + '" cy="' + y + '" r="' + nodeRadius + '" ' +
                'fill="' + fillColor + '" stroke="' + strokeColor + '" ' +
                'stroke-width="' + strokeWidth + '" ' +
                'class="node' + activeClass + '"/>' +
                '<text x="' + x + '" y="' + (y + 5) + '" text-anchor="middle" fill="white" font-size="12">' +
                    node.id.substring(0, 8) +
                '</text>';
            
            if (i > 0) {
                var prevX = startX + (i - 1) * nodeSpacing;
                svgContent += '<line x1="' + (prevX + nodeRadius) + '" y1="' + y + '" ' +
                      'x2="' + (x - nodeRadius) + '" y2="' + y + '" ' +
                      'stroke="#666" stroke-width="2"/>';
            }
        }
        
        svgContent += '</svg>';
        container.innerHTML = svgContent;
    }

    function updateTimeline() {
        var container = document.getElementById('event-timeline');
        
        if (events.length === 0) {
            container.innerHTML = '<div class="placeholder">No events</div>';
            return;
        }

        var html = '';
        for (var i = 0; i < events.length; i++) {
            var event = events[i];
            var isActive = i === currentEventIndex;
            var eventType = event.type || 'unknown';
            var nodeId = event.data && event.data.node_id || '';
            var time = event.data && event.data.timestamp_ms ? new Date(event.data.timestamp_ms).toLocaleTimeString() : '';
            var activeClass = isActive ? ' active' : '';
            
            html += '<div class="timeline-event ' + activeClass + ' ' + eventType + '" data-index="' + i + '">' +
                '<span class="event-time">' + time + '</span>' +
                '<span class="event-type">' + eventType + '</span>' +
                '<span class="event-node">' + nodeId + '</span>' +
            '</div>';
        }
        container.innerHTML = html;

        var timelineEvents = container.querySelectorAll('.timeline-event');
        for (var i = 0; i < timelineEvents.length; i++) {
            timelineEvents[i].onclick = (function(idx) {
                return function() { goToEvent(idx); };
            })(i);
        }
    }

    function goToEvent(index) {
        if (index < 0 || index >= events.length) return;
        
        currentEventIndex = index;
        var event = events[index];
        
        updateControls();
        updateTimeline();
        renderWorkflowGraph();
        renderStateInspector(event);
    }

    function renderStateInspector(event) {
        var container = document.getElementById('state-inspector');
        
        if (!event) {
            container.innerHTML = '<div class="placeholder">Select an event to inspect state</div>';
            return;
        }

        var type = event.type;
        var data = event.data || {};
        var stateContent = '';

        if (data.state_snapshot) {
            stateContent = '<pre>' + JSON.stringify(data.state_snapshot, null, 2) + '</pre>';
        } else if (data.old_value !== undefined || data.new_value !== undefined) {
            stateContent = '<div class="state-change">' +
                '<strong>Key:</strong> ' + (data.key || 'N/A') + '<br>' +
                '<strong>Old Value:</strong> <pre>' + JSON.stringify(data.old_value, null, 2) + '</pre>' +
                '<strong>New Value:</strong> <pre>' + JSON.stringify(data.new_value, null, 2) + '</pre>' +
            '</div>';
        } else if (data.error) {
            stateContent = '<div class="error-state"><strong>Error:</strong> ' + data.error + '</div>';
        } else {
            stateContent = '<pre>' + JSON.stringify(data, null, 2) + '</pre>';
        }

        var detailsHtml = '<div class="event-details"><h3>' + type + '</h3>' +
            '<p><strong>Timestamp:</strong> ' + (data.timestamp_ms ? new Date(data.timestamp_ms).toLocaleString() : 'N/A') + '</p>';
        
        if (data.node_id) {
            detailsHtml += '<p><strong>Node:</strong> ' + data.node_id + '</p>';
        }
        if (data.duration_ms) {
            detailsHtml += '<p><strong>Duration:</strong> ' + data.duration_ms + 'ms</p>';
        }
        if (data.status) {
            detailsHtml += '<p><strong>Status:</strong> ' + data.status + '</p>';
        }
        detailsHtml += '</div>';

        container.innerHTML = detailsHtml +
            '<div class="state-content"><h4>State</h4>' + stateContent + '</div>';
    }

    function updateControls() {
        var hasEvents = events.length > 0;
        var idx = currentEventIndex;
        
        document.getElementById('btn-first').disabled = !hasEvents || idx <= 0;
        document.getElementById('btn-prev').disabled = !hasEvents || idx <= 0;
        document.getElementById('btn-next').disabled = !hasEvents || idx >= events.length - 1;
        document.getElementById('btn-last').disabled = !hasEvents || idx >= events.length - 1;
        
        var indexText = hasEvents ? (idx + 1) + ' / ' + events.length : '- / -';
        document.getElementById('event-index').textContent = indexText;
    }

    function toggleRealtime() {
        isRealtime = !isRealtime;
        var btn = document.getElementById('btn-realtime');
        
        if (isRealtime) {
            btn.textContent = '⏹ Stop';
            btn.classList.add('btn-danger');
            realtimeInterval = setInterval(function() {
                fetchSessions();
            }, 5000);
        } else {
            btn.textContent = '▶ Real-time';
            btn.classList.remove('btn-danger');
            if (realtimeInterval) {
                clearInterval(realtimeInterval);
                realtimeInterval = null;
            }
        }
    }

    // Initialize when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
    </script>
</body>
</html>"##;

/// Debugger JavaScript - Placeholder
pub const DEBUGGER_JS: &str = "console.log('Debugger loaded');";

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
            <nav class="nav">
                <a href="/" class="nav-link active">Dashboard</a>
                <a href="/debugger" class="nav-link">🔍 Debugger</a>
            </nav>
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

/* ============================
   Debugger Styles
   ============================ */

.debugger {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-primary);
}

.debugger-content {
    display: flex;
    flex: 1;
    overflow: hidden;
}

/* Sessions Sidebar */
.sessions-sidebar {
    width: 280px;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border-color);
    padding: 1rem;
    overflow-y: auto;
}

.sessions-sidebar h2 {
    font-size: 1.1rem;
    margin-bottom: 1rem;
    color: var(--text-primary);
}

.sessions-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}

.session-card {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.75rem;
    cursor: pointer;
    transition: all 0.2s;
}

.session-card:hover {
    border-color: var(--primary-color);
}

.session-card.active {
    border-color: var(--primary-color);
    background: rgba(33, 150, 243, 0.1);
}

.session-id {
    font-family: monospace;
    font-size: 0.85rem;
    color: var(--text-primary);
    margin-bottom: 0.25rem;
}

.session-status {
    display: inline-block;
    font-size: 0.75rem;
    padding: 0.125rem 0.5rem;
    border-radius: 3px;
    margin-bottom: 0.25rem;
}

.status-running { background: #2196f3; color: white; }
.status-completed { background: #4caf50; color: white; }
.status-failed { background: #f44336; color: white; }

.session-events, .session-time {
    font-size: 0.75rem;
    color: var(--text-secondary);
}

/* Main Debugger Area */
.debugger-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 1rem;
    gap: 1rem;
    overflow: hidden;
}

/* Workflow Graph */
.graph-section {
    flex: 1;
    min-height: 200px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
}

.graph-section h2 {
    font-size: 1rem;
    margin-bottom: 0.5rem;
    color: var(--text-primary);
}

.workflow-graph {
    height: calc(100% - 2rem);
    display: flex;
    align-items: center;
    justify-content: center;
}

.workflow-graph svg {
    width: 100%;
    height: 100%;
}

.workflow-graph .node {
    cursor: pointer;
    transition: all 0.2s;
}

.workflow-graph .node:hover {
    transform: scale(1.1);
}

/* Timeline */
.timeline-section {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
}

.timeline-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
}

.event-index {
    font-family: monospace;
    padding: 0.25rem 0.75rem;
    background: var(--bg-tertiary);
    border-radius: 4px;
}

.event-timeline {
    display: flex;
    gap: 0.25rem;
    overflow-x: auto;
    padding: 0.5rem 0;
    min-height: 50px;
}

.timeline-event {
    flex-shrink: 0;
    padding: 0.25rem 0.5rem;
    background: var(--bg-tertiary);
    border-radius: 4px;
    font-size: 0.7rem;
    cursor: pointer;
    white-space: nowrap;
    border-left: 3px solid #666;
}

.timeline-event.workflow_start { border-left-color: #9c27b0; }
.timeline-event.node_start { border-left-color: #2196f3; }
.timeline-event.state_change { border-left-color: #ff9800; }
.timeline-event.node_end { border-left-color: #4caf50; }
.timeline-event.workflow_end { border-left-color: #9c27b0; }
.timeline-event.error { border-left-color: #f44336; }

.timeline-event:hover {
    background: var(--bg-primary);
}

.timeline-event.active {
    background: rgba(33, 150, 243, 0.2);
    border-color: var(--primary-color);
}

.event-time {
    color: var(--text-secondary);
    margin-right: 0.25rem;
}

.event-type {
    font-weight: 600;
    color: var(--text-primary);
}

.event-node {
    color: var(--text-secondary);
    margin-left: 0.25rem;
}

/* State Inspector */
.state-section {
    height: 200px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow: auto;
}

.state-section h2 {
    font-size: 1rem;
    margin-bottom: 0.5rem;
    color: var(--text-primary);
}

.state-inspector {
    font-size: 0.85rem;
}

.state-inspector pre {
    background: var(--bg-tertiary);
    padding: 0.75rem;
    border-radius: 4px;
    overflow-x: auto;
    font-size: 0.75rem;
}

.event-details {
    margin-bottom: 1rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--border-color);
}

.event-details h3 {
    font-size: 1rem;
    color: var(--primary-color);
    margin-bottom: 0.5rem;
}

.state-content h4 {
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
}

.error-state {
    color: #f44336;
    padding: 0.5rem;
    background: rgba(244, 67, 54, 0.1);
    border-radius: 4px;
}

/* Placeholder */
.placeholder {
    color: var(--text-secondary);
    font-style: italic;
    text-align: center;
    padding: 2rem;
}

.empty {
    color: var(--text-secondary);
    text-align: center;
    padding: 1rem;
}

/* Nav */
.nav {
    display: flex;
    gap: 0.5rem;
}

.nav-link {
    padding: 0.5rem 1rem;
    color: var(--text-secondary);
    text-decoration: none;
    border-radius: 4px;
    transition: all 0.2s;
}

.nav-link:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
}

.nav-link.active {
    background: var(--primary-color);
    color: white;
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
