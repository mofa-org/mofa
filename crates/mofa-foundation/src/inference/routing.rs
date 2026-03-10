//! Inference routing policy engine.
//!
//! Determines where an inference request should execute (local vs cloud)
//! based on the configured policy, memory scheduler outcome, and
//! hardware capabilities.

use std::fmt;

use crate::hardware::{CpuFamily, HardwareCapability, OsClassification};
use crate::scheduler::AdmissionOutcome;

use super::types::{InferenceRequest, Precision, RequestPriority, RoutedBackend};

/// A snapshot of the orchestrator's live memory state.
///
/// Passed into routing decisions so that policies (especially the
/// [`DegradationLadder`](RoutingPolicy::DegradationLadder)) can compare
/// estimated model footprints against **currently available** memory
/// rather than the stale `HardwareCapability` snapshot captured at boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemorySnapshot {
    /// Total configured memory capacity in MB.
    pub capacity_mb: usize,
    /// Memory currently allocated by loaded models in MB.
    pub allocated_mb: usize,
    /// Memory available for new models: `capacity_mb - allocated_mb`.
    pub available_mb: usize,
}

/// Policy governing how inference requests are routed between
/// local backends and cloud providers.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

impl fmt::Display for RoutingPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalOnly => write!(f, "local-only"),
            Self::CloudOnly => write!(f, "cloud-only"),
            Self::LocalFirstWithCloudFallback => write!(f, "local-first"),
            Self::LatencyOptimized => write!(f, "latency-optimized"),
            Self::CostOptimized => write!(f, "cost-optimized"),
            Self::DegradationLadder => write!(f, "degradation-ladder"),
        }
    }
}

impl fmt::Display for RoutingDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UseLocal { model_id } => write!(f, "local({})", model_id),
            Self::UseLocalDegraded {
                model_id,
                degraded_precision,
                ..
            } => write!(f, "local-degraded({}@{})", model_id, degraded_precision),
            Self::UseCloud { provider } => write!(f, "cloud({})", provider),
            Self::Rejected { reason } => write!(f, "rejected({})", reason),
        }
    }
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
    memory: &MemorySnapshot,
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
            resolve_degradation_ladder(request, admission, memory, cloud_provider)
        }
    }
}

