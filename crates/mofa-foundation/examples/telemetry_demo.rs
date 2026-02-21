//! Time-Travel Debugger Telemetry Demo
//!
//! This example demonstrates the telemetry infrastructure by running a simple
//! workflow with a ChannelTelemetryEmitter attached, then displaying the
//! captured execution trace in a formatted timeline.
//!
//! Run with: cargo run --example telemetry_demo -p mofa-foundation

use mofa_foundation::workflow::{
    ChannelTelemetryEmitter, ExecutorConfig, InMemorySessionRecorder, WorkflowExecutor,
    WorkflowGraph, WorkflowNode, WorkflowValue,
};
use mofa_kernel::workflow::telemetry::{DebugEvent, DebugSession, SessionRecorder};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       🕰️  MoFA Time-Travel Debugger — Telemetry Demo       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // ─── 1. Build a sample workflow ────────────────────────────────────
    let mut graph = WorkflowGraph::new("data_pipeline", "Data Processing Pipeline");

    graph.add_node(WorkflowNode::start("start"));
    graph.add_node(WorkflowNode::task(
        "validate",
        "Validate Input",
        |_ctx, input| async move {
            let value = input.as_i64().unwrap_or(0);
            if value > 0 {
                Ok(WorkflowValue::Int(value))
            } else {
                Err("Input must be positive".to_string())
            }
        },
    ));
    graph.add_node(WorkflowNode::task(
        "transform",
        "Transform Data",
        |_ctx, input| async move {
            let value = input.as_i64().unwrap_or(0);
            Ok(WorkflowValue::Int(value * 3 + 7))
        },
    ));
    graph.add_node(WorkflowNode::task(
        "enrich",
        "Enrich Results",
        |_ctx, input| async move {
            let value = input.as_i64().unwrap_or(0);
            Ok(WorkflowValue::Map({
                let mut m = std::collections::HashMap::new();
                m.insert("result".to_string(), WorkflowValue::Int(value));
                m.insert(
                    "label".to_string(),
                    WorkflowValue::String(format!("processed_{}", value)),
                );
                m
            }))
        },
    ));
    graph.add_node(WorkflowNode::end("end"));

    graph.connect("start", "validate");
    graph.connect("validate", "transform");
    graph.connect("transform", "enrich");
    graph.connect("enrich", "end");

    // ─── 2. Set up telemetry ───────────────────────────────────────────
    let (emitter, mut rx) = ChannelTelemetryEmitter::new(256);
    let recorder = Arc::new(InMemorySessionRecorder::new());

    // Start a debug session
    let session = DebugSession::new("demo-session-001", "data_pipeline", "exec-001");
    recorder.start_session(&session).await.unwrap();

    println!("📋 Session: {}", session.session_id);
    println!("📊 Workflow: {} ({})", "Data Processing Pipeline", "data_pipeline");
    println!("🔢 Input: 42");
    println!();
    println!("─── Execution Timeline ────────────────────────────────────────");
    println!();

    // ─── 3. Execute with telemetry ─────────────────────────────────────
    let executor = WorkflowExecutor::new(ExecutorConfig::default())
        .with_telemetry(Arc::new(emitter));

    let result = executor
        .execute(&graph, WorkflowValue::Int(42))
        .await
        .unwrap();

    // ─── 4. Display captured events ────────────────────────────────────
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    // Also record events to the session recorder (simulating production use)
    for event in &events {
        recorder
            .record_event("demo-session-001", event)
            .await
            .unwrap();
    }
    recorder
        .end_session("demo-session-001", "completed")
        .await
        .unwrap();

    let base_ts = events.first().map(|e| e.timestamp_ms()).unwrap_or(0);

    for (i, event) in events.iter().enumerate() {
        let relative_ms = event.timestamp_ms() - base_ts;
        let prefix = if i == events.len() - 1 {
            "└──"
        } else {
            "├──"
        };

        match event {
            DebugEvent::WorkflowStart {
                workflow_id,
                execution_id,
                ..
            } => {
                println!(
                    "  {} ⚡ [{:>4}ms] WORKFLOW START  │ workflow={}, exec={}",
                    prefix, relative_ms, workflow_id, execution_id
                );
            }
            DebugEvent::NodeStart {
                node_id,
                state_snapshot,
                ..
            } => {
                let state_preview = serde_json::to_string(state_snapshot)
                    .unwrap_or_default();
                let truncated = if state_preview.len() > 60 {
                    format!("{}...", &state_preview[..57])
                } else {
                    state_preview
                };
                println!(
                    "  {} 🟢 [{:>4}ms] NODE START      │ node={:<12} │ state={}",
                    prefix, relative_ms, node_id, truncated
                );
            }
            DebugEvent::NodeEnd {
                node_id,
                duration_ms,
                state_snapshot,
                ..
            } => {
                let state_preview = serde_json::to_string(state_snapshot)
                    .unwrap_or_default();
                let truncated = if state_preview.len() > 50 {
                    format!("{}...", &state_preview[..47])
                } else {
                    state_preview
                };
                println!(
                    "  {} 🔵 [{:>4}ms] NODE END        │ node={:<12} │ took={}ms │ out={}",
                    prefix, relative_ms, node_id, duration_ms, truncated
                );
            }
            DebugEvent::WorkflowEnd {
                status,
                ..
            } => {
                println!(
                    "  {} 🏁 [{:>4}ms] WORKFLOW END    │ status={}",
                    prefix, relative_ms, status
                );
            }
            DebugEvent::Error {
                node_id, error, ..
            } => {
                println!(
                    "  {} ❌ [{:>4}ms] ERROR           │ node={:?} │ {}",
                    prefix, relative_ms, node_id, error
                );
            }
            DebugEvent::StateChange {
                node_id,
                key,
                old_value,
                new_value,
                ..
            } => {
                println!(
                    "  {} 🔄 [{:>4}ms] STATE CHANGE    │ node={} │ {}={} → {}",
                    prefix,
                    relative_ms,
                    node_id,
                    key,
                    old_value
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or("∅".to_string()),
                    new_value
                );
            }
        }
    }

    println!();
    println!("─── Session Recorder Summary ──────────────────────────────────");
    println!();

    // ─── 5. Show session recorder data ─────────────────────────────────
    let session_data = recorder.get_session("demo-session-001").await.unwrap();
    if let Some(s) = session_data {
        println!("  📦 Session ID:    {}", s.session_id);
        println!("  📊 Workflow:      {}", s.workflow_id);
        println!("  🔢 Event Count:   {}", s.event_count);
        println!("  📌 Status:        {}", s.status);
        println!("  ⏱️  Started:       {}ms", s.started_at);
        if let Some(ended) = s.ended_at {
            println!("  ⏱️  Ended:         {}ms", ended);
            println!("  ⏱️  Duration:      {}ms", ended - s.started_at);
        }
    }

    // ─── 6. Demonstrate replay capability ──────────────────────────────
    println!();
    println!("─── Time-Travel Replay (from SessionRecorder) ──────────────");
    println!();

    let recorded_events = recorder.get_events("demo-session-001").await.unwrap();
    println!("  📼 Replaying {} events from stored session...", recorded_events.len());
    println!();

    for (step, event) in recorded_events.iter().enumerate() {
        let event_json = serde_json::to_string_pretty(event).unwrap();
        let first_line = event_json.lines().next().unwrap_or("");
        println!(
            "  Step {}/{}: {} {}",
            step + 1,
            recorded_events.len(),
            event.event_type(),
            first_line
        );
    }

    println!();
    println!("─── Execution Result ──────────────────────────────────────────");
    println!();
    println!("  ✅ Status: {:?}", result.status);
    println!("  📊 Nodes executed: {}", result.node_records.len());
    for record in &result.node_records {
        println!(
            "     • {} ({:?}, {}ms)",
            record.node_id,
            record.status,
            record.ended_at - record.started_at
        );
    }

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  ✅ Demo complete! Telemetry infrastructure is working.     ║");
    println!("║  Next: Build time-travel UI on top of this data layer.      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}
