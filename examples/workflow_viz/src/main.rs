//! Workflow Visualization Web UI — Phase 2: Live Execution Monitoring
//!
//! An axum-based web server that loads YAML workflow definitions,
//! serves an interactive graph visualization, and streams live execution
//! events to connected WebSocket clients.
//!
//! Run with:  cargo run -p workflow_viz
//! Then open: http://127.0.0.1:3030

#![allow(missing_docs, missing_debug_implementations)]

use axum::{
    Json,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use futures::{SinkExt, StreamExt};
use mofa_foundation::workflow::dsl::{WorkflowDefinition, WorkflowDslParser};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::info;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Live execution status for a node
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// A live execution event broadcast to WebSocket clients
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NodeEvent {
    event_type: String,          // "node_status" | "workflow_start" | "workflow_end"
    node_id: Option<String>,
    status: Option<NodeStatus>,
    timestamp_ms: u64,
    duration_ms: Option<u64>,
    error: Option<String>,
    inputs: Option<Value>,
    outputs: Option<Value>,
    workflow_id: Option<String>,
    execution_id: Option<String>,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

/// Shared state holding parsed workflow definitions and live execution state.
struct AppState {
    /// workflow-id → parsed definition
    definitions: HashMap<String, WorkflowDefinition>,
    /// Broadcast channel for live events
    event_tx: broadcast::Sender<NodeEvent>,
    /// Current node execution states (node_id → status)
    node_states: Arc<RwLock<HashMap<String, NodeStatusEntry>>>,
    /// Whether a simulation is currently running
    sim_running: Arc<RwLock<bool>>,
}

/// Stored state for a node during/after execution
#[derive(Debug, Clone, serde::Serialize)]
struct NodeStatusEntry {
    status: NodeStatus,
    duration_ms: Option<u64>,
    error: Option<String>,
    inputs: Option<Value>,
    outputs: Option<Value>,
}

type SharedState = Arc<RwLock<AppState>>;

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter("info,workflow_viz=debug")
        .init();

    info!("╔═══════════════════════════════════════════════════╗");
    info!("║   MoFA Workflow Visualization — Live Monitor      ║");
    info!("╚═══════════════════════════════════════════════════╝");

    // Load YAML workflow definitions
    let yaml_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../workflow_dsl");
    let mut definitions: HashMap<String, WorkflowDefinition> = HashMap::new();

    if yaml_dir.is_dir() {
        for entry in std::fs::read_dir(&yaml_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
                match WorkflowDslParser::from_file(&path) {
                    Ok(def) => {
                        info!("  ✓ Loaded: {} ({})", def.metadata.name, def.metadata.id);
                        definitions.insert(def.metadata.id.clone(), def);
                    }
                    Err(e) => {
                        info!("  ✗ Failed to parse {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    info!("Loaded {} workflow definitions", definitions.len());

    let (event_tx, _) = broadcast::channel::<NodeEvent>(256);

    let state: SharedState = Arc::new(RwLock::new(AppState {
        definitions,
        event_tx,
        node_states: Arc::new(RwLock::new(HashMap::new())),
        sim_running: Arc::new(RwLock::new(false)),
    }));

    // Build router
    let app = axum::Router::new()
        // Static frontend
        .route("/", get(serve_index))
        .route("/style.css", get(serve_css))
        .route("/app.js", get(serve_js))
        .route("/logo.png", get(serve_logo))
        // REST API — workflows
        .route("/api/workflows", get(list_workflows))
        .route("/api/workflows/:id", get(get_workflow))
        // Live monitoring
        .route("/ws", get(ws_handler))
        .route("/api/simulate", post(simulate_execution))
        .route("/api/execution/state", get(get_execution_state))
        .with_state(state)
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any),
        );

    let addr: std::net::SocketAddr = "127.0.0.1:3030".parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("🚀  http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Static file handlers
// ---------------------------------------------------------------------------

fn static_dir() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(manifest).join("static")
}

async fn serve_index() -> Response {
    let path = static_dir().join("index.html");
    match tokio::fs::read_to_string(&path).await {
        Ok(body) => Html(body).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {e}")).into_response(),
    }
}

async fn serve_css() -> Response {
    let path = static_dir().join("style.css");
    match tokio::fs::read_to_string(&path).await {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static("text/css"))],
            body,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, HeaderValue::from_static("text/css"))],
            format!("/* Error: {e} */"),
        )
            .into_response(),
    }
}

