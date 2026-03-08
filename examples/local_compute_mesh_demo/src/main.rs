//! Local Compute Mesh Demo - Workflow + Inference Routing Integration
//!
//! This example demonstrates the MoFA Compute Mesh demo pipeline:
//! User Prompt → Workflow Execution → Inference Router → Local Backend → Generated Response
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐    ┌─────────────┐    ┌────────────────┐    ┌──────────────────┐    ┌───────────┐
//! │ User Prompt │───▶│    Workflow │───▶│ Inference      │───▶│ Local Inference  │───▶│ Response  │
//! │             │    │   Engine    │    │    Router      │    │    Backend       │    │           │
//! └─────────────┘    └─────────────┘    └────────────────┘    └──────────────────┘    └───────────┘
//! ```
//!
//! # Running the Demo
//!
//! ```bash
//! cargo run -p local_compute_mesh_demo
//! ```

use std::fs;

use mofa_foundation::inference::RoutingPolicy;
use mofa_foundation::inference::{
    InferenceOrchestrator, InferenceRequest, OrchestratorConfig, RoutedBackend,
};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

/// Demo workflow configuration loaded from workflow.yaml
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WorkflowConfig {
    workflow: WorkflowDefinition,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WorkflowDefinition {
    name: String,
    description: String,
    steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WorkflowStep {
    id: String,
    #[serde(rename = "type")]
    step_type: String,
    description: Option<String>,
    /// Prompt for inference steps
    prompt: Option<String>,
}

/// Prints the demo architecture header
fn print_demo_header() {
    info!("");
    info!("========================================");
    info!("  MoFA Local Compute Mesh Demo         ");
    info!("  Workflow + Inference Routing         ");
    info!("========================================");
    info!("");
}

/// Prints the pipeline visualization
fn print_pipeline() {
    info!("Pipeline Architecture:");
    info!("");
    info!(
        "    ┌─────────────┐    ┌─────────────┐    ┌────────────────┐    ┌──────────────────┐    ┌───────────┐"
    );
    info!(
        "    │ User Prompt │───▶│    Workflow │───▶│ Inference      │───▶│ Local Inference  │───▶│ Response  │"
    );
    info!(
        "    │             │    │   Engine    │    │    Router      │    │    Backend       │    │           │"
    );
    info!(
        "    └─────────────┘    └─────────────┘    └────────────────┘    └──────────────────┘    └───────────┘"
    );
    info!("");
    info!("  (1)              (2)                 (3)                  (4)                    (5)");
    info!("");
}

/// Loads the workflow configuration from workflow.yaml
fn load_workflow_config() -> Result<WorkflowConfig, Box<dyn std::error::Error>> {
    info!("Loading workflow definition from workflow.yaml...");

    // Get the path to the example directory
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workflow_path = std::path::Path::new(manifest_dir).join("workflow.yaml");

    let config_content = fs::read_to_string(workflow_path)?;
    let config: WorkflowConfig = serde_yaml::from_str(&config_content)?;

    info!("Loaded workflow: {}", config.workflow.name);
    info!("Description: {}", config.workflow.description);
    info!("Number of steps: {}", config.workflow.steps.len());

    for (i, step) in config.workflow.steps.iter().enumerate() {
        let desc = step.description.as_deref().unwrap_or("N/A");
        info!(
            "  Step {}: {} (type: {}) - {}",
            i + 1,
            step.id,
            step.step_type,
            desc
        );
    }

    info!("");
    Ok(config)
}

/// Execute workflow step - logs the execution
fn execute_workflow_step(step: &WorkflowStep) {
    info!("[workflow] executing step: {}", step.id);
}

/// Find and execute the inference step from workflow
fn execute_inference_step(
    config: &WorkflowConfig,
    orchestrator: &mut InferenceOrchestrator,
) -> String {
    // Find the inference step in the workflow
    let inference_step = config
        .workflow
        .steps
        .iter()
        .find(|s| s.step_type == "inference");

    if let Some(step) = inference_step {
        // Get the prompt from the step, or use a default
        let prompt = step
            .prompt
            .clone()
            .unwrap_or_else(|| "Hello, how are you?".to_owned());

        info!("[workflow] executing step: {}", step.id);

        // Create inference request
        let request = InferenceRequest::new("llama-3-7b", prompt, 7168);

        info!("[inference] sending request to orchestrator...");

        // Call the orchestrator to get the result
        let result = orchestrator.infer(&request);

        // Log the selected backend
        match &result.routed_to {
            RoutedBackend::Local { model_id } => {
                info!("[router] selected backend: local({})", model_id);
            }
            RoutedBackend::Cloud { provider } => {
                info!("[router] selected backend: cloud({})", provider);
            }
            RoutedBackend::Rejected { reason } => {
                info!("[router] request rejected: {}", reason);
            }
        }

        info!("[inference] generating response...");
        info!("[result] {}", result.output);

        result.output
    } else {
        // No inference step found, use mock
        mock_generate("No inference step found in workflow")
    }
}

/// Mock generator for fallback when no local backend is available
fn mock_generate(prompt: &str) -> String {
    // Simple mock response based on the prompt
    if prompt.contains("photosynthesis") {
        "Photosynthesis is the process by which plants convert light energy into chemical energy, \
        producing glucose and oxygen from carbon dioxide and water. It occurs in the chloroplasts \
        of plant cells, primarily using chlorophyll to capture sunlight.".to_string()
    } else {
        format!("Mock response for prompt: {prompt}")
    }
}

/// Main entry point
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with a simple console subscriber
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    // Print startup message
    info!("Starting MoFA local compute mesh demo...");
    info!("");

    // Print demo header
    print_demo_header();

    // Print pipeline visualization
    print_pipeline();

    // Load workflow configuration
    let config = load_workflow_config()?;

    // Create the inference orchestrator with default configuration
    let orchestrator_config = OrchestratorConfig {
        routing_policy: RoutingPolicy::LocalFirstWithCloudFallback,
        memory_capacity_mb: 16384,
        ..Default::default()
    };

    let mut orchestrator = InferenceOrchestrator::new(orchestrator_config);

    info!("Orchestrator initialized with policy: LocalFirstWithCloudFallback");
    info!("");

    // Execute workflow steps
    for step in &config.workflow.steps {
        match step.step_type.as_str() {
            "input" => {
                info!("[workflow] step: {} - accepting user input", step.id);
            }
            "workflow" => {
                execute_workflow_step(step);
            }
            "inference" => {
                // Execute inference step with the orchestrator
                let response = execute_inference_step(&config, &mut orchestrator);
                info!("");
                info!("[response] Final output: {}", response);
            }
            "output" => {
                info!("[workflow] step: {} - returning response", step.id);
            }
            _ => {
                info!("[workflow] step: {} - unknown type", step.id);
            }
        }
    }

    // Print completion message
    info!("");
    info!("========================================");
    info!("  Demo completed successfully!");
    info!("========================================");
    info!("");
    info!("This demo demonstrated:");
    info!("  1. Workflow configuration loading");
    info!("  2. Step execution through the pipeline");
    info!("  3. Inference orchestration with routing");
    info!("  4. Response generation");
    info!("");

    Ok(())
}
