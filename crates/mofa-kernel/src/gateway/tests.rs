//! Unit tests for GatewayRoute, RouteRegistry, and RoutingContext.
//!
//! Uses an `InMemoryRouteRegistry` defined here for testing purposes only —
//! the concrete registry implementation lives in `mofa-foundation`.

use std::collections::HashMap;

use super::error::RegistryError;
use super::route::{GatewayRoute, HttpMethod, RouteRegistry, RoutingContext};

// ─────────────────────────────────────────────────────────────────────────────
// Minimal in-test registry implementation
// ─────────────────────────────────────────────────────────────────────────────

struct InMemoryRouteRegistry {
    routes: HashMap<String, GatewayRoute>,
}

impl InMemoryRouteRegistry {
    fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }
}

impl RouteRegistry for InMemoryRouteRegistry {
    fn register(&mut self, route: GatewayRoute) -> Result<(), RegistryError> {
        route
            .validate()
            .map_err(|e| RegistryError::InvalidRoute(e.to_string()))?;

        if self.routes.contains_key(&route.id) {
            return Err(RegistryError::DuplicateRouteId(route.id.clone()));
        }

        // Conflict: same (path_pattern, method, priority) as an existing route.
        for existing in self.routes.values() {
            if existing.path_pattern == route.path_pattern
                && existing.method == route.method
                && existing.priority == route.priority
            {
                return Err(RegistryError::ConflictingRoutes(
                    route.id.clone(),
                    existing.id.clone(),
                ));
            }
        }

        self.routes.insert(route.id.clone(), route);
        Ok(())
    }

    fn deregister(&mut self, route_id: &str) -> Result<(), RegistryError> {
        if self.routes.remove(route_id).is_none() {
            return Err(RegistryError::RouteNotFound(route_id.to_string()));
        }
        Ok(())
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
// GatewayRoute tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn gateway_route_new_defaults() {
    let route = GatewayRoute::new("r1", "agent-a", "/agents/summarizer", HttpMethod::Post);
    assert_eq!(route.id, "r1");
    assert_eq!(route.agent_id, "agent-a");
    assert_eq!(route.path_pattern, "/agents/summarizer");
    assert_eq!(route.method, HttpMethod::Post);
    assert_eq!(route.priority, 0);
    assert!(route.enabled);
}

#[test]
fn gateway_route_builder() {
    let route = GatewayRoute::new("r1", "agent-a", "/path", HttpMethod::Get)
        .with_priority(10)
        .disabled();
    assert_eq!(route.priority, 10);
    assert!(!route.enabled);
}

#[test]
fn gateway_route_validate_empty_id() {
    let route = GatewayRoute::new("", "agent-a", "/path", HttpMethod::Get);
    assert!(matches!(route.validate(), Err(RegistryError::EmptyRouteId)));
}

#[test]
fn gateway_route_validate_empty_agent_id() {
    let route = GatewayRoute::new("r1", "", "/path", HttpMethod::Get);
    assert!(matches!(route.validate(), Err(RegistryError::EmptyAgentId)));
}

#[test]
fn gateway_route_validate_invalid_path() {
    let route = GatewayRoute::new("r1", "agent-a", "no-leading-slash", HttpMethod::Get);
    assert!(matches!(
        route.validate(),
        Err(RegistryError::InvalidPathPattern(_, _))
    ));
}

// ─────────────────────────────────────────────────────────────────────────────
// RouteRegistry tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn register_and_lookup() {
    let mut reg = InMemoryRouteRegistry::new();
    let route = GatewayRoute::new("r1", "agent-a", "/agents/summarizer", HttpMethod::Post);
    reg.register(route).unwrap();

    let found = reg.lookup("r1").unwrap();
    assert_eq!(found.agent_id, "agent-a");
}

#[test]
fn lookup_missing_returns_none() {
    let reg = InMemoryRouteRegistry::new();
    assert!(reg.lookup("does-not-exist").is_none());
}

#[test]
fn register_duplicate_id_is_error() {
    let mut reg = InMemoryRouteRegistry::new();
    let r1 = GatewayRoute::new("r1", "agent-a", "/path", HttpMethod::Get);
    let r2 = GatewayRoute::new("r1", "agent-b", "/other", HttpMethod::Post);
    reg.register(r1).unwrap();
    assert!(matches!(
        reg.register(r2),
        Err(RegistryError::DuplicateRouteId(_))
    ));
}

