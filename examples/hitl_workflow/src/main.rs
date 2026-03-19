//! HITL Workflow Example
//!
//! This example demonstrates Human-in-the-Loop (HITL) capabilities:
//!
//! **Legacy Approach** (still supported):
//! - Uses `Wait` nodes with in-memory state
//! - Basic pause/resume with file-based state snapshots
//! - Simple workflow execution with manual review points
//!
//! **New Unified HITL System** (Phase 1+):
//! - Uses unified `ReviewRequest` and `ReviewContext` from `mofa-kernel::hitl`
//! - Rich execution trace capture
//! - Review policies and metadata
//! - Production-ready features (will be available after Phase 2 implementation)
//!
//! Run: cargo run --example hitl_workflow -- start
//!      cargo run --example hitl_workflow -- resume --input "Your feedback"
//!      cargo run --example hitl_workflow -- unified --example all

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use mofa_foundation::workflow::{
    ExecutorConfig, WorkflowExecutor, WorkflowGraph, WorkflowNode, EdgeConfig,
    WorkflowContextSnapshot, WorkflowValue, WorkflowContext, WorkflowStatus,
};
use mofa_kernel::hitl::{
    ExecutionStep, ExecutionTrace, ReviewContext, ReviewRequest, ReviewType,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, Level};

mod integration_examples;
#[cfg(feature = "http-api")]
mod api_example;

use integration_examples::run_integration_examples;
#[cfg(feature = "http-api")]
use api_example::test_http_api;

#[derive(Parser)]
#[command(name = "HITL Workflow Example")]
#[command(about = "Human-in-the-Loop workflow execution example")]
struct Cli {
    /// Directory for storing workflow state
    #[arg(long, default_value = "./workflow_state")]
    state_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new workflow execution (legacy Wait node approach)
    Start,

    /// Resume workflow from saved state with human input (legacy approach)
    Resume {
        /// Human feedback/input
        #[arg(long)]
        input: String,
    },

    /// List saved workflow states
    List,

    /// Demonstrate new unified HITL system (Phase 1: Kernel abstractions)
    Unified {
        /// Example to run: basic, context, policies, lifecycle, metadata
        #[arg(default_value = "all")]
        example: String,
    },

    /// Demonstrate HITL integration examples (Phase 2+: Foundation layer)
    Integration {
        /// Example to run: manager, workflow, tool, webhook, rate_limit, multi_tenant, end_to_end, all
        #[arg(default_value = "all")]
        example: String,
    },

    /// Demonstrate HTTP API endpoints (requires http-api feature)
    #[cfg(feature = "http-api")]
    ApiExample,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let cli = Cli::parse();

    // Ensure state directory exists
    fs::create_dir_all(&cli.state_dir)?;

    match cli.command {
        Commands::Start => start_workflow(&cli.state_dir).await?,
        Commands::Resume { input } => resume_workflow(&cli.state_dir, &input).await?,
        Commands::List => list_saved_states(&cli.state_dir)?,
        Commands::Unified { example } => run_unified_examples(&example).await?,
        Commands::Integration { example } => run_integration_examples(&example).await?,
        #[cfg(feature = "http-api")]
        Commands::ApiExample => test_http_api().await?,
    }

    Ok(())
}

/// Create a sample 3-node workflow: Draft → HumanReview → Revise
fn create_sample_workflow() -> WorkflowGraph {
    let mut graph = WorkflowGraph::new("content_review_workflow", "Content Review Workflow");

    // Add a start node
    let start_node = WorkflowNode::start("start");
    graph.add_node(start_node);

    // Node 1: Draft - simulates content generation (simple task node)
    let draft_node = WorkflowNode::task("draft", "Draft Content", |_ctx, input| async move {
        // Simulate draft creation
        Ok(WorkflowValue::String(format!(
            "Draft created from: {}",
            input.as_str().unwrap_or("empty")
        )))
    });
    graph.add_node(draft_node);

    // Node 2: HumanReview - HITL wait point
    let review_node = WorkflowNode::wait("human_review", "Human Review", "review_feedback");
    graph.add_node(review_node);

    // Node 3: Revise - final text processing (simple task node)
    let revise_node = WorkflowNode::task("revise", "Revise Content", |_ctx, input| async move {
        // Simulate revision based on feedback
        Ok(WorkflowValue::String(format!(
            "Revised content: {}",
            input.as_str().unwrap_or("empty")
        )))
    });
    graph.add_node(revise_node);

    // Add an end node
    let end_node = WorkflowNode::end("end");
    graph.add_node(end_node);

    // Add edges: start → draft → human_review → revise → end
    graph.add_edge(EdgeConfig::new("start", "draft"));
    graph.add_edge(EdgeConfig::new("draft", "human_review"));
    graph.add_edge(EdgeConfig::new("human_review", "revise"));
    graph.add_edge(EdgeConfig::new("revise", "end"));

    graph
}

