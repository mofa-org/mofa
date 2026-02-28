//! Inference routing policy engine.
//!
//! Determines where an inference request should execute (local vs cloud)
//! based on the configured policy, memory scheduler outcome, and
//! hardware capabilities.

use crate::hardware::{CpuFamily, HardwareCapability, OsClassification};

use super::types::{InferenceRequest, Precision, RequestPriority, RoutedBackend};

/// Policy governing how inference requests are routed between
/// local backends and cloud providers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RoutingPolicy {
    /// Only use local backends; reject if local is unavailable.
    LocalOnly,
    /// Only use cloud providers; never attempt local execution.
    CloudOnly,
    /// Try local first; fall back to cloud if local admission fails.
    /// This is the recommended default for most deployments.
    #[default]
    LocalFirstWithCloudFallback,
    /// Route to whichever backend is expected to respond fastest.
    LatencyOptimized,
    /// Route to the cheapest option (local is free, cloud costs money).
    CostOptimized,
    /// Try local at progressively lower quantization (Q8 → Q4) before
    /// falling back to cloud. This is the recommended policy for
    /// memory-constrained edge devices (e.g., Apple Silicon laptops).
    ///
    /// The ladder walks `Precision::next_lower()` until either a level
    /// fits within available memory or all local options are exhausted.
    DegradationLadder,
}

/// The outcome of a routing decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Run inference on a local backend.
    UseLocal { model_id: String },
    /// Run inference on a local backend at a **degraded** precision level.
    ///
    /// The orchestrator should reload or re-quantize the model at
    /// `degraded_precision` before executing the request.
    UseLocalDegraded {
        model_id: String,
        /// The target precision to degrade to.
        degraded_precision: Precision,
        /// Human-readable warning about quality impact.
        quality_warning: String,
    },
    /// Run inference on a cloud provider.
    UseCloud { provider: String },
    /// Request cannot be served by any backend under current policy.
    Rejected { reason: String },
}

/// Represents the memory scheduler's admission outcome when evaluating
/// whether a local backend can handle a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    /// Request accepted — sufficient memory for local execution.
    Accepted,
    /// Request deferred — memory is tight, but may be reclaimable.
    Deferred,
    /// Request rejected — insufficient memory for local execution.
    Rejected,
}

/// Resolve a routing decision based on the configured policy,
/// the memory scheduler's admission outcome, and the host hardware.
///
/// # Arguments
/// * `policy` — The active routing policy.
/// * `request` — The inference request to route.
/// * `admission` — The memory scheduler's admission decision for local execution.
/// * `hardware` — The detected hardware capabilities of the host.
/// * `cloud_provider` — The name of the configured cloud provider (e.g., "openai").
///
/// # Returns
/// A `RoutingDecision` indicating where the request should execute.
pub fn resolve(
    policy: &RoutingPolicy,
    request: &InferenceRequest,
    admission: AdmissionOutcome,
    hardware: &HardwareCapability,
    cloud_provider: &str,
) -> RoutingDecision {
    match policy {
        RoutingPolicy::LocalOnly => resolve_local_only(request, admission),

        RoutingPolicy::CloudOnly => RoutingDecision::UseCloud {
            provider: cloud_provider.to_string(),
        },

        RoutingPolicy::LocalFirstWithCloudFallback => {
            resolve_local_first(request, admission, cloud_provider)
        }

        RoutingPolicy::LatencyOptimized => {
            resolve_latency_optimized(request, admission, hardware, cloud_provider)
        }

        RoutingPolicy::CostOptimized => resolve_cost_optimized(request, admission, cloud_provider),

        RoutingPolicy::DegradationLadder => {
            resolve_degradation_ladder(request, admission, hardware, cloud_provider)
        }
    }
}

/// LocalOnly: only use local backends; reject if admission fails.
fn resolve_local_only(request: &InferenceRequest, admission: AdmissionOutcome) -> RoutingDecision {
    match admission {
        AdmissionOutcome::Accepted => RoutingDecision::UseLocal {
            model_id: request.model_id.clone(),
        },
        AdmissionOutcome::Deferred => RoutingDecision::Rejected {
            reason: format!(
                "Local admission deferred for '{}' and cloud fallback is disabled (LocalOnly policy)",
                request.model_id
            ),
        },
        AdmissionOutcome::Rejected => RoutingDecision::Rejected {
            reason: format!(
                "Local admission rejected for '{}' ({}MB required) and cloud fallback is disabled (LocalOnly policy)",
                request.model_id, request.required_memory_mb
            ),
        },
    }
}

