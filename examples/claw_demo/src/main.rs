//! Claw Demo — MoFA Gateway Closed-Loop Showcase
//!
//! Demonstrates the gateway processing pipeline using **real** types from
//! `mofa-kernel`, `mofa-gateway`, and `mofa-foundation` wherever they exist.
//! Inline mocks are used only for components that have no trait or concrete
//! type in the current codebase — each mock is annotated with a `// MOCK:`
//! comment explaining why.
//!
//! # Layers
//!
//! 1. `GatewayRequest` + `HttpMethod`       — real (mofa-kernel::gateway)
//! 2. `RouteRegistry` + `GatewayRoute`      — trait real, inline impl
//! 3. Filter chain                           — MOCK (no trait exists)
//! 4. `HealthChecker` + `NodeId`             — real (mofa-gateway)
//! 5. `LoadBalancer`                         — real (mofa-gateway)
//! 6. `CapabilityRegistry` + `AgentManifest` — real (mofa-foundation + mofa-kernel)
//! 7. Backend response                       — MOCK (no API key)
//! 8. `MockHandoffRecord`                    — MOCK (pending PR #997)
//! 9. Closed-loop report
//!
//! **Note:** This demo does not currently integrate with `mofa-runtime` or execute a real sub-agent. The "handoff" is a local mock. To fully demonstrate a runtime/sub-agent path, further integration is required.
//!
//! # Running
//!
//! ```bash
//! cargo run -p claw_demo
//! ```

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use uuid::Uuid;

// ── Real imports from mofa-kernel gateway module ────────────────────────────
use mofa_kernel::gateway::{
    GatewayRequest, GatewayRoute, HttpMethod, RegistryError, RouteRegistry,
};

// ── Real imports from mofa-gateway ──────────────────────────────────────────
use mofa_gateway::gateway::{HealthChecker, LoadBalancer};
use mofa_gateway::{LoadBalancingAlgorithm, NodeId, NodeStatus};

// ── Real imports from mofa-foundation ───────────────────────────────────────
use mofa_foundation::CapabilityRegistry;
use mofa_kernel::agent::manifest::AgentManifest;

// ─────────────────────────────────────────────────────────────────────────────
// Layer 2 — SimpleRouteRegistry
//
// Inline impl — no concrete RouteRegistry in mofa-foundation yet.
// The kernel defines the trait; we provide a minimal HashMap-backed
// implementation just for this demo.
// ─────────────────────────────────────────────────────────────────────────────

struct SimpleRouteRegistry {
    routes: HashMap<String, GatewayRoute>,
}

impl SimpleRouteRegistry {
    fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Resolve an incoming (path, method) pair to the first matching active
    /// route's `agent_id`.
    fn resolve(&self, path: &str, method: &HttpMethod) -> Option<String> {
        self.list_active()
            .into_iter()
            .find(|r| r.path_pattern == path && r.method == *method)
            .map(|r| r.agent_id.clone())
    }
}

impl RouteRegistry for SimpleRouteRegistry {
    fn register(&mut self, route: GatewayRoute) -> Result<(), RegistryError> {
        // Validate the route and wrap validation errors as RegistryError::InvalidRoute for consistency.
        if let Err(err) = route.validate() {
            return Err(RegistryError::InvalidRoute(err.to_string()));
        }
        if self.routes.contains_key(&route.id) {
            return Err(RegistryError::DuplicateRouteId(route.id.clone()));
        }
        // Enforce deterministic dispatch: prevent multiple routes with the same
        // (path_pattern, method, priority) triple.
        let conflict_id = self
            .routes
            .values()
            .find(|existing| {
                existing.path_pattern == route.path_pattern
                    && existing.method == route.method
                    && existing.priority == route.priority
            })
            .map(|existing| existing.id.clone());
        if let Some(existing_id) = conflict_id {
            return Err(RegistryError::ConflictingRoutes(
                route.id.clone(),
                existing_id,
            ));
        }
        self.routes.insert(route.id.clone(), route);
        Ok(())
    }

    fn deregister(&mut self, route_id: &str) -> Result<(), RegistryError> {
        self.routes
            .remove(route_id)
            .map(|_| ())
            .ok_or_else(|| RegistryError::RouteNotFound(route_id.to_owned()))
    }

    fn lookup(&self, route_id: &str) -> Option<&GatewayRoute> {
        self.routes.get(route_id)
    }

    fn list_active(&self) -> Vec<&GatewayRoute> {
        let mut active: Vec<&GatewayRoute> =
            self.routes.values().filter(|r| r.enabled).collect();
        active.sort_by(|a, b| b.priority.cmp(&a.priority));
        active
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Layer 3 — Filter chain
//
// MOCK: No GatewayFilter/FilterPipeline trait in mofa-kernel or mofa-gateway.
// The three filter steps are simulated as plain functions.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterResult {
    Pass,
    #[allow(dead_code)]
    Reject,
}

impl std::fmt::Display for FilterResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pass => f.write_str("pass"),
            Self::Reject => f.write_str("reject"),
        }
    }
}

fn auth_filter(_req: &GatewayRequest) -> FilterResult {
    FilterResult::Pass
}

fn rate_limit_filter(_req: &GatewayRequest) -> FilterResult {
    FilterResult::Pass
}

fn logging_filter(req: &GatewayRequest) -> FilterResult {
    tracing::debug!(
        request_id = %req.id,
        method = %req.method,
        path = %req.path,
        "logging filter: request received"
    );
    FilterResult::Pass
}