/// Start a new workflow execution
async fn start_workflow(state_dir: &PathBuf) -> Result<()> {
    info!("Starting new HITL workflow...");

    let graph = create_sample_workflow();
    let executor = WorkflowExecutor::new(ExecutorConfig::default());

    // Initial input for the workflow
    let input = WorkflowValue::String("Initial draft: 'Hello World Example'".to_string());

    // Execute the workflow
    let record = executor
        .execute(&graph, input)
        .await
        .map_err(|e| anyhow!("Workflow execution failed: {}", e))?;

    info!("Workflow record: {:?}", record);

    // If the workflow paused, save state for later resumption
    if let WorkflowStatus::Paused = record.status {
        if let Some(ctx) = record.context {
            info!(
                "Workflow paused at node: {:?}, execution ID: {}",
                ctx.last_waiting_node.read().await, record.execution_id
            );

            // Create and save the snapshot
            let snapshot = ctx.snapshot().await;
            let snapshot_path = state_dir.join("workflow_state.json");
            let snapshot_json = serde_json::to_string_pretty(&snapshot)?;
            fs::write(&snapshot_path, snapshot_json)?;

            println!("\n✓ Workflow paused at Human Review step.");
            println!("  State saved to: {}", snapshot_path.display());
            println!("  To resume with feedback, run:");
            println!(
                "    cargo run --example hitl_workflow -- --state-dir {} resume --input 'Your feedback here'",
                state_dir.display()
            );
        } else {
            return Err(anyhow!(
                "Workflow paused but no context was returned from executor"
            ));
        }
    } else {
        info!(
            "Workflow completed with status: {:?}",
            record.status
        );
    }

    Ok(())
}

/// Resume workflow from saved state with human input
async fn resume_workflow(state_dir: &PathBuf, human_input: &str) -> Result<()> {
    info!("Resuming HITL workflow with input: {}", human_input);

    let state_file = state_dir.join("workflow_state.json");

    if !state_file.exists() {
        return Err(anyhow!(
            "No saved workflow state found at {}",
            state_file.display()
        ));
    }

    let snapshot_json = fs::read_to_string(&state_file)?;
    let snapshot: WorkflowContextSnapshot = serde_json::from_str(&snapshot_json)?;

    info!("Loaded workflow snapshot (version: {})", snapshot.version);

    let graph = create_sample_workflow();
    let executor = WorkflowExecutor::new(ExecutorConfig::default());

    // Reconstruct context from snapshot
    let ctx = WorkflowContext::from_snapshot(snapshot.clone());

    // Get the waiting node ID
    let waiting_node_id = snapshot
        .last_waiting_node
        .as_ref()
        .ok_or_else(|| anyhow!("No waiting node found in snapshot"))?;

    // Resume execution with human input
    let human_input_value = WorkflowValue::String(human_input.to_string());

    let record = executor
        .resume_with_human_input(&graph, ctx, waiting_node_id, human_input_value)
        .await
        .map_err(|e| anyhow!("Workflow resume failed: {}", e))?;

    info!("Resume record: {:?}", record);

    if let WorkflowStatus::Paused = record.status {
        println!(
            "\nWorkflow still paused. Further interaction needed.\n"
        );
    } else {
        println!(
            "\n✓ Workflow completed with status: {:?}\n",
            record.status
        );
    }

    Ok(())
}

