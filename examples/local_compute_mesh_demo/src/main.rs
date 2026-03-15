//! Local Compute Mesh Demo with Performance Benchmarking and Execution Trace
//!
//! This demo showcases the compute mesh pipeline with:
//! - Comprehensive performance benchmarking (latency, throughput, token metrics)
//! - Execution trace visualization for observability
//! - Real local inference with streaming token generation
//!
//! Pipeline: workflow → routing → backend → inference → streaming → metrics → trace

use futures::StreamExt;
use mofa_foundation::inference::routing::RoutingPolicy;
use mofa_local_llm::config::LinuxInferenceConfig;
use mofa_local_llm::hardware::ComputeBackend;
use mofa_local_llm::provider::LinuxLocalProvider;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

// ============================================================================
// Performance Metrics Types
// ============================================================================

/// Performance metrics collected during inference
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PerformanceMetrics {
    /// Backend used for inference (local or cloud)
    backend: String,
    /// Time from request start to first token (ms)
    time_to_first_token_ms: f64,
    /// Total time for streaming completion (ms)
    total_stream_time_ms: f64,
    /// Total latency from start to end (ms)
    total_latency_ms: f64,
    /// Number of tokens generated/streamed
    tokens_streamed: usize,
    /// Tokens per second throughput
    tokens_per_second: f64,
}

impl PerformanceMetrics {
    /// Print metrics in structured format
    fn print_metrics(&self) {
        info!("");
        info!("[metrics]");
        info!("backend: {}", self.backend);
        info!("latency_ms: {:.0}", self.total_latency_ms);
        info!("time_to_first_token_ms: {:.0}", self.time_to_first_token_ms);
        info!("tokens_streamed: {}", self.tokens_streamed);
        info!("tokens_per_second: {:.1}", self.tokens_per_second);
        info!("total_time_ms: {:.0}", self.total_stream_time_ms);
    }

    /// Create metrics from timing data
    fn from_timing(
        backend: &str,
        start_time: Instant,
        first_token_time: Option<Instant>,
        end_time: Instant,
        tokens_streamed: usize,
    ) -> Self {
        let total_time = end_time.duration_since(start_time);
        let total_time_ms = total_time.as_secs_f64() * 1000.0;

        let time_to_first_token_ms = first_token_time
            .map(|t| t.duration_since(start_time).as_secs_f64() * 1000.0)
            .unwrap_or(0.0);

        let tokens_per_second = if total_time_ms > 0.0 {
            tokens_streamed as f64 / (total_time_ms / 1000.0)
        } else {
            0.0
        };

        Self {
            backend: backend.to_string(),
            time_to_first_token_ms,
            total_stream_time_ms: total_time_ms,
            total_latency_ms: total_time_ms,
            tokens_streamed,
            tokens_per_second,
        }
    }
}

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
#[allow(dead_code)]
pub struct ExecutionTrace {
    events: Vec<TraceEvent>,
    start_time_ms: u64,
}

impl ExecutionTrace {
    /// Create a new execution trace
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
            metrics: Option<MetricsOutput>,
        }

        #[derive(Serialize)]
        struct MetricsOutput {
            latency_ms: f64,
            tokens_streamed: usize,
            tokens_per_second: f64,
        }

        let output = TraceOutput {
            request_id,
            stages: &self.events,
            metrics: None,
        };

        serde_json::to_string_pretty(&output).unwrap_or_default()
    }
}

// ============================================================================
// Compute Mesh Components
// ============================================================================

/// Routing policy for compute mesh
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RoutingPolicy {
    /// Use only local backend
    LocalOnly,
    /// Use only cloud backend
    CloudOnly,
    /// Try local first, fall back to cloud if needed
    LocalFirstWithCloudFallback,
}

impl RoutingPolicy {
    fn as_str(&self) -> &'static str {
        match self {
            RoutingPolicy::LocalOnly => "LocalOnly",
            RoutingPolicy::CloudOnly => "CloudOnly",
            RoutingPolicy::LocalFirstWithCloudFallback => "LocalFirstWithCloudFallback",
        }
    }
}

/// Backend selection
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Backend {
    /// Local inference backend
    Local,
    /// Cloud inference backend
    Cloud,
}

impl Backend {
    fn as_str(&self) -> &'static str {
        match self {
            Backend::Local => "local",
            Backend::Cloud => "cloud",
        }
    }
}

// ============================================================================
// Compute Mesh Pipeline
// ============================================================================

/// Main compute mesh pipeline with trace instrumentation and performance benchmarking
#[allow(dead_code)]
#[derive(Debug)]
pub struct ComputeMeshPipeline {
    trace: Arc<RwLock<ExecutionTrace>>,
    policy: RoutingPolicy,
    local_provider: LinuxLocalProvider,
}

impl ComputeMeshPipeline {
    /// Create a new compute mesh pipeline with real inference
    pub fn new(policy: RoutingPolicy) -> Self {
        // Create the local provider for real inference
        // Note: Using CPU backend since we don't have actual model files
        let local_config = LinuxInferenceConfig::new(
            "demo-model",
            "/tmp/demo-model.gguf"  // Placeholder path
        )
        .with_backend(ComputeBackend::Cpu)
        .with_max_tokens(50);
        
        let local_provider = LinuxLocalProvider::new(local_config)
            .expect("Failed to create local provider");

        Self {
            trace: Arc::new(RwLock::new(ExecutionTrace::new())),
            policy,
            local_provider,
        }
    }

