//! Unified Inference Orchestrator.
//!
//! This is the central control plane for inference in MoFA. It composes
//! the routing policy, model pool, and memory scheduler into a single
//! entry point that agents use to request inference without knowing
//! whether execution happens locally or in the cloud.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │           InferenceOrchestrator              │
//! │                                             │
//! │  RoutingPolicy  →  MemoryBudget             │
//! │        ↓                ↓                   │
//! │     ModelPool    →  AdmissionCheck           │
//! │        ↓                ↓                   │
//! │     LocalExec  ←→  CloudFallback            │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Phase 1 Scope
//!
//! Phase 1 focuses on deterministic routing and lifecycle control.
//! Precision adaptation (f16→q8→q4 downgrade) and deferred-queue
//! scheduling will be introduced in Phase 2.

use std::time::Duration;

use crate::hardware::{HardwareCapability, detect_hardware};

use super::model_pool::ModelPool;
use super::routing::{self, AdmissionOutcome, RoutingDecision, RoutingPolicy};
use super::types::{InferenceRequest, InferenceResult, RoutedBackend};

/// Configuration for the `InferenceOrchestrator`.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Total memory budget for local models (in MB)
    pub memory_capacity_mb: usize,
    /// Fraction of capacity above which new requests are deferred (0.0–1.0)
    pub defer_threshold: f64,
    /// Fraction of capacity above which new requests are rejected (0.0–1.0)
    pub reject_threshold: f64,
    /// Maximum number of models that can be concurrently loaded
    pub model_pool_capacity: usize,
    /// Models idle longer than this duration are candidates for eviction
    pub idle_timeout: Duration,
    /// The routing policy governing local vs cloud decisions
    pub routing_policy: RoutingPolicy,
    /// The cloud provider to use for fallback (e.g., "openai")
    pub cloud_provider: String,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            memory_capacity_mb: 16384, // 16 GB
            defer_threshold: 0.75,
            reject_threshold: 0.90,
            model_pool_capacity: 5,
            idle_timeout: Duration::from_secs(300),
            routing_policy: RoutingPolicy::default(),
            cloud_provider: "openai".to_string(),
        }
    }
}

/// The unified inference orchestrator.
///
/// Provides a single entry point (`infer`) for agents to request inference.
/// Internally handles:
/// - Memory-aware admission control
/// - Policy-driven routing (local vs cloud)
/// - LRU model lifecycle management
/// - Automatic cloud failover
///
/// Memory tracking is derived from `ModelPool` as the single source of truth.
/// There is no separate `allocated_mb` counter — this prevents inconsistency.
pub struct InferenceOrchestrator {
    config: OrchestratorConfig,
    model_pool: ModelPool,
    hardware: HardwareCapability,
}

impl InferenceOrchestrator {
    /// Create a new orchestrator with the given configuration.
    ///
    /// Hardware capabilities are auto-detected at construction time.
    /// The memory capacity automatically defaults to the host machine's total unified/VRAM memory.
    pub fn new(mut config: OrchestratorConfig) -> Self {
        let hardware = detect_hardware();

        // Dynamically override memory capacity with actual unified memory (MB)
        if config.memory_capacity_mb == 16384 {
            config.memory_capacity_mb = (hardware.total_memory_bytes / 1_000_000) as usize;
        }

        let model_pool = ModelPool::new(config.model_pool_capacity, config.idle_timeout);

        Self {
            config,
            model_pool,
            hardware,
        }
    }

    /// Create an orchestrator with explicit hardware capabilities (for testing).
    pub fn with_hardware(config: OrchestratorConfig, hardware: HardwareCapability) -> Self {
        let model_pool = ModelPool::new(config.model_pool_capacity, config.idle_timeout);

        Self {
            config,
            model_pool,
            hardware,
        }
    }