/// List all saved workflow states
fn list_saved_states(state_dir: &PathBuf) -> Result<()> {
    if !state_dir.exists() {
        println!("State directory does not exist yet.");
        return Ok(());
    }

    println!("\nSaved workflow states:");
    for entry in fs::read_dir(state_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name() {
                println!("  - {}", filename.to_string_lossy());
            }
        }
    }

    Ok(())
}

// =============================================================================
// New Unified HITL System Examples (Phase 1: Kernel Abstractions)
// These examples demonstrate the new unified HITL system that extends
// the legacy Wait node approach with persistent review queues, policies, etc.
// =============================================================================

/// Run unified HITL system examples
async fn run_unified_examples(example: &str) -> Result<()> {
    info!("=== Unified HITL System Examples (Phase 1) ===\n");
    info!("Note: These examples show the NEW unified system that will");
    info!("      extend the legacy Wait node approach with:");
    info!("      - Persistent review queue (database-backed)");
    info!("      - Review policies (configurable rules)");
    info!("      - Rich context capture (execution traces, telemetry)");
    info!("      - Production-ready features (rate limiting, webhooks)\n");

    match example {
        "basic" => example_unified_basic().await?,
        "context" => example_unified_context().await?,
        "policies" => example_unified_policies().await?,
        "lifecycle" => example_unified_lifecycle().await?,
        "metadata" => example_unified_metadata().await?,
        "all" => {
            example_unified_basic().await?;
            example_unified_context().await?;
            example_unified_policies().await?;
            example_unified_lifecycle().await?;
            example_unified_metadata().await?;
        }
        _ => {
            return Err(anyhow!(
                "Unknown example: {}. Use: basic, context, policies, lifecycle, metadata, or all",
                example
            ));
        }
    }

    info!("\n=== Unified HITL examples completed ===");
    info!("To see legacy approach: cargo run --example hitl_workflow -- start");
    Ok(())
}

/// Example 1: Basic Review Request Creation
async fn example_unified_basic() -> Result<()> {
    info!("--- Example 1: Basic Review Request Creation ---");

    let trace = ExecutionTrace {
        steps: vec![ExecutionStep {
            step_id: "step_1".to_string(),
            step_type: "data_fetch".to_string(),
            timestamp_ms: 1000,
            input: Some(serde_json::json!({"url": "https://api.example.com/data"})),
            output: Some(serde_json::json!({"status": "success", "data": [1, 2, 3]})),
            metadata: HashMap::new(),
        }],
        duration_ms: 500,
    };

    let context = ReviewContext::new(
        trace,
        serde_json::json!({"action": "process_payment", "amount": 1000}),
    );

    let request = ReviewRequest::new(
        "exec_123",
        ReviewType::Approval,
        context,
    )
    .with_node_id("payment_node")
    .with_expiration(chrono::Utc::now() + chrono::Duration::hours(24));

    info!("Created review request: {}", request.id.as_str());
    info!("  Execution ID: {}", request.execution_id);
    info!("  Review Type: {:?}", request.review_type);
    info!("  Status: {:?}", request.status);
    info!("  Expires at: {:?}", request.expires_at);

    Ok(())
}

/// Example 2: Review Context with Execution Trace
async fn example_unified_context() -> Result<()> {
    info!("\n--- Example 2: Review Context with Execution Trace ---");

    let trace = ExecutionTrace {
        steps: vec![
            ExecutionStep {
                step_id: "fetch_user_data".to_string(),
                step_type: "api_call".to_string(),
                timestamp_ms: 0,
                input: Some(serde_json::json!({"user_id": "user_123"})),
                output: Some(serde_json::json!({
                    "name": "John Doe",
                    "email": "john@example.com",
                    "balance": 5000
                })),
                metadata: HashMap::from([
                    ("latency_ms".to_string(), serde_json::json!(150)),
                    ("status_code".to_string(), serde_json::json!(200)),
                ]),
            },
            ExecutionStep {
                step_id: "calculate_interest".to_string(),
                step_type: "computation".to_string(),
                timestamp_ms: 200,
                input: Some(serde_json::json!({"balance": 5000, "rate": 0.05})),
                output: Some(serde_json::json!({"interest": 250})),
                metadata: HashMap::new(),
            },
        ],
        duration_ms: 450,
    };

    let context = ReviewContext::new(
        trace,
        serde_json::json!({"user_id": "user_123", "action": "calculate_interest"}),
    )
    .with_output(serde_json::json!({
        "final_balance": 5250,
        "interest_applied": 250
    }));

    info!("Review context created with {} execution steps", context.execution_trace.steps.len());
    info!("  Input data: {}", context.input_data);
    info!("  Output data: {:?}", context.output_data);
    info!("  Total duration: {}ms", context.execution_trace.duration_ms);

    Ok(())
}

