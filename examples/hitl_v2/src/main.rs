use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use mofa_foundation::workflow::{
    ExecutorConfig, WorkflowExecutor, WorkflowGraph, WorkflowNode, EdgeConfig,
    WorkflowContextSnapshot, WorkflowValue, WorkflowContext, WorkflowStatus,
};
use std::fs;
use std::path::PathBuf;
use tracing::{info, Level};

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
    /// Start a new workflow execution
    Start,

    /// Resume workflow from saved state with human input
    Resume {
        /// Human feedback/input
        #[arg(long)]
        input: String,
    },

    /// List saved workflow states
    List,
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
                "    cargo run --example hitl_v2 -- --state-dir {} resume --input 'Your feedback here'",
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
            "\n⏸ Workflow still paused. Further interaction needed.\n"
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