async fn serve_js() -> Response {
    let path = static_dir().join("app.js");
    match tokio::fs::read_to_string(&path).await {
        Ok(body) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/javascript"),
            )],
            body,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/javascript"),
            )],
            format!("// Error: {e}"),
        )
            .into_response(),
    }
}

async fn serve_logo() -> Response {
    let path = static_dir().join("logo.png");
    match tokio::fs::read(&path).await {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))],
            body,
        )
            .into_response(),
        Err(e) => (StatusCode::NOT_FOUND, format!("Logo not found: {e}")).into_response(),
    }
}

// ---------------------------------------------------------------------------
// REST API — Workflow handlers
// ---------------------------------------------------------------------------

/// GET /api/workflows
async fn list_workflows(State(state): State<SharedState>) -> Json<Value> {
    let s = state.read().await;
    let mut defs: Vec<&WorkflowDefinition> = s.definitions.values().collect();
    defs.sort_by(|a, b| a.metadata.id.cmp(&b.metadata.id));
    let items: Vec<Value> = defs
        .iter()
        .map(|def| {
            json!({
                "id": def.metadata.id,
                "name": def.metadata.name,
                "description": def.metadata.description,
                "node_count": def.nodes.len(),
                "edge_count": def.edges.len(),
            })
        })
        .collect();
    Json(json!({ "workflows": items }))
}

/// GET /api/workflows/{id}
async fn get_workflow(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let s = state.read().await;
    let def = s.definitions.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(definition_to_json(def)))
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