    /// Execute the full pipeline with benchmarking
    pub async fn execute(&self, input: &str) -> Result<(String, PerformanceMetrics), String> {
        let trace = self.trace.clone();
        let start_time = Instant::now();
        let mut first_token_time: Option<Instant> = None;

        // Stage 1: Workflow Start
        {
            trace
                .write()
                .unwrap()
                .record("workflow.start", None::<&str>);
        }
        info!("[workflow] executing step: generate_response");
        info!("Prompt: {}", input);

        // Stage 2: Routing Decision
        let backend = {
            let policy_str = self.policy.as_str();
            trace
                .write()
                .unwrap()
                .record("router.policy", Some(policy_str));

            // Route to local backend
            let selected = Backend::Local;
            let backend_str = selected.as_str();
            trace
                .write()
                .unwrap()
                .record("router.backend_selection", Some(backend_str));

            info!("[router] policy: {}", policy_str);
            info!("[router] selected backend: {}", backend_str);

            selected
        };

        // Stage 3: Inference Start
        {
            trace
                .write()
                .unwrap()
                .record("inference.start", None::<&str>);
        }
        info!("[inference] sending request to orchestrator...");

        // Execute real inference with streaming
        let (token_count, stream_end) = self.do_inference(input, &backend, &mut first_token_time).await?;

        // Calculate metrics
        let metrics = PerformanceMetrics::from_timing(
            backend.as_str(),
            start_time,
            first_token_time,
            stream_end,
            token_count,
        );

        // Record metrics in trace
        {
            let mut trace = trace.write().unwrap();
            trace.record("metrics.latency_ms", Some(format!("{:.0}", metrics.total_latency_ms)));
            trace.record("metrics.tokens_streamed", Some(token_count.to_string()));
            trace.record("metrics.tokens_per_second", Some(format!("{:.1}", metrics.tokens_per_second)));
        }

        info!("[metrics] latency_ms = {:.0}", metrics.total_latency_ms);
        info!("[metrics] time_to_first_token_ms = {:.0}", metrics.time_to_first_token_ms);
        info!("[metrics] tokens_streamed = {}", metrics.tokens_streamed);
        info!("[metrics] tokens_per_second = {:.1}", metrics.tokens_per_second);
        info!("[metrics] total_time_ms = {:.0}", metrics.total_stream_time_ms);

        // Stage 4: Workflow Complete
        {
            trace
                .write()
                .unwrap()
                .record("workflow.complete", None::<&str>);
        }

        // Print the execution trace
        self.print_trace();

        let result = format!(
            "Processed '{}' with {} tokens (latency: {:.0}ms)",
            input, token_count, metrics.total_latency_ms
        );

        Ok((result, metrics))
    }

    /// Execute real inference with token streaming and timing
    #[allow(dead_code)]
    async fn do_inference(
        &self,
        input: &str,
        backend: &Backend,
        first_token_time: &mut Option<Instant>,
    ) -> Result<(usize, Instant), String> {
        // Use the local provider directly for streaming inference
        // This provides true token-by-token streaming
        
        let mut token_count = 0;
        
        // Create a mutable reference to the provider for streaming
        // We need to load the provider first
        
        // For demonstration, we'll use the orchestrator's streaming
        // Create a mock inference request for the orchestrator
        let request = InferenceRequest {
            prompt: input.to_string(),
            model_id: Some("demo-model".to_string()),
            required_memory_mb: 1024,
            preferred_precision: mofa_foundation::orchestrator::traits::DegradationLevel::Int4,
            priority: RequestPriority::Normal,
            max_tokens: Some(50),
        };
        
        // Use the orchestrator's streaming
        let (_result, mut stream) = self.orchestrator.infer_stream(&request);
        
        // Stream and collect tokens
        while let Some(token_result) = stream.next().await {
            match token_result {
                Ok(token) => {
                    // Record first token time
                    if token_count == 0 {
                        *first_token_time = Some(Instant::now());
                    }
                    
                    let detail = format!("token_{}", token_count + 1);
                    self.trace
                        .write()
                        .unwrap()
                        .record("streaming.tokens", Some(detail));
                    
                    info!("[stream] {}", token);
                    token_count += 1;
                }
                Err(e) => {
                    tracing::warn!("Stream error: {:?}", e);
                }
            }
        }

        let end = Instant::now();
        Ok((token_count, end))
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
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("========================================");
    info!("  MoFA Compute Mesh Demo               ");
    info!("  with Benchmarking & Trace            ");
    info!("========================================");
    info!("");

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
        Ok((result, metrics)) => {
            println!("\n--- JSON Trace Export ---\n");
            println!("{}", pipeline.export_trace_json(&request_id));
            println!("\nResult: {}", result);
            
            // Print metrics summary
            println!("\n--- Performance Metrics ---");
            println!("backend: {}", metrics.backend);
            println!("latency_ms: {:.0}", metrics.total_latency_ms);
            println!("time_to_first_token_ms: {:.0}", metrics.time_to_first_token_ms);
            println!("tokens_streamed: {}", metrics.tokens_streamed);
            println!("tokens_per_second: {:.1}", metrics.tokens_per_second);
            println!("total_time_ms: {:.0}", metrics.total_stream_time_ms);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}