/// LocalFirstWithCloudFallback: try local, fall back to cloud on deferral/rejection.
fn resolve_local_first(
    request: &InferenceRequest,
    admission: AdmissionOutcome,
    cloud_provider: &str,
) -> RoutingDecision {
    match admission {
        AdmissionOutcome::Accepted => RoutingDecision::UseLocal {
            model_id: request.model_id.clone(),
        },
        AdmissionOutcome::Deferred | AdmissionOutcome::Rejected => RoutingDecision::UseCloud {
            provider: cloud_provider.to_string(),
        },
    }
}

/// LatencyOptimized: prefer local for GPU-accelerated hardware,
/// fall back to cloud otherwise or when memory is constrained.
fn resolve_latency_optimized(
    request: &InferenceRequest,
    admission: AdmissionOutcome,
    hardware: &HardwareCapability,
    cloud_provider: &str,
) -> RoutingDecision {
    // If local hardware has GPU acceleration and memory is available, use local
    if hardware.gpu_available && admission == AdmissionOutcome::Accepted {
        return RoutingDecision::UseLocal {
            model_id: request.model_id.clone(),
        };
    }
    // Otherwise, cloud is likely faster than CPU-only local inference
    RoutingDecision::UseCloud {
        provider: cloud_provider.to_string(),
    }
}

/// CostOptimized: always prefer local (free) when possible.
fn resolve_cost_optimized(
    request: &InferenceRequest,
    admission: AdmissionOutcome,
    cloud_provider: &str,
) -> RoutingDecision {
    match admission {
        AdmissionOutcome::Accepted | AdmissionOutcome::Deferred => {
            // Even deferred requests are worth waiting for locally to save cost
            RoutingDecision::UseLocal {
                model_id: request.model_id.clone(),
            }
        }
        AdmissionOutcome::Rejected => {
            // Only use cloud as absolute last resort
            RoutingDecision::UseCloud {
                provider: cloud_provider.to_string(),
            }
        }
    }
}

/// DegradationLadder: when local admission is deferred/rejected, walk down
/// the precision ladder (preferred → Q8 → Q4) to find a quantization level
/// that fits within available memory. Only falls back to cloud after all
/// local precision levels are exhausted.
fn resolve_degradation_ladder(
    request: &InferenceRequest,
    admission: AdmissionOutcome,
    hardware: &HardwareCapability,
    cloud_provider: &str,
) -> RoutingDecision {
    // If the model fits at the preferred precision, use it directly.
    if admission == AdmissionOutcome::Accepted {
        return RoutingDecision::UseLocal {
            model_id: request.model_id.clone(),
        };
    }

    // Walk the degradation ladder starting from the requested precision.
    let available_mb = hardware.available_memory_bytes / (1024 * 1024);
    let mut level = request.preferred_precision;

    while let Some(next) = level.next_lower() {
        let estimated_mb = estimate_memory_at_precision(
            request.required_memory_mb,
            &request.preferred_precision,
            &next,
        );
        if (estimated_mb as u64) <= available_mb {
            return RoutingDecision::UseLocalDegraded {
                model_id: request.model_id.clone(),
                degraded_precision: next,
                quality_warning: format!(
                    "Degraded from {} to {} due to memory pressure ({} MB available, {} MB needed at {})",
                    request.preferred_precision, next, available_mb, estimated_mb, next
                ),
            };
        }
        level = next;
    }

    // Exhausted all local options — fall back to cloud.
    RoutingDecision::UseCloud {
        provider: cloud_provider.to_string(),
    }
}