// ─────────────────────────────────────────────────────────────────────────────
// Layer 8 — Sub-agent handoff
//
// MOCK: Pending PR #997 merge — HandoffProtocol coordination traits
// not yet in main branch.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct MockHandoffRecord {
    handoff_id: String,
    from_agent: String,
    to_agent: String,
    #[allow(dead_code)]
    payload: String,
}

impl MockHandoffRecord {
    fn new(from: &str, to: &str, payload: &str) -> Self {
        Self {
            handoff_id: Uuid::new_v4().to_string(),
            from_agent: from.to_owned(),
            to_agent: to.to_owned(),
            payload: payload.to_owned(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // ── Layer 1 — GatewayRequest (real: mofa-kernel::gateway) ───────────────
    println!("[Claw Demo] Starting closed loop...");

    let request = GatewayRequest::new(
        Uuid::new_v4().to_string(),
        "/api/chat",
        HttpMethod::Post,
    );

    // ── Layer 2 — RouteRegistry (real trait, inline impl) ───────────────────
    let mut router = SimpleRouteRegistry::new();
    let route = GatewayRoute::new("chat-route", "openai", "/api/chat", HttpMethod::Post);
    router
        .register(route)
        .expect("route registration must succeed");
    println!("[Claw Demo] Route registered: POST /api/chat -> openai");

    let backend_name = router
        .resolve(&request.path, &request.method)
        .expect("route must resolve: POST /api/chat");
    let resolved = format!("{backend_name}-gpt4");
    println!("[Claw Demo] Route resolved: backend={resolved}");

    // ── Layer 3 — Filter chain (MOCK) ───────────────────────────────────────
    // MOCK: No GatewayFilter/FilterPipeline trait in mofa-kernel or mofa-gateway.
    let auth = auth_filter(&request);
    let rate = rate_limit_filter(&request);
    let log = logging_filter(&request);

    assert_eq!(auth, FilterResult::Pass, "AUTH filter must pass");
    assert_eq!(rate, FilterResult::Pass, "RATE_LIMIT filter must pass");
    assert_eq!(log, FilterResult::Pass, "LOGGING filter must pass");

    println!("[Claw Demo] Filter chain: AUTH({auth}) RATE_LIMIT({rate}) LOGGING({log})");

    // ── Layer 4 — HealthChecker (real: mofa-gateway) ────────────────────────
    let health_checker = HealthChecker::new(
        Duration::from_secs(5),
        Duration::from_secs(1),
        3,
    );
    let openai_node = NodeId::new("openai");
    health_checker.register_node(openai_node.clone()).await;

    // check_node with no registered address assumes healthy (backward-compat
    // path in health_checker.rs) and promotes the node to NodeStatus::Healthy.
    let _healthy = health_checker
        .check_node(&openai_node)
        .await
        .expect("health check must succeed");

    let status = health_checker
        .get_status(&openai_node)
        .await
        .expect("node must be registered");
    assert_eq!(status, NodeStatus::Healthy, "openai node must be healthy");
    println!("[Claw Demo] Health check: openai backend healthy");

    // ── Layer 5 — LoadBalancer (real: mofa-gateway) ─────────────────────────
    let lb = LoadBalancer::new(LoadBalancingAlgorithm::RoundRobin);
    let endpoint_node = NodeId::new("https://api.openai.com");
    lb.add_node(endpoint_node).await;

    let selected = lb
        .select_node()
        .await
        .expect("load balancer must not error")
        .expect("load balancer must have at least one node");
    println!("[Claw Demo] Load balancer selected: endpoint={selected}");

    // ── Layer 6 — CapabilityRegistry (real: mofa-foundation) ────────────────
    let mut cap_registry = CapabilityRegistry::new();
    let manifest = AgentManifest::builder("openai-backend", "OpenAI Backend")
        .description("Chat completion backend")
        .build();
    cap_registry.register(manifest);

    let _descriptor = cap_registry
        .find_by_id("openai-backend")
        .expect("manifest must be registered");
    println!("[Claw Demo] Registry lookup: descriptor found");

    // ── Layer 7 — Backend response (MOCK) ───────────────────────────────────
    // Replace with real OpenAIProvider call when OPENAI_API_KEY is set:
    //
    //   use mofa_sdk::llm::{openai_from_env, LLMClient};
    //   let provider = openai_from_env().expect("OPENAI_API_KEY");
    //   let client   = LLMClient::new(Arc::new(provider));
    //   let resp     = client.ask("...").await?;
    let backend_response = "Mock LLM response: Analysis complete";
    println!("[Claw Demo] Backend response received");

    // ── Layer 8 — SubAgent handoff (MOCK) ───────────────────────────────────
    // MOCK: Pending PR #997 merge — HandoffProtocol coordination traits
    // not yet in main branch.
    let handoff = MockHandoffRecord::new(
        "gateway-agent",
        "openai-agent",
        backend_response,
    );
    println!("[Claw Demo] SubAgent handoff created: id={}", handoff.handoff_id);
    tracing::debug!(
        from = %handoff.from_agent,
        to   = %handoff.to_agent,
        "handoff dispatched"
    );

    // ── Layer 9 — Closed loop complete ──────────────────────────────────────
    println!("[Claw Demo] Closed loop complete \u{2713}");

    Ok(())
}
