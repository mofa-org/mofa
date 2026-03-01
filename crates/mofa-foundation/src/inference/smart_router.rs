//! Smart Routing Policy Engine for multi-provider selection.
//!
//! Extends the existing [`routing::resolve`] function with a stateful
//! `SmartRouter` that manages a registry of providers and selects the
//! best one based on task type, routing policy, and provider capabilities.
//!
//! # Design Principles
//!
//! - **Extends, does not replace** the existing `routing` module.
//! - **No retry logic** — that belongs in `mofa-runtime`.
//! - **No admission control** — that is handled by `InferenceOrchestrator`.
//! - **No precision degradation** — that is a model-pool concern.
//!
//! # Example
//!
//! ```rust
//! use mofa_foundation::inference::smart_router::{
//!     SmartRouter, ProviderEntry, TaskType,
//! };
//! use mofa_foundation::inference::RoutingPolicy;
//!
//! let mut router = SmartRouter::new(RoutingPolicy::LocalFirstWithCloudFallback);
//!
//! router.register_provider(ProviderEntry {
//!     id: "local-llama".into(),
//!     name: "Llama 3 (local)".into(),
//!     is_local: true,
//!     supported_tasks: vec![TaskType::Llm],
//!     latency_ms: 80,
//!     cost_per_1k_tokens: 0.0,
//! });
//!
//! router.register_provider(ProviderEntry {
//!     id: "openai-gpt4".into(),
//!     name: "GPT-4 (cloud)".into(),
//!     is_local: false,
//!     supported_tasks: vec![TaskType::Llm, TaskType::Embedding],
//!     latency_ms: 200,
//!     cost_per_1k_tokens: 0.03,
//! });
//!
//! let decision = router.route(TaskType::Llm);
//! assert!(decision.is_some());
//! ```

use super::routing::RoutingPolicy;

/// The category of inference task.
///
/// Used by [`SmartRouter`] to filter providers by capability before
/// applying the routing policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TaskType {
    /// Large Language Model (chat, completion)
    Llm,
    /// Automatic Speech Recognition (transcription)
    Asr,
    /// Text-to-Speech synthesis
    Tts,
    /// Text embedding / vector generation
    Embedding,
    /// Vision-Language Model (multimodal)
    Vlm,
}

impl TaskType {
    /// Parse a task type from a human-readable string.
    ///
    /// Returns `None` for unrecognised strings.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "llm" | "language" | "chat" | "completion" => Some(Self::Llm),
            "asr" | "speech" | "transcription" => Some(Self::Asr),
            "tts" | "synthesis" | "audio" => Some(Self::Tts),
            "embedding" | "vector" | "vec" => Some(Self::Embedding),
            "vlm" | "vision" | "multimodal" => Some(Self::Vlm),
            _ => None,
        }
    }
}

/// A registered inference provider.
///
/// Holds static metadata used by the router to rank providers.
/// Does **not** hold connections or runtime state — that is the
/// responsibility of the caller (e.g., `InferenceOrchestrator`).
#[derive(Debug, Clone)]
pub struct ProviderEntry {
    /// Unique identifier (e.g., `"local-llama"`, `"openai-gpt4"`)
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// `true` for local/on-device backends, `false` for cloud APIs
    pub is_local: bool,
    /// Task types this provider can handle
    pub supported_tasks: Vec<TaskType>,
    /// Expected latency in milliseconds
    pub latency_ms: u64,
    /// Cost per 1 000 tokens (0.0 for local backends)
    pub cost_per_1k_tokens: f32,
}

/// The outcome of a [`SmartRouter::route`] call.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteSelection {
    /// The chosen provider's id
    pub provider_id: String,
    /// The chosen provider's human-readable name
    pub provider_name: String,
    /// Whether the provider is local
    pub is_local: bool,
    /// Short tag explaining why this provider was chosen (e.g., `"local-first"`)
    pub reason: &'static str,
}

/// A stateful, policy-driven router that selects among registered
/// providers based on task type and routing policy.
///
/// This complements the existing stateless [`super::routing::resolve`]
/// function by adding:
/// - A provider registry
/// - Task-type filtering
/// - Multi-provider ranking within the same policy
pub struct SmartRouter {
    policy: RoutingPolicy,
    providers: Vec<ProviderEntry>,
}

impl SmartRouter {
    /// Create a new router with the given default policy.
    pub fn new(policy: RoutingPolicy) -> Self {
        Self {
            policy,
            providers: Vec::new(),
        }
    }

    /// Register a provider. Duplicate IDs are silently ignored.
    pub fn register_provider(&mut self, entry: ProviderEntry) {
        if !self.providers.iter().any(|p| p.id == entry.id) {
            self.providers.push(entry);
        }
    }

    /// Remove a provider by id.
    pub fn unregister_provider(&mut self, id: &str) {
        self.providers.retain(|p| p.id != id);
    }

    /// Return a snapshot of all registered providers.
    pub fn providers(&self) -> &[ProviderEntry] {
        &self.providers
    }

    /// Override the active routing policy.
    pub fn set_policy(&mut self, policy: RoutingPolicy) {
        self.policy = policy;
    }

