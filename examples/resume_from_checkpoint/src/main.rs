//! # Resume-from-Checkpoint Example
//!
//! Demonstrates the corrected behaviour of `resume_from_checkpoint` (fix #994).
//!
//! ## What this example shows
//!
//! * A workflow that pauses at a `Wait` (human-review) node.
//! * How to serialise the mid-execution state into an `ExecutionCheckpoint`.
//! * How `resume_from_checkpoint` correctly **preserves** `Paused` status when
//!   the resumed execution hits another Wait node, instead of incorrectly
//!   overwriting it with `Completed`.
//! * How to supply the human decision and drive the workflow through to
//!   `Completed` via a second resume cycle.
//! * The `WorkflowEnd` debug-telemetry event being emitted on both the initial
//!   execute path and the resume path.
//!
//! ## Running
//!
//! ```bash
//! cargo run --example resume_from_checkpoint
//! ```
//!
//! ## Workflow graph
//!
//! ```text
//! start ──► validate_input ──► wait_for_approval ──► finalize ──► end
//!                    (automatic)          (pauses)       (automatic)
//! ```

use mofa_foundation::workflow::{
    ExecutionCheckpoint, ExecutorConfig, WorkflowExecutor, WorkflowGraph, WorkflowNode,
    WorkflowStatus, WorkflowValue,
};
use mofa_foundation::workflow::{DebugEvent, TelemetryEmitter};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, Level};

// ---------------------------------------------------------------------------
// Minimal in-process telemetry collector
// ---------------------------------------------------------------------------

/// Records the names of emitted `DebugEvent` variants so we can print them.
#[derive(Clone)]
struct ConsoleTelemetry {
    events: Arc<RwLock<Vec<String>>>,
}

impl ConsoleTelemetry {
    fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn print_summary(&self) {
        let events = self.events.read().await;
        println!("\n📡 Telemetry events captured ({} total):", events.len());
        for e in events.iter() {
            println!("   • {e}");
        }
    }
}

#[async_trait::async_trait]
impl TelemetryEmitter for ConsoleTelemetry {
    async fn emit(&self, event: DebugEvent) {
        let label = match &event {
            DebugEvent::WorkflowStart { workflow_id, .. } => {
                format!("WorkflowStart  [wf={workflow_id}]")
            }
            DebugEvent::WorkflowEnd { workflow_id, status, .. } => {
                format!("WorkflowEnd    [wf={workflow_id}, status={status}]")
            }
            DebugEvent::NodeStart { node_id, .. } => format!("NodeStart      [node={node_id}]"),
            DebugEvent::NodeEnd { node_id, .. } => format!("NodeEnd        [node={node_id}]"),
            _ => "Other".to_string(),
        };
        self.events.write().await.push(label);
    }