    /// The single entry point for inference.
    ///
    /// Agents call this method with an `InferenceRequest`. The orchestrator
    /// evaluates admission, routes to the appropriate backend, manages model
    /// lifecycle, and returns the result.
    ///
    /// In this Phase 1 implementation, actual model execution is simulated.
    /// Real backend integration (MLX, OpenAI) will be wired in Phase 2.
    pub fn infer(&mut self, request: &InferenceRequest) -> InferenceResult {
        // Step 1: Evict idle models to free memory before admission check
        self.model_pool.evict_idle();

        // Step 2: Evaluate admission based on current memory state
        // Memory is always derived from ModelPool (single source of truth)
        let admission = self.evaluate_admission(request);

        // Step 3: Resolve routing based on policy + admission + hardware
        let decision = routing::resolve(
            &self.config.routing_policy,
            request,
            admission,
            &self.hardware,
            &self.config.cloud_provider,
        );

        // Step 4: Execute based on routing decision
        match &decision {
            RoutingDecision::UseLocal { model_id } => {
                // Load the model if not already loaded
                if !self.model_pool.is_loaded(model_id) {
                    self.model_pool.load(
                        model_id,
                        request.required_memory_mb,
                        request.preferred_precision,
                    );
                } else {
                    self.model_pool.touch(model_id);
                }

                InferenceResult {
                    output: format!(
                        "[local:{}] Inference result for: {}",
                        model_id, request.prompt
                    ),
                    routed_to: RoutedBackend::Local {
                        model_id: model_id.clone(),
                    },
                    actual_precision: request.preferred_precision,
                }
            }
            RoutingDecision::UseCloud { provider } => InferenceResult {
                output: format!(
                    "[cloud:{}] Inference result for: {}",
                    provider, request.prompt
                ),
                routed_to: RoutedBackend::Cloud {
                    provider: provider.clone(),
                },
                actual_precision: request.preferred_precision,
            },
            RoutingDecision::Rejected { reason } => InferenceResult {
                output: format!("[rejected] {}", reason),
                routed_to: RoutedBackend::Rejected {
                    reason: reason.clone(),
                },
                actual_precision: request.preferred_precision,
            },
        }
    }

    /// Evaluate whether a local backend can admit this request based on
    /// current memory usage and configured thresholds.
    ///
    /// Memory is always read from ModelPool — no separate counter to
    /// get out of sync.
    fn evaluate_admission(&self, request: &InferenceRequest) -> AdmissionOutcome {
        let current_mb = self.model_pool.total_memory_mb();
        let projected_mb = current_mb + request.required_memory_mb;
        let capacity = self.config.memory_capacity_mb;

        if capacity == 0 {
            return AdmissionOutcome::Rejected;
        }

        let projected_usage = projected_mb as f64 / capacity as f64;

        if projected_usage <= self.config.defer_threshold {
            AdmissionOutcome::Accepted
        } else if projected_usage <= self.config.reject_threshold {
            // Phase 1: Deferred is treated as a routing signal.
            // Phase 2 will add a queue-based scheduler with retry logic.
            AdmissionOutcome::Deferred
        } else {
            AdmissionOutcome::Rejected
        }
    }

    /// Get the current memory usage as a fraction of total capacity (0.0–1.0).
    pub fn memory_utilization(&self) -> f64 {
        if self.config.memory_capacity_mb == 0 {
            return 1.0;
        }
        self.model_pool.total_memory_mb() as f64 / self.config.memory_capacity_mb as f64
    }

    /// Get the number of currently loaded models.
    pub fn loaded_model_count(&self) -> usize {
        self.model_pool.len()
    }

    /// Get the total allocated memory (in MB), derived from ModelPool.
    pub fn allocated_memory_mb(&self) -> usize {
        self.model_pool.total_memory_mb()
    }

    /// Get a reference to the detected hardware capabilities.
    pub fn hardware(&self) -> &HardwareCapability {
        &self.hardware
    }

    /// Get a reference to the active routing policy.
    pub fn routing_policy(&self) -> &RoutingPolicy {
        &self.config.routing_policy
    }