#[test]
fn deregister_removes_route() {
    let mut reg = InMemoryRouteRegistry::new();
    reg.register(GatewayRoute::new("r1", "agent-a", "/path", HttpMethod::Get))
        .unwrap();
    reg.deregister("r1").unwrap();
    assert!(reg.lookup("r1").is_none());
}

#[test]
fn deregister_missing_is_error() {
    let mut reg = InMemoryRouteRegistry::new();
    assert!(matches!(
        reg.deregister("ghost"),
        Err(RegistryError::RouteNotFound(_))
    ));
}

#[test]
fn list_active_excludes_disabled_routes() {
    let mut reg = InMemoryRouteRegistry::new();
    reg.register(GatewayRoute::new("r1", "agent-a", "/active", HttpMethod::Get))
        .unwrap();
    reg.register(
        GatewayRoute::new("r2", "agent-b", "/disabled", HttpMethod::Post).disabled(),
    )
    .unwrap();

    let active = reg.list_active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "r1");
}

#[test]
fn list_active_sorted_by_descending_priority() {
    let mut reg = InMemoryRouteRegistry::new();
    reg.register(
        GatewayRoute::new("low", "agent-a", "/low", HttpMethod::Get).with_priority(1),
    )
    .unwrap();
    reg.register(
        GatewayRoute::new("high", "agent-b", "/high", HttpMethod::Post).with_priority(10),
    )
    .unwrap();
    reg.register(
        GatewayRoute::new("mid", "agent-c", "/mid", HttpMethod::Put).with_priority(5),
    )
    .unwrap();

    let active = reg.list_active();
    assert_eq!(active[0].id, "high");
    assert_eq!(active[1].id, "mid");
    assert_eq!(active[2].id, "low");
}

// ─────────────────────────────────────────────────────────────────────────────
// Conflict detection tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn conflict_same_path_method_and_equal_priority() {
    let mut reg = InMemoryRouteRegistry::new();
    reg.register(GatewayRoute::new("r1", "agent-a", "/v1/chat", HttpMethod::Post))
        .unwrap();
    // Same path, method, and priority (0) as r1 — must be rejected.
    let result = reg.register(GatewayRoute::new("r2", "agent-b", "/v1/chat", HttpMethod::Post));
    assert!(
        matches!(result, Err(RegistryError::ConflictingRoutes(ref new, ref existing))
            if new == "r2" && existing == "r1"),
        "expected ConflictingRoutes(r2, r1), got {result:?}"
    );
}

#[test]
fn no_conflict_same_path_method_different_priority() {
    let mut reg = InMemoryRouteRegistry::new();
    reg.register(GatewayRoute::new("r1", "agent-a", "/v1/chat", HttpMethod::Post))
        .unwrap();
    // Different priority — should succeed.
    reg.register(
        GatewayRoute::new("r2", "agent-b", "/v1/chat", HttpMethod::Post).with_priority(1),
    )
    .unwrap();
    assert!(reg.lookup("r2").is_some());
}

#[test]
fn no_conflict_same_path_different_method() {
    let mut reg = InMemoryRouteRegistry::new();
    reg.register(GatewayRoute::new("r1", "agent-a", "/v1/chat", HttpMethod::Post))
        .unwrap();
    reg.register(GatewayRoute::new("r2", "agent-b", "/v1/chat", HttpMethod::Get))
        .unwrap();
    assert_eq!(reg.list_active().len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// RoutingContext tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn routing_context_new() {
    let ctx = RoutingContext::new("/agents/summarizer", HttpMethod::Post, "req-abc-123");
    assert_eq!(ctx.path, "/agents/summarizer");
    assert_eq!(ctx.method, HttpMethod::Post);
    assert_eq!(ctx.correlation_id, "req-abc-123");
    assert!(ctx.headers.is_empty());
}

#[test]
fn routing_context_headers_are_lowercased() {
    let ctx = RoutingContext::new("/path", HttpMethod::Get, "cid-1")
        .with_header("Content-Type", "application/json")
        .with_header("X-Api-Key", "secret");

    assert_eq!(
        ctx.headers.get("content-type"),
        Some(&"application/json".to_string())
    );
    assert_eq!(
        ctx.headers.get("x-api-key"),
        Some(&"secret".to_string())
    );
}

#[test]
fn http_method_from_str_ci() {
    assert_eq!(HttpMethod::from_str_ci("get"), Some(HttpMethod::Get));
    assert_eq!(HttpMethod::from_str_ci("POST"), Some(HttpMethod::Post));
    assert_eq!(HttpMethod::from_str_ci("pAtCh"), Some(HttpMethod::Patch));
    assert_eq!(HttpMethod::from_str_ci("UNKNOWN"), None);
}
