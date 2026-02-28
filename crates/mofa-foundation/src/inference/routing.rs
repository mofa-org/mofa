//! Inference routing policy engine.
//!
//! Determines where an inference request should execute (local vs cloud)
//! based on the configured policy, memory scheduler outcome, and
//! hardware capabilities.

use crate::hardware::{CpuFamily, HardwareCapability, OsClassification};

use super::types::{InferenceRequest, RequestPriority, RoutedBackend};

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
}

/// The outcome of a routing decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Run inference on a local backend.
    UseLocal { model_id: String },
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
}