    /// Return the active routing policy.
    pub fn policy(&self) -> &RoutingPolicy {
        &self.policy
    }

    /// Select the best provider for the given task type under the
    /// active routing policy.
    ///
    /// Returns `None` when no registered provider supports the task.
    pub fn route(&self, task: TaskType) -> Option<RouteSelection> {
        self.route_with_policy(task, &self.policy)
    }

    /// Select the best provider using an explicit (override) policy.
    pub fn route_with_policy(
        &self,
        task: TaskType,
        policy: &RoutingPolicy,
    ) -> Option<RouteSelection> {
        // Step 1: filter to providers that support this task type
        let eligible: Vec<&ProviderEntry> = self
            .providers
            .iter()
            .filter(|p| p.supported_tasks.contains(&task))
            .collect();

        if eligible.is_empty() {
            return None;
        }

        // Step 2: rank according to policy
        match policy {
            RoutingPolicy::LocalOnly => Self::pick_local(&eligible, "local-only"),
            RoutingPolicy::CloudOnly => Self::pick_cloud(&eligible, "cloud-only"),
            RoutingPolicy::LocalFirstWithCloudFallback => {
                Self::pick_local(&eligible, "local-first")
                    .or_else(|| Self::pick_cloud(&eligible, "local-first-fallback"))
            }
            RoutingPolicy::LatencyOptimized => Self::pick_lowest_latency(&eligible),
            RoutingPolicy::CostOptimized => Self::pick_lowest_cost(&eligible),
        }
    }

    // ── private helpers ─────────────────────────────────────────

    /// Pick the local provider with the lowest latency, if any.
    fn pick_local(eligible: &[&ProviderEntry], reason: &'static str) -> Option<RouteSelection> {
        eligible
            .iter()
            .filter(|p| p.is_local)
            .min_by_key(|p| p.latency_ms)
            .map(|p| RouteSelection {
                provider_id: p.id.clone(),
                provider_name: p.name.clone(),
                is_local: true,
                reason,
            })
    }

    /// Pick the cloud provider with the lowest latency, if any.
    fn pick_cloud(eligible: &[&ProviderEntry], reason: &'static str) -> Option<RouteSelection> {
        eligible
            .iter()
            .filter(|p| !p.is_local)
            .min_by_key(|p| p.latency_ms)
            .map(|p| RouteSelection {
                provider_id: p.id.clone(),
                provider_name: p.name.clone(),
                is_local: false,
                reason,
            })
    }

    /// Pick the provider with the absolute lowest latency.
    fn pick_lowest_latency(eligible: &[&ProviderEntry]) -> Option<RouteSelection> {
        eligible
            .iter()
            .min_by_key(|p| p.latency_ms)
            .map(|p| RouteSelection {
                provider_id: p.id.clone(),
                provider_name: p.name.clone(),
                is_local: p.is_local,
                reason: "latency-optimized",
            })
    }

    /// Pick the provider with the lowest cost (local = 0.0 wins ties).
    fn pick_lowest_cost(eligible: &[&ProviderEntry]) -> Option<RouteSelection> {
        eligible
            .iter()
            .min_by(|a, b| {
                a.cost_per_1k_tokens
                    .partial_cmp(&b.cost_per_1k_tokens)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|p| RouteSelection {
                provider_id: p.id.clone(),
                provider_name: p.name.clone(),
                is_local: p.is_local,
                reason: "cost-optimized",
            })
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::routing::RoutingPolicy;

    fn local_llm() -> ProviderEntry {
        ProviderEntry {
            id: "local-llama".into(),
            name: "Llama 3 local".into(),
            is_local: true,
            supported_tasks: vec![TaskType::Llm],
            latency_ms: 80,
            cost_per_1k_tokens: 0.0,
        }
    }

    fn cloud_llm() -> ProviderEntry {
        ProviderEntry {
            id: "openai-gpt4".into(),
            name: "GPT-4".into(),
            is_local: false,
            supported_tasks: vec![TaskType::Llm, TaskType::Embedding],
            latency_ms: 200,
            cost_per_1k_tokens: 0.03,
        }
    }

    fn local_tts() -> ProviderEntry {
        ProviderEntry {
            id: "local-kokoro".into(),
            name: "Kokoro TTS".into(),
            is_local: true,
            supported_tasks: vec![TaskType::Tts],
            latency_ms: 40,
            cost_per_1k_tokens: 0.0,
        }
    }

    fn cloud_tts() -> ProviderEntry {
        ProviderEntry {
            id: "elevenlabs".into(),
            name: "ElevenLabs".into(),
            is_local: false,
            supported_tasks: vec![TaskType::Tts],
            latency_ms: 150,
            cost_per_1k_tokens: 0.015,
        }
    }

    // ── local-first policy ──────────────────────────────────────

    #[test]
    fn local_first_prefers_local_provider() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalFirstWithCloudFallback);
        router.register_provider(cloud_llm());
        router.register_provider(local_llm());

        let sel = router.route(TaskType::Llm).unwrap();
        assert_eq!(sel.provider_id, "local-llama");
        assert!(sel.is_local);
        assert_eq!(sel.reason, "local-first");
    }

