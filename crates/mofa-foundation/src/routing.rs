//! Smart Routing Policy Engine for MoFA
//!
//! This module provides intelligent request routing based on:
//! - Policy-based routing (local-first, latency-first, cost-first)
//! - Task-type routing (ASR/LLM/TTS/Embedding model selection)
//! - Memory-aware admission control
//! - Provider-aware retry/failover
//! - Dynamic precision degradation under resource pressure
//!
//! # Example
//!
//! ```rust
//! use mofa_foundation::routing::{SmartRouter, RoutingPolicy, TaskType};
//!
//! #[tokio::main]
//! async fn main() {
//!     let router = SmartRouter::new(RoutingPolicy::LocalFirst);
//!     // ... use the router
//! }
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Type of inference task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskType {
    /// Automatic Speech Recognition
    Asr,
    /// Large Language Model
    Llm,
    /// Text-to-Speech
    Tts,
    /// Text Embedding
    Embedding,
    /// Vision Language Model
    Vlm,
    /// Unknown/Generic
    Unknown,
}

impl TaskType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "asr" | "speech" | "transcription" => TaskType::Asr,
            "llm" | "language" | "chat" | "completion" => TaskType::Llm,
            "tts" | "speech synthesis" | "audio" => TaskType::Tts,
            "embedding" | "vec" | "vector" => TaskType::Embedding,
            "vlm" | "vision" | "multimodal" => TaskType::Vlm,
            _ => TaskType::Unknown,
        }
    }
}

/// Routing policy for selecting backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingPolicy {
    /// Prefer local backends first, then fall back to cloud
    LocalFirst,
    /// Prefer the fastest backend regardless of location
    LatencyFirst,
    /// Prefer the cheapest backend
    CostFirst,
    /// Quality-first, prefer best models regardless of cost/latency
    QualityFirst,
    /// Hybrid: balance local and cloud based on task type
    Hybrid,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        RoutingPolicy::LocalFirst
    }
}

/// Backend type (local or cloud)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    /// Local model (CPU/GPU)
    Local,
    /// Cloud API (OpenAI, Anthropic, etc.)
    Cloud,
}

/// Supported precision levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precision {
    /// Full precision (FP32)
    Full,
    /// Half precision (FP16)
    Half,
    /// Quantized (INT8)
    Quantized,
    /// Highly quantized (INT4)
    HighlyQuantized,
}

impl Precision {
    /// Get memory multiplier relative to FP32
    pub fn memory_multiplier(&self) -> f32 {
        match self {
            Precision::Full => 1.0,
            Precision::Half => 0.5,
            Precision::Quantized => 0.25,
            Precision::HighlyQuantized => 0.125,
        }
    }
}

/// Information about a provider backend
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub backend_type: BackendType,
    pub supported_tasks: Vec<TaskType>,
    pub latency_ms: u64,
    pub cost_per_1k_tokens: f32,
    pub available: bool,
    pub max_memory_bytes: usize,
    pub current_memory_usage_bytes: usize,
    pub supported_precisions: Vec<Precision>,
}

/// Request for routing
#[derive(Debug, Clone)]
pub struct RoutingRequest {
    pub task_type: TaskType,
    pub preferred_policy: Option<RoutingPolicy>,
    pub required_precision: Option<Precision>,
    pub max_latency_ms: Option<u64>,
    pub max_cost_per_1k: Option<f32>,
    pub memory_requirement_bytes: Option<usize>,
    pub context_length: Option<usize>,
}

/// Result of routing decision
#[derive(Debug, Clone)]
pub struct RoutingResult {
    pub provider_id: String,
    pub provider_name: String,
    pub backend_type: BackendType,
    pub recommended_precision: Precision,
    pub estimated_latency_ms: u64,
    pub estimated_cost_per_1k: f32,
    pub reason: String,
}

/// Admission decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionDecision {
    /// Request admitted
    Admitted,
    /// Request needs to wait for resources
    Deferred,
    /// Request rejected due to constraints
    Rejected(String),
}

/// Trait for inference providers
#[async_trait]
pub trait InferenceProvider: Send + Sync {
    /// Get provider info
    fn info(&self) -> &ProviderInfo;

    /// Check if provider supports a task type
    fn supports_task(&self, task: TaskType) -> bool;

    /// Execute inference (placeholder)
    async fn execute(&self, input: &str) -> Result<String, String>;

    /// Get current memory usage
    fn current_memory_usage(&self) -> usize;