/// LocalOnly: only use local backends; reject if admission fails.
fn resolve_local_only(request: &InferenceRequest, admission: AdmissionOutcome) -> RoutingDecision {
    match admission {
        AdmissionOutcome::Accept => RoutingDecision::UseLocal {
            model_id: request.model_id.clone(),
        },
        AdmissionOutcome::Defer => RoutingDecision::Rejected {
            reason: format!(
                "Local admission deferred for '{}' and cloud fallback is disabled (LocalOnly policy)",
                request.model_id
            ),
        },
        AdmissionOutcome::Reject => RoutingDecision::Rejected {
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
        AdmissionOutcome::Accept => RoutingDecision::UseLocal {
            model_id: request.model_id.clone(),
        },
        AdmissionOutcome::Defer | AdmissionOutcome::Reject => RoutingDecision::UseCloud {
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
    if hardware.gpu_available && admission == AdmissionOutcome::Accept {
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
        AdmissionOutcome::Accept | AdmissionOutcome::Defer => {
            // Even deferred requests are worth waiting for locally to save cost
            RoutingDecision::UseLocal {
                model_id: request.model_id.clone(),
            }
        }
        AdmissionOutcome::Reject => {
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
    memory: &MemorySnapshot,
    cloud_provider: &str,
) -> RoutingDecision {
    // If the model fits at the preferred precision, use it directly.
    if admission == AdmissionOutcome::Accept {
        return RoutingDecision::UseLocal {
            model_id: request.model_id.clone(),
        };
    }

    // Walk the degradation ladder starting from the requested precision.
    // Use the LIVE available memory from the orchestrator's ModelPool,
    // not the stale hardware snapshot captured at boot time.
    let available_mb = memory.available_mb;
    let mut level = request.preferred_precision;

    while let Some(next) = level.next_lower() {
        let estimated_mb = estimate_memory_at_precision(
            request.required_memory_mb,
            &request.preferred_precision,
            &next,
        );
        if estimated_mb <= available_mb {
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

    /// Default memory snapshot for non-degradation tests (plenty of space).
    fn mock_memory() -> MemorySnapshot {
        MemorySnapshot {
            capacity_mb: 16384,
            allocated_mb: 0,
            available_mb: 16384,
        }
    }

    #[test]
    fn test_local_only_accepts_when_admitted() {
        let decision = resolve(
            &RoutingPolicy::LocalOnly,
            &mock_request(),
            AdmissionOutcome::Accept,
            &mock_hardware(),
            "openai",
            &mock_memory(),
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
            AdmissionOutcome::Reject,
            &mock_hardware(),
            "openai",
            &mock_memory(),
        );
        assert!(matches!(decision, RoutingDecision::Rejected { .. }));
    }

    #[test]
    fn test_local_only_never_falls_back_to_cloud() {
        let decision = resolve(
            &RoutingPolicy::LocalOnly,
            &mock_request(),
            AdmissionOutcome::Defer,
            &mock_hardware(),
            "openai",
            &mock_memory(),
        );
        // Should reject, NOT fall back to cloud
        assert!(matches!(decision, RoutingDecision::Rejected { .. }));
    }

    #[test]
    fn test_cloud_only_always_uses_cloud() {
        let decision = resolve(
            &RoutingPolicy::CloudOnly,
            &mock_request(),
            AdmissionOutcome::Accept, // Even if local would accept
            &mock_hardware(),
            "openai",
            &mock_memory(),
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
            AdmissionOutcome::Reject,
            &mock_hardware(),
            "openai",
            &mock_memory(),
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
            AdmissionOutcome::Accept,
            &mock_hardware(),
            "openai",
            &mock_memory(),
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
            AdmissionOutcome::Defer,
            &mock_hardware(),
            "openai",
            &mock_memory(),
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
            AdmissionOutcome::Accept,
            &hw,
            "openai",
            &mock_memory(),
        );
        // No GPU → cloud is faster
        assert_eq!(
            decision,
            RoutingDecision::UseCloud {
                provider: "openai".into()
            }
        );
    }

    #[test]
    fn test_routing_policy_display() {
        assert_eq!(format!("{}", RoutingPolicy::LocalOnly), "local-only");
        assert_eq!(format!("{}", RoutingPolicy::CloudOnly), "cloud-only");
        assert_eq!(
            format!("{}", RoutingPolicy::LocalFirstWithCloudFallback),
            "local-first"
        );
        assert_eq!(
            format!("{}", RoutingPolicy::LatencyOptimized),
            "latency-optimized"
        );
        assert_eq!(
            format!("{}", RoutingPolicy::CostOptimized),
            "cost-optimized"
        );
    }

    #[test]
    fn test_routing_decision_display() {
        assert_eq!(
            format!(
                "{}",
                RoutingDecision::UseLocal {
                    model_id: "llama-3".into()
                }
            ),
            "local(llama-3)"
        );
        assert_eq!(
            format!(
                "{}",
                RoutingDecision::UseCloud {
                    provider: "openai".into()
                }
            ),
            "cloud(openai)"
        );
        assert_eq!(
            format!(
                "{}",
                RoutingDecision::Rejected {
                    reason: "no memory".into()
                }
            ),
            "rejected(no memory)"
        );
    }

    #[test]
    fn test_admission_outcome_display() {
        assert_eq!(format!("{}", AdmissionOutcome::Accept), "Accept");
        assert_eq!(format!("{}", AdmissionOutcome::Defer), "Defer");
        assert_eq!(format!("{}", AdmissionOutcome::Reject), "Reject");
    }

    #[test]
    fn test_routing_policy_serde_roundtrip() {
        for variant in [
            RoutingPolicy::LocalOnly,
            RoutingPolicy::CloudOnly,
            RoutingPolicy::LocalFirstWithCloudFallback,
            RoutingPolicy::LatencyOptimized,
            RoutingPolicy::CostOptimized,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: RoutingPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn test_routing_decision_serde_roundtrip() {
        let variants = vec![
            RoutingDecision::UseLocal {
                model_id: "llama-3".into(),
            },
            RoutingDecision::UseCloud {
                provider: "openai".into(),
            },
            RoutingDecision::Rejected {
                reason: "no memory".into(),
            },
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let back: RoutingDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    // ==================================================================
    // DegradationLadder policy tests
    // ==================================================================

    #[test]
    fn test_degradation_ladder_uses_local_when_accepted() {
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(),
            AdmissionOutcome::Accept,
            &mock_hardware(),
            "openai",
            &mock_memory(),
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
        let mem = MemorySnapshot {
            capacity_mb: 16384,
            allocated_mb: 8192,
            available_mb: 8192,
        };
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(), // F16, 13312 MB
            AdmissionOutcome::Defer,
            &mock_hardware(),
            "openai",
            &mem,
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
    fn test_admission_outcome_serde_roundtrip() {
        for variant in [
            AdmissionOutcome::Accept,
            AdmissionOutcome::Defer,
            AdmissionOutcome::Reject,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: AdmissionOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }
    #[test]
    fn test_degradation_ladder_degrades_to_q4_when_q8_too_large() {
        // Constrained: only 4096 MB available (live budget)
        // Request: 13312 MB at F16 (2 bpp)
        // Q8 estimate: 13312 * (1.0 / 2.0) = 6656 MB → does NOT fit in 4096 MB
        // Q4 estimate: 13312 * (0.5 / 2.0) = 3328 MB → fits in 4096 MB
        let mem = MemorySnapshot {
            capacity_mb: 8192,
            allocated_mb: 4096,
            available_mb: 4096,
        };
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(),
            AdmissionOutcome::Reject,
            &mock_hardware(),
            "openai",
            &mem,
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
        // Extremely constrained: only 1024 MB available
        // Q4 estimate: 13312 * (0.5 / 2.0) = 3328 MB → does NOT fit in 1024 MB
        let mem = MemorySnapshot {
            capacity_mb: 4096,
            allocated_mb: 3072,
            available_mb: 1024,
        };
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(),
            AdmissionOutcome::Reject,
            &mock_hardware(),
            "openai",
            &mem,
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
        let mem = MemorySnapshot {
            capacity_mb: 8192,
            allocated_mb: 6144,
            available_mb: 2048,
        };
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &req,
            AdmissionOutcome::Reject,
            &mock_hardware(),
            "openai",
            &mem,
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

    // ==================================================================
    // Regression test for #1114: stale memory snapshot
    // ==================================================================

    /// Regression test for issue #1114.
    ///
    /// The degradation ladder must use the live `MemorySnapshot` (derived from
    /// `ModelPool`) rather than the stale `HardwareCapability` snapshot that
    /// is captured once at `InferenceOrchestrator::new()` time.
    ///
    /// Scenario: hardware reports 16 GB available, but 12 GB is already
    /// allocated by other models (live available = 4 GB). The ladder should
    /// see 4 GB — NOT 16 GB — and degrade to Q4 instead of approving Q8.
    #[test]
    fn test_degradation_ladder_uses_live_memory_not_hardware_snapshot() {
        // Hardware says 16 GB available (captured at boot)
        let stale_hw = HardwareCapability {
            os: OsClassification::MacOS,
            cpu_family: CpuFamily::AppleSilicon,
            gpu_available: true,
            gpu_type: Some(GpuType::Metal),
            total_memory_bytes: 17_179_869_184,     // 16 GB total
            available_memory_bytes: 17_179_869_184, // 16 GB available (stale!)
        };

        // But ModelPool has 12 GB allocated → only 4 GB truly available
        let live_mem = MemorySnapshot {
            capacity_mb: 16384,
            allocated_mb: 12288, // 12 GB already loaded
            available_mb: 4096,  // only 4 GB left
        };

        // Request: 13312 MB at F16
        // Q8 estimate: 6656 MB → does NOT fit in 4096 MB live available
        // Q4 estimate: 3328 MB → fits in 4096 MB live available
        //
        // If the ladder incorrectly used hardware (16 GB), Q8 would be
        // approved — causing OOM. With the fix, it correctly degrades to Q4.
        let decision = resolve(
            &RoutingPolicy::DegradationLadder,
            &mock_request(), // F16, 13312 MB
            AdmissionOutcome::Reject,
            &stale_hw,
            "openai",
            &live_mem,
        );

        match decision {
            RoutingDecision::UseLocalDegraded {
                degraded_precision, ..
            } => {
                assert_eq!(
                    degraded_precision,
                    Precision::Q4,
                    "Ladder must use live memory (4 GB), not stale hardware (16 GB). \
                     Q8 (6656 MB) should NOT fit; Q4 (3328 MB) should."
                );
            }
            other => panic!(
                "Expected UseLocalDegraded at Q4 with live memory, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_memory_snapshot_serde_roundtrip() {
        let snap = MemorySnapshot {
            capacity_mb: 16384,
            allocated_mb: 8000,
            available_mb: 8384,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: MemorySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snap);
    }
}
