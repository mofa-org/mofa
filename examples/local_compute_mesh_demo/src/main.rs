//! Local Compute Mesh Demo with Performance Benchmarking
//!
//! This demo demonstrates the compute mesh routing between local and cloud inference backends.
//! It includes comprehensive performance benchmarking to measure latency, throughput, and token metrics.

use std::time::Instant;
use mofa_foundation::inference::{
    InferenceOrchestrator,
    InferenceRequest,
    OrchestratorConfig,
    RequestPriority,
    RoutingPolicy,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Performance metrics collected during inference
#[derive(Debug, Clone)]
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

/// Simulated streaming response for demo purposes
/// In a real implementation, this would stream tokens from the LLM
struct StreamingResponse {
    tokens: Vec<String>,
}

impl StreamingResponse {
    /// Simulate streaming tokens with timing
    fn stream_and_measure(
        &self,
        backend: &str,
        mut on_token: impl FnMut(&str, Option<Instant>),
    ) -> PerformanceMetrics {
        let start_time = Instant::now();
        let mut first_token_time: Option<Instant> = None;
        let mut token_count = 0;

        for token in &self.tokens {
            // Simulate token arrival timing
            let now = Instant::now();
            if first_token_time.is_none() {
                first_token_time = Some(now);
            }
            token_count += 1;
            on_token(token, first_token_time);

            // Small delay to simulate streaming
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let end_time = Instant::now();
        PerformanceMetrics::from_timing(
            backend,
            start_time,
            first_token_time,
            end_time,
            token_count,
        )
    }
}

/// Execute inference with the orchestrator and measure performance
fn execute_inference_with_benchmark(
    orchestrator: &mut InferenceOrchestrator,
    prompt: &str,
    model_id: &str,
) -> (String, PerformanceMetrics) {
    // Start timing
    let start_time = Instant::now();

    // Create inference request
    let request = InferenceRequest::new(model_id, prompt, 4096)
        .with_priority(RequestPriority::Normal);

    info!("[inference] sending request to orchestrator...");

    // Execute inference
    let result = orchestrator.infer(&request);

    // Determine backend type for metrics
    let backend_type = match &result.routed_to {
        mofa_foundation::inference::RoutedBackend::Local { .. } => "local",
        mofa_foundation::inference::RoutedBackend::Cloud { .. } => "cloud",
        mofa_foundation::inference::RoutedBackend::Rejected { .. } => "rejected",
    };

    info!("[router] policy: {:?}", orchestrator.routing_policy());
    info!("[router] selected backend: {}", backend_type);

    // Simulate streaming response for demo
    // In production, this would stream actual tokens from the LLM
    let response_text = format!(
        "This is a simulated response for: {}. In production, \
        this would stream actual tokens from the local or cloud LLM.",
        prompt
    );

    let tokens: Vec<String> = response_text
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let streaming_response = StreamingResponse {
        tokens,
    };

    // Stream tokens and measure
    let metrics = streaming_response.stream_and_measure(backend_type, |token, _first_token| {
        info!("[stream] {}", token);
    });

    let end_time = Instant::now();
    let total_latency = end_time.duration_since(start_time).as_secs_f64() * 1000.0;

    // Combine with actual request latency
    let final_metrics = PerformanceMetrics {
        backend: metrics.backend.clone(),
        time_to_first_token_ms: metrics.time_to_first_token_ms,
        total_stream_time_ms: metrics.total_stream_time_ms,
        total_latency_ms: total_latency,
        tokens_streamed: metrics.tokens_streamed,
        tokens_per_second: metrics.tokens_per_second,
    };

    (result.output, final_metrics)
}

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

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("========================================");
    info!("  MoFA Compute Mesh Demo               ");
    info!("  with Performance Benchmarking         ");
    info!("========================================");
    info!("");

    // Get prompt from command line args or use default
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Explain photosynthesis".to_string());

    info!("[workflow] executing step: generate_response");
    info!("Prompt: {}", prompt);
    info!("");

    // Demo 1: LocalFirstWithCloudFallback policy (default)
    info!("=== Demo 1: LocalFirstWithCloudFallback ===");
    demo_local_first(&prompt).await?;

    info!("");
    info!("=== Demo 2: CloudOnly ===");
    demo_cloud_only(&prompt).await?;

    info!("");
    info!("=== Demo 3: LocalOnly ===");
    demo_local_only(&prompt).await?;

    info!("");
    info!("========================================");
    info!("  Demo completed!                      ");
    info!("========================================");

    Ok(())
}

/// Demo with LocalFirstWithCloudFallback policy
async fn demo_local_first(prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = OrchestratorConfig {
        memory_capacity_mb: 16384,
        defer_threshold: 0.75,
        reject_threshold: 0.90,
        model_pool_capacity: 5,
        routing_policy: RoutingPolicy::LocalFirstWithCloudFallback,
        cloud_provider: "openai".to_string(),
        ..Default::default()
    };

    let mut orchestrator = InferenceOrchestrator::new(config);

    let (_result, metrics) = execute_inference_with_benchmark(
        &mut orchestrator,
        prompt,
        "llama-3-7b",
    );

    metrics.print_metrics();

    Ok(())
}

/// Demo with CloudOnly policy
async fn demo_cloud_only(prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = OrchestratorConfig {
        memory_capacity_mb: 16384,
        defer_threshold: 0.75,
        reject_threshold: 0.90,
        model_pool_capacity: 5,
        routing_policy: RoutingPolicy::CloudOnly,
        cloud_provider: "openai".to_string(),
        ..Default::default()
    };

    let mut orchestrator = InferenceOrchestrator::new(config);

    let (_result, metrics) = execute_inference_with_benchmark(
        &mut orchestrator,
        prompt,
        "llama-3-7b",
    );

    metrics.print_metrics();

    Ok(())
}

/// Demo with LocalOnly policy
async fn demo_local_only(prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = OrchestratorConfig {
        memory_capacity_mb: 16384,
        defer_threshold: 0.75,
        reject_threshold: 0.90,
        model_pool_capacity: 5,
        routing_policy: RoutingPolicy::LocalOnly,
        cloud_provider: "openai".to_string(),
        ..Default::default()
    };

    let mut orchestrator = InferenceOrchestrator::new(config);

    let (_result, metrics) = execute_inference_with_benchmark(
        &mut orchestrator,
        prompt,
        "llama-3-7b",
    );

    metrics.print_metrics();

    Ok(())
}