    /// Get available memory
    fn available_memory(&self) -> usize;
}

/// Smart Router for inference requests
pub struct SmartRouter {
    config: RouterConfig,
    providers: RwLock<Vec<Arc<dyn InferenceProvider>>>,
    stats: RouterStats,
}

/// Configuration for the smart router
#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub default_policy: RoutingPolicy,
    pub enable_memory_pressure_routing: bool,
    pub memory_threshold_percent: u8,
    pub enable_retry: bool,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub enable_precision_degradation: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            default_policy: RoutingPolicy::LocalFirst,
            enable_memory_pressure_routing: true,
            memory_threshold_percent: 80,
            enable_retry: true,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_precision_degradation: true,
        }
    }
}

/// Router statistics
#[derive(Debug, Default)]
pub struct RouterStats {
    pub total_requests: AtomicU64,
    pub local_requests: AtomicU64,
    pub cloud_requests: AtomicU64,
    pub rejected_requests: AtomicU64,
    pub deferred_requests: AtomicU64,
    pub failed_requests: AtomicU64,
}

impl Clone for RouterStats {
    fn clone(&self) -> Self {
        Self {
            total_requests: AtomicU64::new(self.total_requests.load(Ordering::SeqCst)),
            local_requests: AtomicU64::new(self.local_requests.load(Ordering::SeqCst)),
            cloud_requests: AtomicU64::new(self.cloud_requests.load(Ordering::SeqCst)),
            rejected_requests: AtomicU64::new(self.rejected_requests.load(Ordering::SeqCst)),
            deferred_requests: AtomicU64::new(self.deferred_requests.load(Ordering::SeqCst)),
            failed_requests: AtomicU64::new(self.failed_requests.load(Ordering::SeqCst)),
        }
    }
}

impl SmartRouter {
    /// Create a new SmartRouter with the given policy
    pub fn new(policy: RoutingPolicy) -> Self {
        Self {
            config: RouterConfig {
                default_policy: policy,
                ..Default::default()
            },
            providers: RwLock::new(Vec::new()),
            stats: RouterStats::default(),
        }
    }

    /// Create a new SmartRouter with custom config
    pub fn with_config(config: RouterConfig) -> Self {
        Self {
            config,
            providers: RwLock::new(Vec::new()),
            stats: RouterStats::default(),
        }
    }

    /// Register a provider
    pub async fn register_provider(&self, provider: Arc<dyn InferenceProvider>) {
        let mut providers = self.providers.write().await;
        info!("Registering provider: {}", provider.info().name);
        providers.push(provider);
    }

    /// Unregister a provider
    pub async fn unregister_provider(&self, provider_id: &str) {
        let mut providers = self.providers.write().await;
        providers.retain(|p| p.info().id != provider_id);
        info!("Unregistered provider: {}", provider_id);
    }

    /// Route a request to the best provider
    pub async fn route(&self, request: RoutingRequest) -> Result<RoutingResult, String> {
        self.stats.total_requests.fetch_add(1, Ordering::SeqCst);

        let policy = request.preferred_policy.unwrap_or(self.config.default_policy);
        let providers = self.providers.read().await;

        // Filter providers that support the task
        let eligible: Vec<&Arc<dyn InferenceProvider>> = providers
            .iter()
            .filter(|p| p.supports_task(request.task_type) && p.info().available)
            .collect();

        if eligible.is_empty() {
            self.stats.rejected_requests.fetch_add(1, Ordering::SeqCst);
            return Err(format!("No provider available for task type: {:?}", request.task_type));
        }

        // Check admission control
        let admission = self.check_admission(&eligible, &request).await;
        match admission {
            AdmissionDecision::Rejected(reason) => {
                self.stats.rejected_requests.fetch_add(1, Ordering::SeqCst);
                return Err(format!("Request rejected: {}", reason));
            }
            AdmissionDecision::Deferred => {
                self.stats.deferred_requests.fetch_add(1, Ordering::SeqCst);
                return Err("Request deferred - insufficient resources".to_string());
            }
            AdmissionDecision::Admitted => {}
        }

        // Select provider based on policy
        let result = self.select_provider(&eligible, &request, policy).await?;

        // Update stats
        match result.backend_type {
            BackendType::Local => {
                self.stats.local_requests.fetch_add(1, Ordering::SeqCst);
            }
            BackendType::Cloud => {
                self.stats.cloud_requests.fetch_add(1, Ordering::SeqCst);
            }
        }

        Ok(result)
    }

