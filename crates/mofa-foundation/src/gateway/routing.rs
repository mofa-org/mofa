//! Concrete routing strategy implementations for the gateway.
//!
//! # Strategies
//!
//! | Type | Description |
//! |------|-------------|
//! | [`WeightedRoundRobinRouter`] | Distributes requests proportionally across weighted agent backends |
//! | [`CapabilityMatchRouter`] | Selects the highest-scoring agent for a task description |
//! | [`RouterRegistry`] | Maps route IDs to their assigned strategy |

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use mofa_kernel::gateway::envelope::RequestEnvelope;
use mofa_kernel::gateway::routing::RoutingStrategy;
use tracing::warn;

// ─────────────────────────────────────────────────────────────────────────────
// WeightedRoundRobinRouter
// ─────────────────────────────────────────────────────────────────────────────

/// Distributes requests proportionally across a set of weighted agent backends.
///
/// Selection is atomic — a global counter is incremented on every call and
/// mapped to a backend via a cumulative-weight lookup, so concurrent gateway
/// threads never double-select the same slot.
///
/// Weights can be updated at runtime via [`update_backends`] without dropping
/// the router or interrupting in-flight requests.
pub struct WeightedRoundRobinRouter {
    /// Snapshot of backends used for selection.
    /// Stored behind RwLock so `update_backends` can swap it atomically.
    backends: RwLock<WeightedBackends>,
    /// Monotonically increasing request counter.
    counter: AtomicU64,
}

struct WeightedBackends {
    /// Parallel vec of agent IDs.
    agents: Vec<String>,
    /// Cumulative weights for O(log n) selection.
    cumulative: Vec<u64>,
    /// Total weight sum.
    total: u64,
}

impl WeightedBackends {
    fn new(backends: Vec<(String, u32)>) -> Self {
        let mut agents = Vec::with_capacity(backends.len());
        let mut cumulative = Vec::with_capacity(backends.len());
        let mut running = 0u64;
        for (id, w) in backends {
            running += w as u64;
            agents.push(id);
            cumulative.push(running);
        }
        Self {
            agents,
            cumulative,
            total: running,
        }
    }

    fn select(&self, slot: u64) -> Option<&str> {
        if self.total == 0 {
            return None;
        }
        let target = (slot % self.total) + 1;
        // Binary search for the first cumulative weight >= target.
        let idx = self.cumulative.partition_point(|&c| c < target);
        self.agents.get(idx).map(|s| s.as_str())
    }
}

impl WeightedRoundRobinRouter {
    /// Create a new router with the given `(agent_id, weight)` pairs.
    ///
    /// Backends with weight `0` are ignored.
    pub fn new(backends: Vec<(impl Into<String>, u32)>) -> Self {
        let backends = backends
            .into_iter()
            .filter(|(_, w)| *w > 0)
            .map(|(id, w)| (id.into(), w))
            .collect();
        Self {
            backends: RwLock::new(WeightedBackends::new(backends)),
            counter: AtomicU64::new(0),
        }
    }

    /// Replace the backend list at runtime.  In-flight selections complete
    /// against the old list; new selections use the new list.
    pub fn update_backends(&self, backends: Vec<(impl Into<String>, u32)>) {
        let backends = backends
            .into_iter()
            .filter(|(_, w)| *w > 0)
            .map(|(id, w)| (id.into(), w))
            .collect();
        let new = WeightedBackends::new(backends);
        *self.backends.write().unwrap() = new;
    }
}

