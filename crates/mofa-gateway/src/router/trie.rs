//! Trie-based path router implementing [`GatewayRouter`].
//!
//! Routes are stored in sorted-by-priority order.  Resolution performs a
//! linear scan with a simple path-template matcher that supports `{param}`
//! capture groups (axum / actix style).
//!
//! The linear scan is O(R × D) where R = number of routes and D = path depth,
//! which is entirely acceptable for gateway use-cases (route tables are
//! small) and trivially correct to verify.

use mofa_kernel::gateway::{
    GatewayError, GatewayRouter, HttpMethod, RouteConfig, RouteMatch,
};
use std::collections::HashMap;

/// [`GatewayRouter`] implementation using priority-sorted linear route lookup
/// with `{param}` template matching.
#[derive(Default)]
pub struct TrieRouter {
    /// Routes sorted by descending priority (highest first).
    routes: Vec<RouteConfig>,
}

impl TrieRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Match a concrete path against a template such as `/v1/models/{model_id}`.
    ///
    /// Returns `Some(params)` when the template matches, where `params` maps
    /// capture names to their extracted values.  Returns `None` on mismatch.
    fn match_path(template: &str, path: &str) -> Option<HashMap<String, String>> {
        let t_parts: Vec<&str> = template.trim_matches('/').split('/').collect();
        let p_parts: Vec<&str> = path.trim_matches('/').split('/').collect();

        if t_parts.len() != p_parts.len() {
            return None;
        }

        let mut params = HashMap::new();
        for (t, p) in t_parts.iter().zip(p_parts.iter()) {
            if t.starts_with('{') && t.ends_with('}') {
                // Extract the param name (strip braces).
                let name = &t[1..t.len() - 1];
                params.insert(name.to_string(), p.to_string());
            } else if *t != *p {
                return None;
            }
        }
        Some(params)
    }
}

impl GatewayRouter for TrieRouter {
    fn register(&mut self, route: RouteConfig) -> Result<(), GatewayError> {
        if self.routes.iter().any(|r| r.id == route.id) {
            return Err(GatewayError::DuplicateRoute(route.id));
        }
        // Insert maintaining descending priority order.
        let pos = self
            .routes
            .partition_point(|r| r.priority > route.priority);
        self.routes.insert(pos, route);
        Ok(())
    }

    fn resolve(&self, path: &str, method: &HttpMethod) -> Option<RouteMatch> {
        for route in &self.routes {
            // Method check: empty methods vec means "accept all".
            if !route.methods.is_empty() && !route.methods.contains(method) {
                continue;
            }
            if let Some(path_params) = Self::match_path(&route.path_pattern, path) {
                return Some(RouteMatch {
                    route_id: route.id.clone(),
                    backend_id: route.backend_id.clone(),
                    path_params,
                    timeout_ms: route.timeout_ms,
                });
            }
        }
        None
    }

    fn routes(&self) -> Vec<&RouteConfig> {
        self.routes.iter().collect()
    }

    fn deregister(&mut self, route_id: &str) -> Result<(), GatewayError> {
        let before = self.routes.len();
        self.routes.retain(|r| r.id != route_id);
        if self.routes.len() == before {
            return Err(GatewayError::RouteNotFound(route_id.to_string()));
        }
        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::gateway::RouteConfig;

    fn make_route(id: &str, pattern: &str, backend: &str) -> RouteConfig {
        RouteConfig::new(id, pattern, backend)
    }

    #[test]
    fn exact_path_matches() {
        let mut router = TrieRouter::new();
        router
            .register(make_route("health", "/health", "svc"))
            .unwrap();
        let m = router.resolve("/health", &HttpMethod::Get).unwrap();
        assert_eq!(m.route_id, "health");
        assert!(m.path_params.is_empty());
    }

    #[test]
    fn param_path_extracts_value() {
        let mut router = TrieRouter::new();
        router
            .register(make_route("model", "/v1/models/{model_id}", "openai"))
            .unwrap();
        let m = router
            .resolve("/v1/models/gpt-4", &HttpMethod::Get)
            .unwrap();
        assert_eq!(m.path_params.get("model_id").unwrap(), "gpt-4");
    }

    #[test]
    fn no_match_returns_none() {
        let router = TrieRouter::new();
        assert!(router
            .resolve("/nonexistent", &HttpMethod::Get)
            .is_none());
    }

    #[test]
    fn method_filter_respected() {
        let mut router = TrieRouter::new();
        let route = make_route("chat", "/v1/chat", "openai")
            .with_methods(vec![HttpMethod::Post]);
        router.register(route).unwrap();
        // GET should not match
        assert!(router.resolve("/v1/chat", &HttpMethod::Get).is_none());
        // POST should match
        assert!(router.resolve("/v1/chat", &HttpMethod::Post).is_some());
    }

    #[test]
    fn higher_priority_wins() {
        let mut router = TrieRouter::new();
        router
            .register(make_route("low", "/v1/chat", "backend-low").with_priority(0))
            .unwrap();
        router
            .register(make_route("high", "/v1/chat", "backend-high").with_priority(10))
            .unwrap();
        let m = router.resolve("/v1/chat", &HttpMethod::Post).unwrap();
        assert_eq!(m.route_id, "high");
    }

    #[test]
    fn duplicate_route_id_rejected() {
        let mut router = TrieRouter::new();
        router.register(make_route("r1", "/a", "b")).unwrap();
        let err = router.register(make_route("r1", "/b", "b")).unwrap_err();
        assert!(matches!(err, GatewayError::DuplicateRoute(ref id) if id == "r1"));
    }

    #[test]
    fn deregister_removes_route() {
        let mut router = TrieRouter::new();
        router.register(make_route("r1", "/a", "b")).unwrap();
        router.deregister("r1").unwrap();
        assert!(router.resolve("/a", &HttpMethod::Get).is_none());
    }

    #[test]
    fn deregister_unknown_id_returns_error() {
        let mut router = TrieRouter::new();
        assert!(router.deregister("ghost").is_err());
    }
}
