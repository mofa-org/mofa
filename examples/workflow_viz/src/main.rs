//! Workflow Visualization Web UI
//!
//! An axum-based web server that loads YAML workflow definitions,
//! serves an interactive graph visualization, and streams simulated
//! telemetry events via WebSocket.
//!
//! Run with:  cargo run -p workflow_viz
//! Then open: http://127.0.0.1:3030

#![allow(missing_docs, missing_debug_implementations)]

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use mofa_foundation::workflow::dsl::{WorkflowDefinition, WorkflowDslParser};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

/// Shared state holding parsed workflow definitions.
struct AppState {
    /// workflow‑id → parsed definition
    definitions: HashMap<String, WorkflowDefinition>,
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
    info!("║      MoFA Workflow Visualization Server           ║");
    info!("╚═══════════════════════════════════════════════════╝");

    // Load YAML workflow definitions -----------------------------------------
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

    let state: SharedState = Arc::new(RwLock::new(AppState { definitions }));

    // Build router -----------------------------------------------------------
    let app = axum::Router::new()
        // Static frontend
        .route("/", get(serve_index))
        .route("/style.css", get(serve_css))
        .route("/app.js", get(serve_js))
        // REST API
        .route("/api/workflows", get(list_workflows))
        .route("/api/workflows/:id", get(get_workflow))
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
// Static file handlers (read from disk for hot-reload during development)
// ---------------------------------------------------------------------------

fn static_dir() -> std::path::PathBuf {
    // Resolve relative to the crate root (where Cargo.toml lives)
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(manifest).join("static")
}

async fn serve_index() -> Html<String> {
    let path = static_dir().join("index.html");
    Html(std::fs::read_to_string(&path).unwrap_or_else(|e| format!("Error loading index.html: {e}")))
}

async fn serve_css() -> Response {
    let path = static_dir().join("style.css");
    let body = std::fs::read_to_string(&path).unwrap_or_else(|e| format!("/* Error: {e} */"));
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/css"))],
        body,
    )
        .into_response()
}

async fn serve_js() -> Response {
    let path = static_dir().join("app.js");
    let body = std::fs::read_to_string(&path).unwrap_or_else(|e| format!("// Error: {e}"));
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript"),
        )],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// REST API handlers
// ---------------------------------------------------------------------------

/// GET /api/workflows — list all loaded workflows
async fn list_workflows(State(state): State<SharedState>) -> Json<Value> {
    let s = state.read().await;
    let items: Vec<Value> = s
        .definitions
        .values()
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

/// GET /api/workflows/:id — return graph JSON for a single workflow
async fn get_workflow(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let s = state.read().await;
    let def = s.definitions.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(definition_to_json(def)))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a parsed `WorkflowDefinition` into the graph JSON structure
/// expected by the frontend, without needing real agent instances.
fn definition_to_json(def: &WorkflowDefinition) -> Value {
    use mofa_foundation::workflow::dsl::NodeDefinition;
    use mofa_foundation::workflow::NodeType;

    let nodes: Vec<Value> = def
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

    let edges: Vec<Value> = def
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

    // Derive start_node / end_nodes from the definitions
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

