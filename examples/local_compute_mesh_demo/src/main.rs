//! Local Compute Mesh Demo with Execution Trace Visualization
//!
//! This demo showcases the compute mesh pipeline with execution trace visualization
//! to improve observability of how requests flow through the system.
//!
//! Pipeline: workflow → routing → backend → streaming → metrics → execution trace

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

// ============================================================================
// Trace Event Types
// ============================================================================

/// Represents a single trace event in the compute mesh pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// The stage name (e.g., "workflow", "router", "inference")
    pub stage: String,
    /// Optional detail/attribute (e.g., "policy=LocalFirstWithCloudFallback")
    pub detail: Option<String>,
    /// Timestamp when this event was recorded (milliseconds since epoch)
    pub timestamp_ms: u64,
}

/// Execution trace container
#[derive(Debug, Clone, Default)]
pub struct ExecutionTrace {
    events: Vec<TraceEvent>,
    start_time_ms: u64,
}

impl ExecutionTrace {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            start_time_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// Record a trace event
    pub fn record(&mut self, stage: impl Into<String>, detail: Option<impl Into<String>>) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.events.push(TraceEvent {
            stage: stage.into(),
            detail: detail.map(|d| d.into()),
            timestamp_ms: timestamp,
        });
    }

    /// Print formatted execution trace
    pub fn print_trace(&self) {
        println!("\n==== Compute Mesh Execution Trace ====\n");

        for event in &self.events {
            match &event.detail {
                Some(detail) => println!("[trace] {} = {}", event.stage, detail),
                None => println!("[trace] {}", event.stage),
            }
        }

        println!();
    }

    /// Export trace as JSON
    pub fn to_json(&self, request_id: &str) -> String {
        #[derive(Serialize)]
        struct TraceOutput<'a> {
            request_id: &'a str,
            stages: &'a Vec<TraceEvent>,
        }

        let output = TraceOutput {
            request_id,
            stages: &self.events,
        };

        serde_json::to_string_pretty(&output).unwrap_or_default()
    }
}

// ============================================================================
// Compute Mesh Components
// ============================================================================

/// Routing policy for compute mesh
#[derive(Debug, Clone)]
pub enum RoutingPolicy {
    LocalOnly,
    CloudOnly,
    LocalFirstWithCloudFallback,
}

/// Backend selection
#[derive(Debug, Clone)]
pub enum Backend {
    Local,
    Cloud,
}

/// Workflow step
#[derive(Debug, Clone)]
pub enum WorkflowStep {
    GenerateResponse,
    ProcessInput,
    FormatOutput,
}

// ============================================================================
// Compute Mesh Pipeline
// ============================================================================

/// Main compute mesh pipeline with trace instrumentation
pub struct ComputeMeshPipeline {
    trace: Arc<RwLock<ExecutionTrace>>,
    policy: RoutingPolicy,
}

impl ComputeMeshPipeline {
    pub fn new(policy: RoutingPolicy) -> Self {
        Self {
            trace: Arc::new(RwLock::new(ExecutionTrace::new())),
            policy,
        }
    }

    /// Execute the full pipeline
    pub async fn execute(&self, input: &str) -> Result<String, String> {
        let trace = self.trace.clone();

        // Stage 1: Workflow Start
        {
            trace
                .write()
                .unwrap()
                .record("workflow.start", None::<&str>);
        }
        info!("[workflow] executing step: generate_response");

        // Stage 2: Routing Decision
        let backend = {
            let policy_str = match self.policy {
                RoutingPolicy::LocalOnly => "LocalOnly",
                RoutingPolicy::CloudOnly => "CloudOnly",
                RoutingPolicy::LocalFirstWithCloudFallback => "LocalFirstWithCloudFallback",
            };
            trace
                .write()
                .unwrap()
                .record("router.policy", Some(policy_str));

            // Simulate routing logic
            let selected = Backend::Local;
            let backend_str = match selected {
                Backend::Local => "local",
                Backend::Cloud => "cloud",
            };
            trace
                .write()
                .unwrap()
                .record("router.backend_selection", Some(backend_str));

            info!("[router] policy: {}", policy_str);
            info!("[router] selected backend: {}", backend_str);

            selected
        };

        // Stage 3: Inference
        {
            trace
                .write()
                .unwrap()
                .record("inference.start", None::<&str>);
        }
        info!("[inference] sending request to orchestrator...");

        // Simulate inference with streaming
        let tokens = self.simulate_inference(input, &backend).await?;

        // Stage 4: Metrics
        let latency_ms = {
            let duration = 820; // Simulated latency
            trace
                .write()
                .unwrap()
                .record("metrics.latency_ms", Some(duration.to_string()));
            info!("[metrics] latency_ms = {}", duration);
            duration
        };

        // Stage 5: Workflow Complete
        {
            trace
                .write()
                .unwrap()
                .record("workflow.complete", None::<&str>);
        }

        // Print the execution trace
        self.print_trace();

        Ok(format!(
            "Processed '{}' with {} tokens (latency: {}ms)",
            input, tokens, latency_ms
        ))
    }

    /// Simulate inference with token streaming
    async fn simulate_inference(&self, input: &str, _backend: &Backend) -> Result<usize, String> {
        // Simulate token streaming
        let _words: Vec<&str> = input.split_whitespace().collect();

        // Simulate streaming tokens
        let streaming_words = [
            "This",
            "is",
            "a",
            "simulated",
            "response",
            "from",
            "the",
            "compute",
            "mesh",
        ];

        for (i, word) in streaming_words.iter().enumerate() {
            let detail = format!("token_{}", i + 1);
            self.trace
                .write()
                .unwrap()
                .record("streaming.tokens", Some(detail));
            info!("[stream] {}", word);
            // Small delay to simulate streaming
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        Ok(streaming_words.len())
    }

    /// Print the execution trace
    pub fn print_trace(&self) {
        let trace = self.trace.read().unwrap();
        trace.print_trace();
    }

    /// Get JSON export of trace
    pub fn export_trace_json(&self, request_id: &str) -> String {
        let trace = self.trace.read().unwrap();
        trace.to_json(request_id)
    }
}

// ============================================================================
// Main Function
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== Local Compute Mesh Demo with Execution Trace ===\n");

    // Get user prompt from command line arguments
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Explain photosynthesis".to_string());

    println!("User prompt: {}\n", prompt);

    // Create compute mesh pipeline with LocalFirstWithCloudFallback policy
    let pipeline = ComputeMeshPipeline::new(RoutingPolicy::LocalFirstWithCloudFallback);

    // Execute the pipeline
    let request_id = Uuid::new_v4().to_string();
    info!("Request ID: {}\n", request_id);

    match pipeline.execute(&prompt).await {
        Ok(result) => {
            println!("\n--- JSON Trace Export ---\n");
            println!("{}", pipeline.export_trace_json(&request_id));
            println!("\nResult: {}", result);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}