/// GET /ws — upgrade to WebSocket for live event streaming
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: SharedState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast
    let mut rx = {
        let s = state.read().await;
        s.event_tx.subscribe()
    };

    info!("WebSocket client connected");

    // Send current state snapshot as initial catch-up
    {
        let s = state.read().await;
        let states = s.node_states.read().await;
        if !states.is_empty() {
            let snapshot = json!({
                "event_type": "state_snapshot",
                "states": &*states,
            });
            let _ = sender.send(Message::Text(snapshot.to_string().into())).await;
        }
    }

    // Forward broadcast events to client
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            let json = serde_json::to_string(&event).unwrap_or_default();
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!("WS client lagged {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                    // Heartbeat ping
                    let hb = json!({"event_type": "heartbeat", "timestamp_ms": now_ms()});
                    if sender.send(Message::Text(hb.to_string().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Receive side — just consume pings/pongs and detect close
    let recv_task = tokio::spawn(async move {
        while let Some(result) = receiver.next().await {
            match result {
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {} // ignore all client messages
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    info!("WebSocket client disconnected");
}

// ---------------------------------------------------------------------------
// GET /api/execution/state — for reconnection catch-up
// ---------------------------------------------------------------------------

async fn get_execution_state(State(state): State<SharedState>) -> Json<Value> {
    let s = state.read().await;
    let states = s.node_states.read().await;
    let sim_running = *s.sim_running.read().await;
    Json(json!({
        "states": &*states,
        "sim_running": sim_running,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/simulate — synthetic execution
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct SimParams {
    workflow_id: String,
}

async fn simulate_execution(
    State(state): State<SharedState>,
    Query(params): Query<SimParams>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Get the workflow definition and atomically check-and-set sim_running
    let (def_json, event_tx, node_states_arc, sim_running_arc) = {
        let s = state.read().await;

        // Atomic check-and-set under the same write lock to prevent races
        {
            let mut running = s.sim_running.write().await;
            if *running {
                return Err((StatusCode::CONFLICT, Json(json!({"error": "Simulation already running"}))));
            }
            *running = true;
        }

        let def = s.definitions.get(&params.workflow_id)
            .ok_or((StatusCode::NOT_FOUND, Json(json!({"error": "Workflow not found"}))))?;
        (
            definition_to_json(def),
            s.event_tx.clone(),
            s.node_states.clone(),
            s.sim_running.clone(),
        )
    };

    let execution_id = uuid::Uuid::new_v4().to_string();
    let workflow_id = params.workflow_id.clone();

    // Compute topological order
    let nodes = def_json["nodes"].as_array().cloned().unwrap_or_default();
    let edges = def_json["edges"].as_array().cloned().unwrap_or_default();
    let order = topo_order(&nodes, &edges);

    // Reset node states to pending
    {
        let mut states = node_states_arc.write().await;
        states.clear();
        for node_id in &order {
            states.insert(node_id.clone(), NodeStatusEntry {
                status: NodeStatus::Pending,
                duration_ms: None,
                error: None,
                inputs: None,
                outputs: None,
            });
        }
    }

    info!("  ▶ Starting simulation of '{}' ({} nodes)", workflow_id, order.len());

    // Broadcast workflow start
    let _ = event_tx.send(NodeEvent {
        event_type: "workflow_start".to_string(),
        node_id: None,
        status: None,
        timestamp_ms: now_ms(),
        duration_ms: None,
        error: None,
        inputs: None,
        outputs: None,
        workflow_id: Some(workflow_id.clone()),
        execution_id: Some(execution_id.clone()),
    });

    // Spawn background task to walk nodes
    let wf_id = workflow_id.clone();
    let exec_id = execution_id.clone();
    tokio::spawn(async move {
        for node_id in &order {
            let node_meta = nodes.iter()
                .find(|n| n["id"].as_str() == Some(node_id))
                .cloned()
                .unwrap_or(json!({}));
            let node_type = node_meta["type"].as_str().unwrap_or("task");

            // Determine duration based on node type
            let duration_ms: u64 = match node_type {
                "start" | "end" => 150 + pseudo_rand(node_id) % 100,
                "condition" => 200 + pseudo_rand(node_id) % 200,
                "parallel" | "join" => 400 + pseudo_rand(node_id) % 300,
                "loop" => 600 + pseudo_rand(node_id) % 500,
                _ => 300 + pseudo_rand(node_id) % 500,
            };

            // Random failure chance (~8%)
            let will_fail = pseudo_rand(node_id) % 13 == 0 && node_type != "start" && node_type != "end";

            // -- RUNNING --
            {
                let mut states = node_states_arc.write().await;
                if let Some(entry) = states.get_mut(node_id) {
                    entry.status = NodeStatus::Running;
                    entry.inputs = Some(json!({
                        "triggered_by": "simulation",
                        "node_type": node_type,
                    }));
                }
            }
            let _ = event_tx.send(NodeEvent {
                event_type: "node_status".to_string(),
                node_id: Some(node_id.clone()),
                status: Some(NodeStatus::Running),
                timestamp_ms: now_ms(),
                duration_ms: None,
                error: None,
                inputs: Some(json!({"triggered_by": "simulation", "node_type": node_type})),
                outputs: None,
                workflow_id: None,
                execution_id: None,
            });

            // Simulate execution time
            tokio::time::sleep(std::time::Duration::from_millis(duration_ms)).await;

            // -- COMPLETED or FAILED --
            let (final_status, error) = if will_fail {
                (NodeStatus::Failed, Some(format!("Simulated error in node '{}'", node_id)))
            } else {
                (NodeStatus::Completed, None)
            };

            let outputs = if final_status == NodeStatus::Completed {
                Some(json!({
                    "result": format!("Output from {}", node_id),
                    "processed_items": pseudo_rand(node_id) % 100 + 1,
                }))
            } else {
                None
            };

            {
                let mut states = node_states_arc.write().await;
                if let Some(entry) = states.get_mut(node_id) {
                    entry.status = final_status;
                    entry.duration_ms = Some(duration_ms);
                    entry.error = error.clone();
                    entry.outputs = outputs.clone();
                }
            }

            let _ = event_tx.send(NodeEvent {
                event_type: "node_status".to_string(),
                node_id: Some(node_id.clone()),
                status: Some(final_status),
                timestamp_ms: now_ms(),
                duration_ms: Some(duration_ms),
                error: error.clone(),
                inputs: None,
                outputs: outputs.clone(),
                workflow_id: None,
                execution_id: None,
            });

            // If a node fails, stop the simulation
            if will_fail {
                info!("  ✗ Node '{}' failed — stopping simulation", node_id);
                break;
            }
        }

        // Broadcast workflow end
        let _ = event_tx.send(NodeEvent {
            event_type: "workflow_end".to_string(),
            node_id: None,
            status: None,
            timestamp_ms: now_ms(),
            duration_ms: None,
            error: None,
            inputs: None,
            outputs: None,
            workflow_id: Some(wf_id.clone()),
            execution_id: Some(exec_id),
        });

        *sim_running_arc.write().await = false;
        info!("  ✓ Simulation of '{}' complete", wf_id);
    });

    Ok(Json(json!({
        "execution_id": execution_id,
        "workflow_id": workflow_id,
        "status": "started",
    })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}

/// Deterministic pseudo-random from a string (for consistent simulation behaviour)
fn pseudo_rand(s: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Topological order via Kahn's BFS
fn topo_order(nodes: &[Value], edges: &[Value]) -> Vec<String> {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_deg: HashMap<String, usize> = HashMap::new();
    for n in nodes {
        let id = n["id"].as_str().unwrap_or("").to_string();
        adj.entry(id.clone()).or_default();
        in_deg.entry(id).or_insert(0);
    }
    for e in edges {
        let from = e["from"].as_str().unwrap_or("").to_string();
        let to = e["to"].as_str().unwrap_or("").to_string();
        adj.entry(from.clone()).or_default().push(to.clone());
        *in_deg.entry(to).or_insert(0) += 1;
    }

    let mut queue: Vec<String> = in_deg.iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(id, _)| id.clone())
        .collect();
    queue.sort();
    let mut order = Vec::new();
    let mut qi = 0;
    while qi < queue.len() {
        let cur = queue[qi].clone();
        qi += 1;
        order.push(cur.clone());
        if let Some(nexts) = adj.get(&cur) {
            for nx in nexts {
                if let Some(d) = in_deg.get_mut(nx) {
                    *d -= 1;
                    if *d == 0 {
                        queue.push(nx.clone());
                    }
                }
            }
        }
    }
    // Remaining nodes
    for n in nodes {
        let id = n["id"].as_str().unwrap_or("").to_string();
        if !order.contains(&id) {
            order.push(id);
        }
    }
    order
}

/// Convert a parsed `WorkflowDefinition` into the graph JSON structure
/// expected by the frontend, without needing real agent instances.
fn definition_to_json(def: &WorkflowDefinition) -> Value {
    use mofa_foundation::workflow::dsl::NodeDefinition;
    use mofa_foundation::workflow::NodeType;

    let mut nodes: Vec<Value> = def
        .nodes
        .iter()
        .map(|n| {
            let type_str = match n.node_type() {
                NodeType::Start => "start",
                NodeType::End => "end",
                NodeType::Task => "task",
                NodeType::Agent => "agent",
                NodeType::Condition => "condition",
                NodeType::Parallel => "parallel",
                NodeType::Join => "join",
                NodeType::Loop => "loop",
                NodeType::Wait => "wait",
                NodeType::Transform => "transform",
                NodeType::SubWorkflow => "sub_workflow",
            };
            let name = match n {
                NodeDefinition::Start { id, name, .. } => {
                    name.as_deref().unwrap_or(id).to_string()
                }
                NodeDefinition::End { id, name, .. } => {
                    name.as_deref().unwrap_or(id).to_string()
                }
                NodeDefinition::Task { name, .. }
                | NodeDefinition::LlmAgent { name, .. }
                | NodeDefinition::Condition { name, .. }
                | NodeDefinition::Parallel { name, .. }
                | NodeDefinition::Join { name, .. }
                | NodeDefinition::Loop { name, .. }
                | NodeDefinition::Transform { name, .. }
                | NodeDefinition::Wait { name, .. }
                | NodeDefinition::SubWorkflow { name, .. } => name.clone(),
            };
            json!({
                "id": n.id(),
                "name": name,
                "type": type_str,
            })
        })
        .collect();
    nodes.sort_by(|a, b| a["id"].as_str().unwrap_or("").cmp(b["id"].as_str().unwrap_or("")));

    let mut edges: Vec<Value> = def
        .edges
        .iter()
        .map(|e| {
            let edge_type = if e.condition.is_some() {
                "conditional"
            } else {
                "normal"
            };
            json!({
                "from": e.from,
                "to": e.to,
                "edge_type": edge_type,
                "label": e.label.as_deref().or(e.condition.as_deref()),
            })
        })
        .collect();
    edges.sort_by(|a, b| {
        let af = a["from"].as_str().unwrap_or("");
        let bf = b["from"].as_str().unwrap_or("");
        af.cmp(bf).then_with(|| {
            a["to"].as_str().unwrap_or("").cmp(b["to"].as_str().unwrap_or(""))
        })
    });

    let start_node = def.nodes.iter().find_map(|n| {
        if matches!(n, NodeDefinition::Start { .. }) {
            Some(n.id().to_string())
        } else {
            None
        }
    });
    let end_nodes: Vec<String> = def
        .nodes
        .iter()
        .filter_map(|n| {
            if matches!(n, NodeDefinition::End { .. }) {
                Some(n.id().to_string())
            } else {
                None
            }
        })
        .collect();

    json!({
        "id": def.metadata.id,
        "name": def.metadata.name,
        "description": def.metadata.description,
        "nodes": nodes,
        "edges": edges,
        "start_node": start_node,
        "end_nodes": end_nodes,
    })
}