    /// Manually unload a model from the pool, freeing its memory.
    pub fn unload_model(&mut self, model_id: &str) -> usize {
        self.model_pool.unload(model_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{CpuFamily, GpuType, HardwareCapability, OsClassification};

    fn test_hardware() -> HardwareCapability {
        HardwareCapability {
            os: OsClassification::MacOS,
            cpu_family: CpuFamily::AppleSilicon,
            gpu_available: true,
            gpu_type: Some(GpuType::Metal),
            total_memory_bytes: 32_000_000_000,
            available_memory_bytes: 16_000_000_000,
        }
    }

    fn test_config() -> OrchestratorConfig {
        OrchestratorConfig {
            memory_capacity_mb: 24576, // 24 GB
            defer_threshold: 0.75,
            reject_threshold: 0.90,
            model_pool_capacity: 5,
            idle_timeout: Duration::from_secs(300),
            routing_policy: RoutingPolicy::LocalFirstWithCloudFallback,
            cloud_provider: "openai".to_string(),
        }
    }

    #[test]
    fn test_local_inference_happy_path() {
        let mut orch = InferenceOrchestrator::with_hardware(test_config(), test_hardware());

        let request = InferenceRequest::new("llama-3-7b", "Hello world", 7168);
        let result = orch.infer(&request);

        assert_eq!(
            result.routed_to,
            RoutedBackend::Local {
                model_id: "llama-3-7b".into()
            }
        );
        assert!(result.output.contains("local:llama-3-7b"));
        assert_eq!(orch.loaded_model_count(), 1);
        assert_eq!(orch.allocated_memory_mb(), 7168);
    }

    #[test]
    fn test_cloud_fallback_when_memory_full() {
        let mut config = test_config();
        config.memory_capacity_mb = 24576; // 24 GB
        let mut orch = InferenceOrchestrator::with_hardware(config, test_hardware());

        // Load a model that uses ~54% of capacity (under 75% defer threshold)
        // 13312 / 24576 = 54.2% → Accepted
        let req1 = InferenceRequest::new("llama-3-13b", "First", 13312);
        let result1 = orch.infer(&req1);
        assert_eq!(
            result1.routed_to,
            RoutedBackend::Local {
                model_id: "llama-3-13b".into()
            }
        );

        // Second model: (13312 + 10240) / 24576 = 95.8% → exceeds 90% reject → cloud
        let req2 = InferenceRequest::new("mistral-7b", "Second", 10240);
        let result2 = orch.infer(&req2);
        assert_eq!(
            result2.routed_to,
            RoutedBackend::Cloud {
                provider: "openai".into()
            }
        );
    }

    #[test]
    fn test_local_only_rejects_when_full() {
        let mut config = test_config();
        config.memory_capacity_mb = 16384; // 16 GB
        config.routing_policy = RoutingPolicy::LocalOnly;
        let mut orch = InferenceOrchestrator::with_hardware(config, test_hardware());

        // Fill memory: 10000 / 16384 = 61% → Accepted
        let req1 = InferenceRequest::new("model-a", "test", 10000);
        orch.infer(&req1);

        // Second: (10000 + 6000) / 16384 = 97.6% → Rejected
        // With LocalOnly policy, rejection means no cloud fallback
        let req2 = InferenceRequest::new("model-b", "test", 6000);
        let result = orch.infer(&req2);
        assert!(matches!(result.routed_to, RoutedBackend::Rejected { .. }));
        assert!(result.output.contains("rejected"));
    }

    #[test]
    fn test_memory_utilization_tracking() {
        let mut config = test_config();
        config.memory_capacity_mb = 10000;
        let mut orch = InferenceOrchestrator::with_hardware(config, test_hardware());

        assert_eq!(orch.memory_utilization(), 0.0);

        let req = InferenceRequest::new("model-a", "test", 5000);
        orch.infer(&req);

        assert!((orch.memory_utilization() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_model_unloading_frees_memory() {
        let mut orch = InferenceOrchestrator::with_hardware(test_config(), test_hardware());

        let req = InferenceRequest::new("model-a", "test", 8000);
        orch.infer(&req);
        assert_eq!(orch.allocated_memory_mb(), 8000);

        let freed = orch.unload_model("model-a");
        assert_eq!(freed, 8000);
        assert_eq!(orch.allocated_memory_mb(), 0);
        assert_eq!(orch.loaded_model_count(), 0);
    }

    #[test]
    fn test_cloud_only_always_routes_to_cloud() {
        let mut config = test_config();
        config.routing_policy = RoutingPolicy::CloudOnly;
        let mut orch = InferenceOrchestrator::with_hardware(config, test_hardware());

        let req = InferenceRequest::new("llama-3-7b", "Hello", 7168);
        let result = orch.infer(&req);

        assert_eq!(
            result.routed_to,
            RoutedBackend::Cloud {
                provider: "openai".into()
            }
        );
        // Model should NOT be loaded locally
        assert_eq!(orch.loaded_model_count(), 0);
    }
}