    fn is_enabled(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Build the workflow graph
// ---------------------------------------------------------------------------

fn build_approval_workflow() -> WorkflowGraph {
    let mut graph = WorkflowGraph::new("approval_wf", "Approval Workflow");

    // Start node
    graph.add_node(WorkflowNode::start("start"));

    // Automatic validation step
    graph.add_node(WorkflowNode::task(
        "validate_input",
        "Validate Input",
        |_ctx, input| async move {
            println!("  🔍  validate_input: received input = {input:?}");
            // Simulate basic validation delay
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(WorkflowValue::String("validated".to_string()))
        },
    ));

    // Wait node: execution pauses here until a human approves
    graph.add_node(WorkflowNode::wait(
        "wait_for_approval",
        "Wait For Approval",
        "human_approval", // event type tag
    ));

    // Second automatic step, only reached after approval
    graph.add_node(WorkflowNode::task(
        "finalize",
        "Finalize",
        |_ctx, input| async move {
            println!("  ✅  finalize: running with approval = {input:?}");
            Ok(WorkflowValue::String("done".to_string()))
        },
    ));

    graph.add_node(WorkflowNode::end("end"));

    graph.connect("start", "validate_input");
    graph.connect("validate_input", "wait_for_approval");
    graph.connect("wait_for_approval", "finalize");
    graph.connect("finalize", "end");

    graph
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let telemetry = Arc::new(ConsoleTelemetry::new());

    let executor = WorkflowExecutor::new(ExecutorConfig::default())
        .with_telemetry(telemetry.clone());

    let graph = build_approval_workflow();

    // -----------------------------------------------------------------------
    // Phase 1: Initial execution — pauses at `wait_for_approval`
    // -----------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Phase 1: Initial execution");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let first_run = executor
        .execute(&graph, WorkflowValue::String("order-42".to_string()))
        .await
        .expect("execute() must not return Err");

    println!("\n  Status after Phase 1: {:?}", first_run.status);
    assert!(
        matches!(first_run.status, WorkflowStatus::Paused),
        "Workflow should be Paused at the Wait node"
    );
    assert!(
        first_run.context.is_some(),
        "ExecutionRecord.context must be set so we can resume"
    );

    info!("Phase 1 complete — workflow is paused waiting for human approval.");

    // -----------------------------------------------------------------------
    // Phase 2: Simulate the system restarting (checkpoint-based resume)
    //
    // In production the `WorkflowContext` snapshot would be persisted to a
    // database between Phase 1 and Phase 2.  Here we construct the checkpoint
    // directly from the first run's outputs to keep the example self-contained.
    // -----------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Phase 2: Resume from checkpoint (before human provides input)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Nodes that finished during Phase 1
    let checkpoint = ExecutionCheckpoint {
        execution_id: first_run.execution_id.clone(),
        workflow_id: "approval_wf".to_string(),
        // `start` and `validate_input` completed; `wait_for_approval` did NOT.
        completed_nodes: vec!["start".to_string(), "validate_input".to_string()],
        node_outputs: first_run.outputs.clone(),
        variables: std::collections::HashMap::new(),
        timestamp: 0,
    };

    let resumed = executor
        .resume_from_checkpoint(&graph, checkpoint)
        .await
        .expect("resume_from_checkpoint must not return Err");

    println!("\n  Status after Phase 2 resume: {:?}", resumed.status);

    // ✅ Key invariant guaranteed by fix #994:
    // The resumed execution hit `wait_for_approval` again, so status must be
    // Paused — NOT incorrectly overwritten to Completed.
    assert!(
        matches!(resumed.status, WorkflowStatus::Paused),
        "BUG #994 regression: status must remain Paused after checkpoint \
         resume that encounters a Wait node. Got: {:?}",
        resumed.status
    );
    assert!(
        resumed.context.is_some(),
        "context must be set so we can do another resume cycle"
    );

    info!("Phase 2 complete — status correctly preserved as Paused.");

    // -----------------------------------------------------------------------
    // Phase 3: Human provides approval → resume with human input
    //
    // Now that a human has approved, we use `resume_with_human_input` to
    // continue past the Wait node and drive the workflow to Completed.
    // -----------------------------------------------------------------------
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Phase 3: Human approves → resume_with_human_input");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Retrieve the live context that was saved on the last Paused record
    let live_ctx = resumed.context.expect("context must be present");

    let final_record = executor
        .resume_with_human_input(
            &graph,
            live_ctx,
            "wait_for_approval",
            WorkflowValue::String("approved by alice".to_string()),
        )
        .await
        .expect("resume_with_human_input must not return Err");

    println!("\n  Status after Phase 3: {:?}", final_record.status);
    assert!(
        matches!(final_record.status, WorkflowStatus::Completed),
        "Expected Completed after human approval. Got: {:?}",
        final_record.status
    );

    // Show final outputs
    println!("\n  Final outputs:");
    for (node_id, value) in &final_record.outputs {
        println!("    {node_id} → {value:?}");
    }

    info!("Phase 3 complete — workflow reached Completed. ✓");

    // -----------------------------------------------------------------------
    // Print telemetry summary
    // -----------------------------------------------------------------------
    telemetry.print_summary().await;

    println!("\n✅  All phases completed successfully!\n");
}
