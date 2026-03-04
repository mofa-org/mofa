//! Visual Debugger Demo Example
//!
//! This example demonstrates the Visual Debugger feature by creating a debug session
//! with workflow-like events. Users can open the debugger UI (from monitoring_dashboard) 
//! to step through execution, inspect state, and visualize the workflow.
//!
//! Run with: cargo run -p visual_debugger_demo
//!
//! Then run: cargo run -p monitoring_dashboard
//! And open: http://127.0.0.1:8080/debugger
//!
//! You should see the demo-session-1 in the sidebar.

use std::sync::Arc;

use mofa_foundation::workflow::session_recorder::InMemorySessionRecorder;
use mofa_kernel::workflow::telemetry::{DebugEvent, DebugSession, SessionRecorder};
use tracing::info;

/// Creates simulated workflow events for demonstration
/// 
/// This represents what would be recorded during an actual workflow execution.
/// The workflow processes data through multiple stages:
/// 1. input - Receives raw input data
/// 2. validate - Validates the input
/// 3. transform - Transforms/cleans the data
/// 4. process - Processes the data
/// 5. output - Produces final output
fn create_demo_workflow_events(execution_id: &str) -> Vec<DebugEvent> {
    let base_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    
    vec![
        DebugEvent::WorkflowStart {
            workflow_id: "data-pipeline".to_string(),
            execution_id: execution_id.to_string(),
            timestamp_ms: base_time,
        },
        DebugEvent::NodeStart {
            node_id: "input".to_string(),
            timestamp_ms: base_time + 100,
            state_snapshot: serde_json::json!({
                "input_data": {
                    "records": [
                        {"id": 1, "value": "hello"},
                        {"id": 2, "value": "world"},
                        {"id": 3, "value": "test"}
                    ]
                },
                "validated": false
            }),
        },
        DebugEvent::StateChange {
            node_id: "input".to_string(),
            timestamp_ms: base_time + 150,
            key: "validated".to_string(),
            old_value: Some(serde_json::json!(false)),
            new_value: serde_json::json!(true),
        },
        DebugEvent::NodeEnd {
            node_id: "input".to_string(),
            timestamp_ms: base_time + 200,
            state_snapshot: serde_json::json!({
                "input_data": {
                    "records": [
                        {"id": 1, "value": "hello"},
                        {"id": 2, "value": "world"},
                        {"id": 3, "value": "test"}
                    ]
                },
                "validated": true
            }),
            duration_ms: 100,
        },
        DebugEvent::NodeStart {
            node_id: "validate".to_string(),
            timestamp_ms: base_time + 300,
            state_snapshot: serde_json::json!({
                "input_data": {
                    "records": [
                        {"id": 1, "value": "hello"},
                        {"id": 2, "value": "world"},
                        {"id": 3, "value": "test"}
                    ]
                },
                "validated": true,
                "validation_passed": true
            }),
        },
        DebugEvent::NodeEnd {
            node_id: "validate".to_string(),
            timestamp_ms: base_time + 400,
            state_snapshot: serde_json::json!({
                "input_data": {
                    "records": [
                        {"id": 1, "value": "hello"},
                        {"id": 2, "value": "world"},
                        {"id": 3, "value": "test"}
                    ]
                },
                "validated": true,
                "validation_passed": true
            }),
            duration_ms: 100,
        },
        DebugEvent::NodeStart {
            node_id: "transform".to_string(),
            timestamp_ms: base_time + 500,
            state_snapshot: serde_json::json!({
                "input_data": {
                    "records": [
                        {"id": 1, "value": "HELLO"},
                        {"id": 2, "value": "WORLD"},
                        {"id": 3, "value": "TEST"}
                    ]
                },
                "validated": true,
                "transformed": true
            }),
        },
        DebugEvent::NodeEnd {
            node_id: "transform".to_string(),
            timestamp_ms: base_time + 600,
            state_snapshot: serde_json::json!({
                "input_data": {
                    "records": [
                        {"id": 1, "value": "HELLO"},
                        {"id": 2, "value": "WORLD"},
                        {"id": 3, "value": "TEST"}
                    ]
                },
                "validated": true,
                "transformed": true
            }),
            duration_ms: 100,
        },
        DebugEvent::NodeStart {
            node_id: "process".to_string(),
            timestamp_ms: base_time + 700,
            state_snapshot: serde_json::json!({
                "processed_count": 3,
                "status": "processing"
            }),
        },
        DebugEvent::StateChange {
            node_id: "process".to_string(),
            timestamp_ms: base_time + 800,
            key: "status".to_string(),
            old_value: Some(serde_json::json!("processing")),
            new_value: serde_json::json!("completed"),
        },
        DebugEvent::NodeEnd {
            node_id: "process".to_string(),
            timestamp_ms: base_time + 900,
            state_snapshot: serde_json::json!({
                "processed_count": 3,
                "status": "completed"
            }),
            duration_ms: 200,
        },
        DebugEvent::NodeStart {
            node_id: "output".to_string(),
            timestamp_ms: base_time + 1000,
            state_snapshot: serde_json::json!({
                "processed_count": 3,
                "status": "completed",
                "output": {
                    "summary": "Processed 3 records successfully",
                    "records": [
                        {"id": 1, "value": "HELLO"},
                        {"id": 2, "value": "WORLD"},
                        {"id": 3, "value": "TEST"}
                    ]
                }
            }),
        },
        DebugEvent::NodeEnd {
            node_id: "output".to_string(),
            timestamp_ms: base_time + 1100,
            state_snapshot: serde_json::json!({
                "processed_count": 3,
                "status": "completed",
                "output": {
                    "summary": "Processed 3 records successfully",
                    "records": [
                        {"id": 1, "value": "HELLO"},
                        {"id": 2, "value": "WORLD"},
                        {"id": 3, "value": "TEST"}
                    ]
                }
            }),
            duration_ms: 100,
        },
        DebugEvent::WorkflowEnd {
            workflow_id: "data-pipeline".to_string(),
            execution_id: execution_id.to_string(),
            timestamp_ms: base_time + 1200,
            status: "completed".to_string(),
        },
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,mofa=debug")
        .init();

    let session_id = "demo-session-1";
    let execution_id = "exec-1";

    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║         MoFA Visual Debugger Demo                         ║");
    info!("╚══════════════════════════════════════════════════════════════╝");

    // Step 1: Create session recorder for debugging
    let recorder = Arc::new(InMemorySessionRecorder::new());
    
    // Create and start a debug session
    let session = DebugSession::new(session_id, "data-pipeline", execution_id);
    recorder.start_session(&session).await?;
    info!("📋 Created debug session: {}", session_id);

    // Step 2: Record demo workflow events
    info!("📝 Recording demo workflow events...");
    let events = create_demo_workflow_events(execution_id);
    for event in &events {
        recorder.record_event(session_id, event).await?;
    }
    
    // End the session
    recorder.end_session(session_id, "completed").await?;
    info!("✅ Recorded {} events for workflow: data-pipeline", events.len());

    // Step 3: List sessions to verify
    let sessions = recorder.list_sessions().await?;
    info!("📊 Total sessions in recorder: {}", sessions.len());
    for s in &sessions {
        info!("   - {} ({} events)", s.session_id, s.event_count);
    }

    info!("\n╔══════════════════════════════════════════════════════════════╗");
    info!("║  🎯 Visual Debugger Demo Data Ready!                       ║");
    info!("║                                                              ║");
    info!("║  To view in debugger:                                      ║");
    info!("║    1. Run: cargo run -p monitoring_dashboard              ║");
    info!("║    2. Open: http://127.0.0.1:8080/debugger                ║");
    info!("║    3. You'll see 'demo-session-1' in the sidebar          ║");
    info!("║                                                              ║");
    info!("║  Session ID:     {}                                       ║", session_id);
    info!("║  Workflow:       data-pipeline                             ║");
    info!("║  Events:         {}                                        ║", events.len());
    info!("║                                                              ║");
    info!("║  The session recorder is ready to be connected            ║");
    info!("║  to a DashboardServer for visualization.                  ║");
    info!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}