/// Example 3: Review Policies
async fn example_unified_policies() -> Result<()> {
    info!("\n--- Example 3: Review Policies ---");

    use mofa_kernel::hitl::{AlwaysReviewPolicy, NeverReviewPolicy, ReviewPolicy};

    let always_policy: Arc<dyn ReviewPolicy> = Arc::new(AlwaysReviewPolicy);
    let context = create_sample_context();
    
    if let Some(request) = always_policy.should_request_review(&context).await? {
        info!("AlwaysReviewPolicy: Review requested - {}", request.id.as_str());
    }

    let request = ReviewRequest::new("exec_456", ReviewType::Approval, context.clone());
    let can_auto_approve = always_policy.can_auto_approve(&request).await?;
    info!("AlwaysReviewPolicy can auto-approve: {}", can_auto_approve);

    let never_policy: Arc<dyn ReviewPolicy> = Arc::new(NeverReviewPolicy);
    let should_review = never_policy.should_request_review(&context).await?;
    info!("NeverReviewPolicy should review: {:?}", should_review.is_some());

    Ok(())
}

/// Example 4: Review Request Lifecycle
async fn example_unified_lifecycle() -> Result<()> {
    info!("\n--- Example 4: Review Request Lifecycle ---");

    use mofa_kernel::hitl::{ReviewResponse, ReviewStatus};

    let mut request = ReviewRequest::new(
        "exec_789",
        ReviewType::Approval,
        create_sample_context(),
    );

    info!("1. Created review request: {}", request.id.as_str());
    info!("   Status: {:?}", request.status);
    info!("   Is resolved: {}", request.is_resolved());

    request.status = ReviewStatus::Approved;
    request.resolved_at = Some(chrono::Utc::now());
    request.resolved_by = Some("reviewer_123".to_string());
    request.response = Some(ReviewResponse::Approved {
        comment: Some("Looks good!".to_string()),
    });

    info!("2. Review approved");
    info!("   Status: {:?}", request.status);
    info!("   Resolved by: {:?}", request.resolved_by);

    Ok(())
}

/// Example 5: Review Metadata and Priority
async fn example_unified_metadata() -> Result<()> {
    info!("\n--- Example 5: Review Metadata and Priority ---");

    use mofa_kernel::hitl::ReviewMetadata;

    let mut metadata = ReviewMetadata::default();
    metadata.priority = 9;
    metadata.assigned_to = Some("senior_reviewer@example.com".to_string());
    metadata.tags = vec!["critical".to_string(), "payment".to_string()];
    metadata.custom.insert(
        "department".to_string(),
        serde_json::json!("finance"),
    );

    let request = ReviewRequest::new(
        "exec_critical",
        ReviewType::Approval,
        create_sample_context(),
    )
    .with_metadata(metadata);

    info!("Review request with metadata:");
    info!("  Priority: {}", request.metadata.priority);
    info!("  Assigned to: {:?}", request.metadata.assigned_to);
    info!("  Tags: {:?}", request.metadata.tags);

    Ok(())
}

/// Helper function to create a sample review context
fn create_sample_context() -> ReviewContext {
    let trace = ExecutionTrace {
        steps: vec![ExecutionStep {
            step_id: "sample_step".to_string(),
            step_type: "sample".to_string(),
            timestamp_ms: 0,
            input: Some(serde_json::json!({"test": "data"})),
            output: Some(serde_json::json!({"result": "success"})),
            metadata: HashMap::new(),
        }],
        duration_ms: 100,
    };

    ReviewContext::new(
        trace,
        serde_json::json!({"example": "input"}),
    )
}