    #[test]
    fn local_first_falls_back_to_cloud_when_no_local() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalFirstWithCloudFallback);
        router.register_provider(cloud_llm());

        let sel = router.route(TaskType::Llm).unwrap();
        assert_eq!(sel.provider_id, "openai-gpt4");
        assert!(!sel.is_local);
        assert_eq!(sel.reason, "local-first-fallback");
    }

    // ── cloud-only policy ───────────────────────────────────────

    #[test]
    fn cloud_only_ignores_local_providers() {
        let mut router = SmartRouter::new(RoutingPolicy::CloudOnly);
        router.register_provider(local_llm());
        router.register_provider(cloud_llm());

        let sel = router.route(TaskType::Llm).unwrap();
        assert_eq!(sel.provider_id, "openai-gpt4");
    }

    #[test]
    fn cloud_only_returns_none_when_no_cloud() {
        let mut router = SmartRouter::new(RoutingPolicy::CloudOnly);
        router.register_provider(local_llm());

        assert!(router.route(TaskType::Llm).is_none());
    }

    // ── local-only policy ───────────────────────────────────────

    #[test]
    fn local_only_returns_none_when_no_local() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalOnly);
        router.register_provider(cloud_llm());

        assert!(router.route(TaskType::Llm).is_none());
    }

    // ── latency-optimized policy ────────────────────────────────

    #[test]
    fn latency_optimized_picks_fastest_provider() {
        let mut router = SmartRouter::new(RoutingPolicy::LatencyOptimized);
        router.register_provider(cloud_llm()); // 200 ms
        router.register_provider(local_llm()); // 80 ms

        let sel = router.route(TaskType::Llm).unwrap();
        assert_eq!(sel.provider_id, "local-llama");
        assert_eq!(sel.reason, "latency-optimized");
    }

    // ── cost-optimized policy ───────────────────────────────────

    #[test]
    fn cost_optimized_picks_cheapest_provider() {
        let mut router = SmartRouter::new(RoutingPolicy::CostOptimized);
        router.register_provider(cloud_llm()); // $0.03
        router.register_provider(local_llm()); // $0.00

        let sel = router.route(TaskType::Llm).unwrap();
        assert_eq!(sel.provider_id, "local-llama");
        assert_eq!(sel.reason, "cost-optimized");
    }

    // ── task-type filtering ─────────────────────────────────────

    #[test]
    fn routes_tts_task_to_tts_providers_only() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalFirstWithCloudFallback);
        router.register_provider(local_llm());
        router.register_provider(cloud_llm());
        router.register_provider(local_tts());
        router.register_provider(cloud_tts());

        let sel = router.route(TaskType::Tts).unwrap();
        assert_eq!(sel.provider_id, "local-kokoro");
    }

    #[test]
    fn returns_none_for_unsupported_task_type() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalFirstWithCloudFallback);
        router.register_provider(local_llm());

        assert!(router.route(TaskType::Asr).is_none());
    }

    // ── route_with_policy override ──────────────────────────────

    #[test]
    fn route_with_policy_overrides_default() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalOnly);
        router.register_provider(cloud_llm());

        // Default policy (LocalOnly) would return None
        assert!(router.route(TaskType::Llm).is_none());

        // Override to CloudOnly
        let sel = router
            .route_with_policy(TaskType::Llm, &RoutingPolicy::CloudOnly)
            .unwrap();
        assert_eq!(sel.provider_id, "openai-gpt4");
    }

    // ── provider management ─────────────────────────────────────

    #[test]
    fn duplicate_provider_is_ignored() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalFirstWithCloudFallback);
        router.register_provider(local_llm());
        router.register_provider(local_llm()); // duplicate

        assert_eq!(router.providers().len(), 1);
    }

    #[test]
    fn unregister_removes_provider() {
        let mut router = SmartRouter::new(RoutingPolicy::LocalFirstWithCloudFallback);
        router.register_provider(local_llm());
        router.register_provider(cloud_llm());

        router.unregister_provider("local-llama");
        assert_eq!(router.providers().len(), 1);

        let sel = router.route(TaskType::Llm).unwrap();
        assert_eq!(sel.provider_id, "openai-gpt4");
    }

    // ── TaskType::from_str_opt ──────────────────────────────────

    #[test]
    fn task_type_parses_known_strings() {
        assert_eq!(TaskType::from_str_opt("llm"), Some(TaskType::Llm));
        assert_eq!(TaskType::from_str_opt("Chat"), Some(TaskType::Llm));
        assert_eq!(TaskType::from_str_opt("ASR"), Some(TaskType::Asr));
        assert_eq!(TaskType::from_str_opt("tts"), Some(TaskType::Tts));
        assert_eq!(
            TaskType::from_str_opt("embedding"),
            Some(TaskType::Embedding)
        );
        assert_eq!(TaskType::from_str_opt("vision"), Some(TaskType::Vlm));
        assert_eq!(TaskType::from_str_opt("unknown"), None);
    }
}