impl RoutingStrategy for WeightedRoundRobinRouter {
    fn select_agent(&self, _envelope: &RequestEnvelope) -> Option<String> {
        let slot = self.counter.fetch_add(1, Ordering::Relaxed);
        let backends = self.backends.read().unwrap();
        backends.select(slot).map(String::from)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AgentScorer — extensibility seam for CapabilityRegistry (#404)
// ─────────────────────────────────────────────────────────────────────────────

/// Scores agents by how well they match a task description.
///
/// This trait is the extensibility seam between the `CapabilityMatchRouter`
/// and the `CapabilityRegistry` introduced in issue #404.  Once that PR
/// merges, `CapabilityRegistry` implements `AgentScorer` and the router
/// picks it up automatically.
pub trait AgentScorer: Send + Sync {
    /// Return `(agent_id, score)` pairs for all agents that can handle
    /// `task_description`.  A higher score means a better match.
    fn score(&self, task_description: &str) -> Vec<(String, f64)>;
}

// ─────────────────────────────────────────────────────────────────────────────
// CapabilityMatchRouter
// ─────────────────────────────────────────────────────────────────────────────

/// Selects the highest-scoring agent for the task description carried in the
/// request envelope's metadata.
///
/// Falls back to `fallback_agent_id` when no agent scores at or above
/// `threshold`, logging a warning.  This avoids returning errors for
/// borderline requests while still routing them predictably.
pub struct CapabilityMatchRouter {
    scorer: Arc<dyn AgentScorer>,
    threshold: f64,
    fallback_agent_id: String,
    /// Metadata key in `RequestEnvelope` that holds the task description.
    task_key: String,
}

impl CapabilityMatchRouter {
    /// Create a new router.
    ///
    /// * `scorer` — any `AgentScorer` implementation (e.g. `CapabilityRegistry`)
    /// * `threshold` — minimum score for a match to be accepted (0.0–1.0)
    /// * `fallback_agent_id` — agent to use when no match meets the threshold
    /// * `task_key` — key in `RequestEnvelope.payload` holding the task string
    pub fn new(
        scorer: Arc<dyn AgentScorer>,
        threshold: f64,
        fallback_agent_id: impl Into<String>,
        task_key: impl Into<String>,
    ) -> Self {
        Self {
            scorer,
            threshold,
            fallback_agent_id: fallback_agent_id.into(),
            task_key: task_key.into(),
        }
    }
}

impl RoutingStrategy for CapabilityMatchRouter {
    fn select_agent(&self, envelope: &RequestEnvelope) -> Option<String> {
        // Extract task description from the payload using the configured key.
        let task = envelope
            .payload
            .get(&self.task_key)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if task.is_empty() {
            warn!(
                correlation_id = %envelope.correlation_id,
                "CapabilityMatchRouter: task key '{}' missing or empty, using fallback agent '{}'",
                self.task_key, self.fallback_agent_id
            );
            return Some(self.fallback_agent_id.clone());
        }

        let mut scores = self.scorer.score(task);
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((agent_id, score)) = scores.first() {
            if *score >= self.threshold {
                return Some(agent_id.clone());
            }
        }

        warn!(
            correlation_id = %envelope.correlation_id,
            task = %task,
            threshold = self.threshold,
            fallback = %self.fallback_agent_id,
            "CapabilityMatchRouter: no agent scored above threshold, using fallback"
        );
        Some(self.fallback_agent_id.clone())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RouterRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Maps route IDs to their assigned [`RoutingStrategy`].
///
/// The gateway dispatch layer looks up the strategy for the matched route ID
/// on each request, avoiding a match statement in the hot path.
pub struct RouterRegistry {
    strategies: HashMap<String, Arc<dyn RoutingStrategy>>,
}

impl RouterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
        }
    }

    /// Register a strategy for `route_id`.  Replaces any existing strategy.
    pub fn register(&mut self, route_id: impl Into<String>, strategy: Arc<dyn RoutingStrategy>) {
        self.strategies.insert(route_id.into(), strategy);
    }

    /// Look up the strategy for `route_id`.
    ///
    /// Returns `None` when no strategy has been registered for this route.
    pub fn get(&self, route_id: &str) -> Option<Arc<dyn RoutingStrategy>> {
        self.strategies.get(route_id).cloned()
    }

    /// Number of registered strategies.
    pub fn len(&self) -> usize {
        self.strategies.len()
    }

    /// Returns `true` if no strategies are registered.
    pub fn is_empty(&self) -> bool {
        self.strategies.is_empty()
    }
}

impl Default for RouterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use mofa_kernel::gateway::envelope::RequestEnvelope;

    fn envelope(task: &str) -> RequestEnvelope {
        RequestEnvelope::new(
            "route-1",
            json!({ "task": task }),
            IpAddr::from_str("127.0.0.1").unwrap(),
        )
    }

    fn empty_envelope() -> RequestEnvelope {
        RequestEnvelope::new("route-1", json!({}), IpAddr::from_str("127.0.0.1").unwrap())
    }

    // ── WeightedRoundRobinRouter ─────────────────────────────────────────────

    #[test]
    fn wrr_distributes_proportionally() {
        // agent-a weight 3, agent-b weight 1 → expect ~75% / ~25%
        let router = WeightedRoundRobinRouter::new(vec![
            ("agent-a", 3u32),
            ("agent-b", 1u32),
        ]);

        let n = 1000usize;
        let mut counts: HashMap<String, usize> = HashMap::new();
        for _ in 0..n {
            let agent = router.select_agent(&empty_envelope()).unwrap();
            *counts.entry(agent).or_insert(0) += 1;
        }

        let a_pct = *counts.get("agent-a").unwrap_or(&0) as f64 / n as f64;
        let b_pct = *counts.get("agent-b").unwrap_or(&0) as f64 / n as f64;

        // Allow 5% tolerance.
        assert!(
            (a_pct - 0.75).abs() < 0.05,
            "agent-a: expected ~75%, got {:.1}%",
            a_pct * 100.0
        );
        assert!(
            (b_pct - 0.25).abs() < 0.05,
            "agent-b: expected ~25%, got {:.1}%",
            b_pct * 100.0
        );
    }

    #[test]
    fn wrr_single_backend_always_selected() {
        let router = WeightedRoundRobinRouter::new(vec![("only-agent", 5u32)]);
        for _ in 0..10 {
            assert_eq!(
                router.select_agent(&empty_envelope()),
                Some("only-agent".to_string())
            );
        }
    }

    #[test]
    fn wrr_empty_backends_returns_none() {
        let router = WeightedRoundRobinRouter::new(Vec::<(String, u32)>::new());
        assert!(router.select_agent(&empty_envelope()).is_none());
    }

    #[test]
    fn wrr_update_backends_takes_effect() {
        let router = WeightedRoundRobinRouter::new(vec![("old-agent", 1u32)]);
        assert_eq!(
            router.select_agent(&empty_envelope()),
            Some("old-agent".to_string())
        );
        router.update_backends(vec![("new-agent", 1u32)]);
        // Counter carries over; new selection should be from new backends.
        let selected = router.select_agent(&empty_envelope()).unwrap();
        assert_eq!(selected, "new-agent");
    }

    #[test]
    fn wrr_zero_weight_backends_ignored() {
        let router = WeightedRoundRobinRouter::new(vec![("zero", 0u32), ("valid", 1u32)]);
        for _ in 0..5 {
            assert_eq!(
                router.select_agent(&empty_envelope()),
                Some("valid".to_string())
            );
        }
    }

    // ── CapabilityMatchRouter ────────────────────────────────────────────────

    struct MockScorer {
        scores: Vec<(String, f64)>,
    }

    impl AgentScorer for MockScorer {
        fn score(&self, _task: &str) -> Vec<(String, f64)> {
            self.scores.clone()
        }
    }

    fn scorer(scores: Vec<(&str, f64)>) -> Arc<dyn AgentScorer> {
        Arc::new(MockScorer {
            scores: scores
                .into_iter()
                .map(|(id, s)| (id.to_string(), s))
                .collect(),
        })
    }

    #[test]
    fn capability_match_selects_highest_scorer_above_threshold() {
        let router = CapabilityMatchRouter::new(
            scorer(vec![("agent-low", 0.4), ("agent-high", 0.9)]),
            0.7,
            "fallback",
            "task",
        );
        assert_eq!(
            router.select_agent(&envelope("summarise this")),
            Some("agent-high".to_string())
        );
    }

    #[test]
    fn capability_match_falls_back_when_no_score_above_threshold() {
        let router = CapabilityMatchRouter::new(
            scorer(vec![("agent-a", 0.3), ("agent-b", 0.5)]),
            0.7,
            "fallback-agent",
            "task",
        );
        assert_eq!(
            router.select_agent(&envelope("summarise this")),
            Some("fallback-agent".to_string())
        );
    }

    #[test]
    fn capability_match_falls_back_when_scorer_returns_empty() {
        let router = CapabilityMatchRouter::new(
            scorer(vec![]),
            0.7,
            "fallback-agent",
            "task",
        );
        assert_eq!(
            router.select_agent(&envelope("summarise this")),
            Some("fallback-agent".to_string())
        );
    }

    #[test]
    fn capability_match_falls_back_when_task_key_missing() {
        let router = CapabilityMatchRouter::new(
            scorer(vec![("agent-a", 0.9)]),
            0.7,
            "fallback-agent",
            "task",
        );
        // envelope has no "task" key
        assert_eq!(
            router.select_agent(&empty_envelope()),
            Some("fallback-agent".to_string())
        );
    }

    // ── RouterRegistry ───────────────────────────────────────────────────────

    #[test]
    fn router_registry_returns_correct_strategy() {
        let mut reg = RouterRegistry::new();
        let strategy: Arc<dyn RoutingStrategy> =
            Arc::new(WeightedRoundRobinRouter::new(vec![("agent-a", 1u32)]));
        reg.register("route-chat", Arc::clone(&strategy));

        assert!(reg.get("route-chat").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn router_registry_len_and_is_empty() {
        let mut reg = RouterRegistry::new();
        assert!(reg.is_empty());
        reg.register(
            "r1",
            Arc::new(WeightedRoundRobinRouter::new(vec![("a", 1u32)])),
        );
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
    }

    #[test]
    fn router_registry_replaces_existing_strategy() {
        let mut reg = RouterRegistry::new();
        reg.register(
            "r1",
            Arc::new(WeightedRoundRobinRouter::new(vec![("old", 1u32)])),
        );
        reg.register(
            "r1",
            Arc::new(WeightedRoundRobinRouter::new(vec![("new", 1u32)])),
        );
        assert_eq!(reg.len(), 1);
    }
}