    /// Check admission control
    async fn check_admission(
        &self,
        providers: &[&Arc<dyn InferenceProvider>],
        request: &RoutingRequest,
    ) -> AdmissionDecision {
        if !self.config.enable_memory_pressure_routing {
            return AdmissionDecision::Admitted;
        }

        let memory_req = request.memory_requirement_bytes.unwrap_or(0);
        if memory_req == 0 {
            return AdmissionDecision::Admitted;
        }

        for provider in providers {
            let info = provider.info();
            let available = info.max_memory_bytes.saturating_sub(info.current_memory_usage_bytes);
            
            if available >= memory_req {
                return AdmissionDecision::Admitted;
            }
        }

        // All providers are under memory pressure
        if providers.iter().all(|p| {
            let info = p.info();
            let usage_percent = (info.current_memory_usage_bytes * 100) / info.max_memory_bytes.max(1);
            usage_percent >= self.config.memory_threshold_percent as usize
        }) {
            return AdmissionDecision::Deferred;
        }

        AdmissionDecision::Admitted
    }

    /// Select the best provider based on policy
    async fn select_provider(
        &self,
        providers: &[&Arc<dyn InferenceProvider>],
        request: &RoutingRequest,
        policy: RoutingPolicy,
    ) -> Result<RoutingResult, String> {
        match policy {
            RoutingPolicy::LocalFirst => self.select_local_first(providers, request).await,
            RoutingPolicy::LatencyFirst => self.select_latency_first(providers, request).await,
            RoutingPolicy::CostFirst => self.select_cost_first(providers, request).await,
            RoutingPolicy::QualityFirst => self.select_quality_first(providers, request).await,
            RoutingPolicy::Hybrid => self.select_hybrid(providers, request).await,
        }
    }

    async fn select_local_first(
        &self,
        providers: &[&Arc<dyn InferenceProvider>],
        request: &RoutingRequest,
    ) -> Result<RoutingResult, String> {
        // Prefer local providers
        let mut local: Vec<_> = providers
            .iter()
            .filter(|p| p.info().backend_type == BackendType::Local)
            .collect();
        
        let mut cloud: Vec<_> = providers
            .iter()
            .filter(|p| p.info().backend_type == BackendType::Cloud)
            .collect();

        // Try local first
        if !local.is_empty() {
            local.sort_by_key(|p| p.info().latency_ms);
            return self.build_result(local[0], request, "local-first");
        }

        // Fall back to cloud
        if !cloud.is_empty() {
            cloud.sort_by_key(|p| p.info().latency_ms);
            return self.build_result(cloud[0], request, "local-fallback-to-cloud");
        }

        Err("No eligible providers".to_string())
    }

    async fn select_latency_first(
        &self,
        providers: &[&Arc<dyn InferenceProvider>],
        request: &RoutingRequest,
    ) -> Result<RoutingResult, String> {
        let mut sorted = providers.to_vec();
        sorted.sort_by_key(|p| p.info().latency_ms);

        // Filter by max latency if specified
        if let Some(max_latency) = request.max_latency_ms {
            sorted.retain(|p| p.info().latency_ms <= max_latency);
        }

        if sorted.is_empty() {
            return Err("No provider meets latency requirements".to_string());
        }

        self.build_result(sorted[0], request, "latency-first")
    }