/// Estimate the memory footprint (in MB) of a model when loaded at a
/// different precision level.
///
/// Uses the ratio of bytes-per-parameter between the original and target
/// precisions to scale the original memory requirement.
fn estimate_memory_at_precision(
    original_memory_mb: usize,
    original_precision: &Precision,
    target_precision: &Precision,
) -> usize {
    let original_bpp = original_precision.bytes_per_param();
    let target_bpp = target_precision.bytes_per_param();
    // Scale linearly: mem_target = mem_original × (bpp_target / bpp_original)
    ((original_memory_mb as f64) * (target_bpp / original_bpp)).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{CpuFamily, GpuType, HardwareCapability, OsClassification};

    fn mock_hardware() -> HardwareCapability {
        HardwareCapability {
            os: OsClassification::MacOS,
            cpu_family: CpuFamily::AppleSilicon,
            gpu_available: true,
            gpu_type: Some(GpuType::Metal),
            total_memory_bytes: 17_179_869_184,    // 16 GB
            available_memory_bytes: 8_589_934_592, // 8 GB
        }
    }

    fn mock_request() -> InferenceRequest {
        InferenceRequest::new("llama-3-13b", "Hello", 13312)
    }

    #[test]
    fn test_local_only_accepts_when_admitted() {
        let decision = resolve(
            &RoutingPolicy::LocalOnly,
            &mock_request(),
            AdmissionOutcome::Accepted,
            &mock_hardware(),
            "openai",
        );
        assert_eq!(
            decision,
            RoutingDecision::UseLocal {
                model_id: "llama-3-13b".into()
            }
        );
    }

    #[test]
    fn test_local_only_rejects_when_memory_full() {
        let decision = resolve(
            &RoutingPolicy::LocalOnly,
            &mock_request(),
            AdmissionOutcome::Rejected,
            &mock_hardware(),
            "openai",
        );
        assert!(matches!(decision, RoutingDecision::Rejected { .. }));
    }

    #[test]
    fn test_local_only_never_falls_back_to_cloud() {
        let decision = resolve(
            &RoutingPolicy::LocalOnly,
            &mock_request(),
            AdmissionOutcome::Deferred,
            &mock_hardware(),
            "openai",
        );
        // Should reject, NOT fall back to cloud
        assert!(matches!(decision, RoutingDecision::Rejected { .. }));
    }

    #[test]
    fn test_cloud_only_always_uses_cloud() {
        let decision = resolve(
            &RoutingPolicy::CloudOnly,
            &mock_request(),
            AdmissionOutcome::Accepted, // Even if local would accept
            &mock_hardware(),
            "openai",
        );
        assert_eq!(
            decision,
            RoutingDecision::UseCloud {
                provider: "openai".into()
            }
        );
    }

    #[test]
    fn test_local_first_falls_back_to_cloud_on_rejection() {
        let decision = resolve(
            &RoutingPolicy::LocalFirstWithCloudFallback,
            &mock_request(),
            AdmissionOutcome::Rejected,
            &mock_hardware(),
            "openai",
        );
        assert_eq!(
            decision,
            RoutingDecision::UseCloud {
                provider: "openai".into()
            }
        );
    }

    #[test]
    fn test_local_first_uses_local_when_accepted() {
        let decision = resolve(
            &RoutingPolicy::LocalFirstWithCloudFallback,
            &mock_request(),
            AdmissionOutcome::Accepted,
            &mock_hardware(),
            "openai",
        );
        assert_eq!(
            decision,
            RoutingDecision::UseLocal {
                model_id: "llama-3-13b".into()
            }
        );
    }

    #[test]
    fn test_cost_optimized_prefers_local_even_when_deferred() {
        let decision = resolve(
            &RoutingPolicy::CostOptimized,
            &mock_request(),
            AdmissionOutcome::Deferred,
            &mock_hardware(),
            "openai",
        );
        // CostOptimized waits for local rather than paying for cloud
        assert_eq!(
            decision,
            RoutingDecision::UseLocal {
                model_id: "llama-3-13b".into()
            }
        );
    }

    #[test]
    fn test_latency_optimized_uses_cloud_without_gpu() {
        let hw = HardwareCapability {
            os: OsClassification::Linux,
            cpu_family: CpuFamily::X86_64,
            gpu_available: false,
            gpu_type: None,
            total_memory_bytes: 17_179_869_184,    // 16 GB
            available_memory_bytes: 8_589_934_592, // 8 GB
        };
        let decision = resolve(
            &RoutingPolicy::LatencyOptimized,
            &mock_request(),
            AdmissionOutcome::Accepted,
            &hw,
            "openai",
        );
        // No GPU → cloud is faster
        assert_eq!(
            decision,
            RoutingDecision::UseCloud {
                provider: "openai".into()
            }
        );
    }

    // ==================================================================
    // DegradationLadder policy tests
    // ==================================================================

    #[test]
    fn test_degradation_ladder_uses_local_when_accepted() {
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(),
            AdmissionOutcome::Accepted,
            &mock_hardware(),
            "openai",
        );
        assert_eq!(
            decision,
            RoutingDecision::UseLocal {
                model_id: "llama-3-13b".into()
            }
        );
    }

    #[test]
    fn test_degradation_ladder_degrades_to_q8_when_deferred() {
        // Request: 13312 MB at F16 (2 bpp), available: 8192 MB
        // Q8 estimate: 13312 * (1.0 / 2.0) = 6656 MB → fits in 8192 MB
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(), // F16, 13312 MB
            AdmissionOutcome::Deferred,
            &mock_hardware(), // 8 GB available
            "openai",
        );
        match decision {
            RoutingDecision::UseLocalDegraded {
                model_id,
                degraded_precision,
                ..
            } => {
                assert_eq!(model_id, "llama-3-13b");
                assert_eq!(degraded_precision, Precision::Q8);
            }
            other => panic!("Expected UseLocalDegraded, got {:?}", other),
        }
    }

    #[test]
    fn test_degradation_ladder_degrades_to_q4_when_q8_too_large() {
        // Constrained hardware: only 4 GB available
        // Request: 13312 MB at F16 (2 bpp)
        // Q8 estimate: 13312 * (1.0 / 2.0) = 6656 MB → does NOT fit in 4096 MB
        // Q4 estimate: 13312 * (0.5 / 2.0) = 3328 MB → fits in 4096 MB
        let constrained_hw = HardwareCapability {
            os: OsClassification::MacOS,
            cpu_family: CpuFamily::AppleSilicon,
            gpu_available: true,
            gpu_type: Some(GpuType::Metal),
            total_memory_bytes: 8_589_934_592,     // 8 GB total
            available_memory_bytes: 4_294_967_296, // 4 GB available
        };
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(),
            AdmissionOutcome::Rejected,
            &constrained_hw,
            "openai",
        );
        match decision {
            RoutingDecision::UseLocalDegraded {
                model_id,
                degraded_precision,
                ..
            } => {
                assert_eq!(model_id, "llama-3-13b");
                assert_eq!(degraded_precision, Precision::Q4);
            }
            other => panic!("Expected UseLocalDegraded at Q4, got {:?}", other),
        }
    }

    #[test]
    fn test_degradation_ladder_falls_back_to_cloud_when_all_levels_exhausted() {
        // Extremely constrained hardware: only 1 GB available
        // Q4 estimate: 13312 * (0.5 / 2.0) = 3328 MB → does NOT fit in 1024 MB
        let tiny_hw = HardwareCapability {
            os: OsClassification::MacOS,
            cpu_family: CpuFamily::AppleSilicon,
            gpu_available: true,
            gpu_type: Some(GpuType::Metal),
            total_memory_bytes: 4_294_967_296,     // 4 GB total
            available_memory_bytes: 1_073_741_824, // 1 GB available
        };
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(),
            AdmissionOutcome::Rejected,
            &tiny_hw,
            "openai",
        );
        assert_eq!(
            decision,
            RoutingDecision::UseCloud {
                provider: "openai".into()
            }
        );
    }

    #[test]
    fn test_degradation_ladder_no_degradation_possible_from_q4() {
        // Request already at Q4 — no further degradation possible
        let req = InferenceRequest::new("small-model", "Hello", 5000).with_precision(Precision::Q4);
        let constrained_hw = HardwareCapability {
            os: OsClassification::MacOS,
            cpu_family: CpuFamily::AppleSilicon,
            gpu_available: true,
            gpu_type: Some(GpuType::Metal),
            total_memory_bytes: 8_589_934_592,
            available_memory_bytes: 2_147_483_648, // 2 GB
        };
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &req,
            AdmissionOutcome::Rejected,
            &constrained_hw,
            "openai",
        );
        // Q4 has no next_lower() → must fall back to cloud
        assert_eq!(
            decision,
            RoutingDecision::UseCloud {
                provider: "openai".into()
            }
        );
    }

    #[test]
    fn test_memory_estimation_scales_correctly() {
        // 14000 MB at F16, estimate at Q8: 14000 * (1.0 / 2.0) = 7000
        assert_eq!(
            estimate_memory_at_precision(14000, &Precision::F16, &Precision::Q8),
            7000
        );
        // 14000 MB at F16, estimate at Q4: 14000 * (0.5 / 2.0) = 3500
        assert_eq!(
            estimate_memory_at_precision(14000, &Precision::F16, &Precision::Q4),
            3500
        );
        // 28000 MB at F32, estimate at Q4: 28000 * (0.5 / 4.0) = 3500
        assert_eq!(
            estimate_memory_at_precision(28000, &Precision::F32, &Precision::Q4),
            3500
        );
    }

    #[test]
    fn test_precision_next_lower() {
        assert_eq!(Precision::F32.next_lower(), Some(Precision::F16));
        assert_eq!(Precision::F16.next_lower(), Some(Precision::Q8));
        assert_eq!(Precision::Q8.next_lower(), Some(Precision::Q4));
        assert_eq!(Precision::Q4.next_lower(), None);
    }

    #[test]
    fn test_precision_bytes_per_param() {
        assert_eq!(Precision::F32.bytes_per_param(), 4.0);
        assert_eq!(Precision::F16.bytes_per_param(), 2.0);
        assert_eq!(Precision::Q8.bytes_per_param(), 1.0);
        assert_eq!(Precision::Q4.bytes_per_param(), 0.5);
    }
}