    async fn select_cost_first(
        &self,
        providers: &[&Arc<dyn InferenceProvider>],
        request: &RoutingRequest,
    ) -> Result<RoutingResult, String> {
        let mut sorted = providers.to_vec();
        sorted.sort_by(|a, b| {
            a.info()
                .cost_per_1k_tokens
                .partial_cmp(&b.info().cost_per_1k_tokens)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Filter by max cost if specified
        if let Some(max_cost) = request.max_cost_per_1k {
            sorted.retain(|p| p.info().cost_per_1k_tokens <= max_cost);
        }

        if sorted.is_empty() {
            return Err("No provider meets cost requirements".to_string());
        }

        self.build_result(sorted[0], request, "cost-first")
    }

    async fn select_quality_first(
        &self,
        providers: &[&Arc<dyn InferenceProvider>],
        request: &RoutingRequest,
    ) -> Result<RoutingResult, String> {
        // For quality-first, prefer cloud providers with highest precision
        let mut sorted = providers.to_vec();
        sorted.sort_by(|a, b| {
            let a_precision = a.info().supported_precisions.len();
            let b_precision = b.info().supported_precisions.len();
            b_precision.cmp(&a_precision)
        });

        self.build_result(sorted[0], request, "quality-first")
    }

    async fn select_hybrid(
        &self,
        providers: &[&Arc<dyn InferenceProvider>],
        request: &RoutingRequest,
    ) -> Result<RoutingResult, String> {
        // Hybrid: use task-type specific routing
        match request.task_type {
            TaskType::Asr | TaskType::Tts => {
                // For audio tasks, prefer local for low latency
                self.select_local_first(providers, request).await
            }
            TaskType::Llm => {
                // For LLM, balance cost and latency
                if request.context_length.unwrap_or(0) > 4096 {
                    // Long context - prefer cloud
                    let mut cloud: Vec<_> = providers
                        .iter()
                        .filter(|p| p.info().backend_type == BackendType::Cloud)
                        .collect();
                    cloud.sort_by_key(|p| p.info().latency_ms);
                    if !cloud.is_empty() {
                        return self.build_result(cloud[0], request, "hybrid-long-context");
                    }
                }
                self.select_local_first(providers, request).await
            }
            TaskType::Embedding => {
                // For embeddings, prefer local (fast and cheap)
                self.select_local_first(providers, request).await
            }
            _ => self.select_local_first(providers, request).await,
        }
    }

    fn build_result(
        &self,
        provider: &Arc<dyn InferenceProvider>,
        request: &RoutingRequest,
        reason: &str,
    ) -> Result<RoutingResult, String> {
        let info = provider.info();
        
        // Determine recommended precision
        let precision = if self.config.enable_precision_degradation {
            self.determine_precision(info, request)
        } else {
            request.required_precision.unwrap_or(Precision::Half)
        };

        Ok(RoutingResult {
            provider_id: info.id.clone(),
            provider_name: info.name.clone(),
            backend_type: info.backend_type,
            recommended_precision: precision,
            estimated_latency_ms: info.latency_ms,
            estimated_cost_per_1k: info.cost_per_1k_tokens,
            reason: reason.to_string(),
        })
    }

    fn determine_precision(&self, info: &ProviderInfo, request: &RoutingRequest) -> Precision {
        // If user specified precision, use that
        if let Some(p) = request.required_precision {
            return p;
        }

        // Check memory pressure and degrade precision if needed
        let memory_percent = if info.max_memory_bytes > 0 {
            (info.current_memory_usage_bytes * 100) / info.max_memory_bytes
        } else {
            0
        };

        if memory_percent > 90 && info.supported_precisions.contains(&Precision::HighlyQuantized) {
            return Precision::HighlyQuantized;
        }
        if memory_percent > 75 && info.supported_precisions.contains(&Precision::Quantized) {
            return Precision::Quantized;
        }
        if memory_percent > 50 && info.supported_precisions.contains(&Precision::Half) {
            return Precision::Half;
        }

        Precision::Full
    }

    /// Execute a request with retry logic
    pub async fn execute_with_retry(
        &self,
        request: &RoutingRequest,
        input: &str,
    ) -> Result<String, String> {
        let mut last_error = String::new();
        let mut attempts = 0;

        while attempts < self.config.max_retries {
            attempts += 1;
            
            match self.route(request.clone()).await {
                Ok(result) => {
                    let providers = self.providers.read().await;
                    if let Some(provider) = providers.iter().find(|p| p.info().id == result.provider_id) {
                        match provider.execute(input).await {
                            Ok(response) => return Ok(response),
                            Err(e) => {
                                last_error = e;
                                debug!("Provider {} failed, attempt {}/{}", 
                                    result.provider_id, attempts, self.config.max_retries);
                            }
                        }
                    }
                }
                Err(e) => {
                    last_error = e;
                }
            }

            if attempts < self.config.max_retries {
                tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
            }
        }

        self.stats.failed_requests.fetch_add(1, Ordering::SeqCst);
        Err(format!("All retries failed: {}", last_error))
    }

    /// Get router statistics
    pub fn stats(&self) -> RouterStatsSnapshot {
        RouterStatsSnapshot {
            total_requests: self.stats.total_requests.load(Ordering::SeqCst),
            local_requests: self.stats.local_requests.load(Ordering::SeqCst),
            cloud_requests: self.stats.cloud_requests.load(Ordering::SeqCst),
            rejected_requests: self.stats.rejected_requests.load(Ordering::SeqCst),
            deferred_requests: self.stats.deferred_requests.load(Ordering::SeqCst),
            failed_requests: self.stats.failed_requests.load(Ordering::SeqCst),
        }
    }

    /// List available providers
    pub async fn list_providers(&self) -> Vec<ProviderInfo> {
        let providers = self.providers.read().await;
        providers.iter().map(|p| p.info().clone()).collect()
    }

    /// Update provider availability
    pub async fn update_provider_availability(&self, provider_id: &str, available: bool) {
        // This would typically update the provider's internal state
        // For now, we just log it
        debug!("Provider {} availability changed to {}", provider_id, available);
    }
}

/// Snapshot of router statistics
#[derive(Debug, Clone)]
pub struct RouterStatsSnapshot {
    pub total_requests: u64,
    pub local_requests: u64,
    pub cloud_requests: u64,
    pub rejected_requests: u64,
    pub deferred_requests: u64,
    pub failed_requests: u64,
}

/// Mock provider for testing
pub struct MockProvider {
    info: ProviderInfo,
}

impl MockProvider {
    pub fn new(
        id: &str,
        name: &str,
        backend_type: BackendType,
        latency_ms: u64,
        cost_per_1k: f32,
    ) -> Self {
        Self {
            info: ProviderInfo {
                id: id.to_string(),
                name: name.to_string(),
                backend_type,
                supported_tasks: vec![TaskType::Llm, TaskType::Embedding],
                latency_ms,
                cost_per_1k_tokens: cost_per_1k,
                available: true,
                max_memory_bytes: 8 * 1024 * 1024 * 1024, // 8GB
                current_memory_usage_bytes: 2 * 1024 * 1024 * 1024, // 2GB
                supported_precisions: vec![Precision::Full, Precision::Half, Precision::Quantized],
            },
        }
    }
}

#[async_trait]
impl InferenceProvider for MockProvider {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn supports_task(&self, task: TaskType) -> bool {
        self.info.supported_tasks.contains(&task)
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        Ok(format!("Mock response for: {}", input))
    }

    fn current_memory_usage(&self) -> usize {
        self.info.current_memory_usage_bytes
    }

    fn available_memory(&self) -> usize {
        self.info.max_memory_bytes - self.info.current_memory_usage_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_smart_router_local_first() {
        let router = SmartRouter::new(RoutingPolicy::LocalFirst);
        
        // Register providers
        let cloud_provider = Arc::new(MockProvider::new(
            "cloud-1", "Cloud Provider", BackendType::Cloud, 100, 0.01,
        ));
        let local_provider = Arc::new(MockProvider::new(
            "local-1", "Local Provider", BackendType::Local, 50, 0.0,
        ));
        
        router.register_provider(cloud_provider).await;
        router.register_provider(local_provider).await;

        // Route request
        let request = RoutingRequest {
            task_type: TaskType::Llm,
            preferred_policy: None,
            required_precision: None,
            max_latency_ms: None,
            max_cost_per_1k: None,
            memory_requirement_bytes: None,
            context_length: None,
        };

        let result = router.route(request).await.unwrap();
        assert_eq!(result.backend_type, BackendType::Local);
        assert_eq!(result.reason, "local-first");
    }

    #[tokio::test]
    async fn test_smart_router_cost_first() {
        let router = SmartRouter::new(RoutingPolicy::CostFirst);
        
        let cheap_provider = Arc::new(MockProvider::new(
            "cheap", "Cheap Provider", BackendType::Local, 100, 0.0,
        ));
        let expensive_provider = Arc::new(MockProvider::new(
            "expensive", "Expensive Provider", BackendType::Cloud, 50, 0.05,
        ));
        
        router.register_provider(cheap_provider).await;
        router.register_provider(expensive_provider).await;

        let request = RoutingRequest {
            task_type: TaskType::Llm,
            preferred_policy: Some(RoutingPolicy::CostFirst),
            ..Default::default()
        };

        let result = router.route(request).await.unwrap();
        assert_eq!(result.provider_id, "cheap");
    }

    #[tokio::test]
    async fn test_smart_router_stats() {
        let router = SmartRouter::new(RoutingPolicy::LocalFirst);
        
        let provider = Arc::new(MockProvider::new(
            "local-1", "Local Provider", BackendType::Local, 50, 0.0,
        ));
        router.register_provider(provider).await;

        let request = RoutingRequest {
            task_type: TaskType::Llm,
            ..Default::default()
        };

        let _ = router.route(request).await;

        let stats = router.stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.local_requests, 1);
    }
}
