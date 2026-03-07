//! Shared state for the admin API server.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use mofa_kernel::gateway::route::{GatewayRoute, HttpMethod};

/// Per-route runtime statistics.
#[derive(Debug, Default)]
pub struct RouteStats {
    pub requests_served: AtomicU64,
}

impl RouteStats {
    pub fn increment(&self) {
        self.requests_served.fetch_add(1, Ordering::Relaxed);
    }

    pub fn count(&self) -> u64 {
        self.requests_served.load(Ordering::Relaxed)
    }
}

/// Shared state for the admin API.
///
/// Held behind an `Arc` and shared across all admin request handlers.
#[derive(Clone)]
pub struct AdminState {
    inner: Arc<AdminStateInner>,
}

struct AdminStateInner {
    /// Live route registry: route_id → GatewayRoute.
    routes: RwLock<HashMap<String, GatewayRoute>>,
    /// Per-route request counters.
    stats: RwLock<HashMap<String, Arc<RouteStats>>>,
    /// Global request counter.
    total_requests: AtomicU64,
    /// Server start time for uptime computation.
    started_at: Instant,
    /// Whether the hot-reload watcher is active.
    hot_reload_active: AtomicBool,
    /// Admin API key checked on every request.
    admin_key: String,
}

impl AdminState {
    /// Create a new admin state with the given admin key.
    pub fn new(admin_key: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(AdminStateInner {
                routes: RwLock::new(HashMap::new()),
                stats: RwLock::new(HashMap::new()),
                total_requests: AtomicU64::new(0),
                started_at: Instant::now(),
                hot_reload_active: AtomicBool::new(false),
                admin_key: admin_key.into(),
            }),
        }
    }

    // ── Auth ─────────────────────────────────────────────────────────────────

    /// Returns `true` if `key` matches the configured admin key.
    pub fn verify_key(&self, key: &str) -> bool {
        self.inner.admin_key == key
    }

    // ── Route registry ───────────────────────────────────────────────────────

    /// Register a new route. Returns `false` if the ID already exists.
    pub fn register_route(&self, route: GatewayRoute) -> bool {
        let mut routes = self.inner.routes.write().unwrap();
        if routes.contains_key(&route.id) {
            return false;
        }
        let mut stats = self.inner.stats.write().unwrap();
        stats.insert(route.id.clone(), Arc::new(RouteStats::default()));
        routes.insert(route.id.clone(), route);
        true
    }

    /// Deregister a route by ID. Returns `false` if not found.
    pub fn deregister_route(&self, route_id: &str) -> bool {
        let mut routes = self.inner.routes.write().unwrap();
        if routes.remove(route_id).is_none() {
            return false;
        }
        self.inner.stats.write().unwrap().remove(route_id);
        true
    }

    /// Toggle the `enabled` flag on a registered route.
    /// Returns `false` if the route ID does not exist.
    pub fn set_route_enabled(&self, route_id: &str, enabled: bool) -> bool {
        let mut routes = self.inner.routes.write().unwrap();
        match routes.get_mut(route_id) {
            Some(r) => {
                r.enabled = enabled;
                true
            }
            None => false,
        }
    }

    /// Return a snapshot of all registered routes and their request counts.
    pub fn list_routes(&self) -> Vec<(GatewayRoute, u64)> {
        let routes = self.inner.routes.read().unwrap();
        let stats = self.inner.stats.read().unwrap();
        routes
            .values()
            .map(|r| {
                let count = stats.get(&r.id).map(|s| s.count()).unwrap_or(0);
                (r.clone(), count)
            })
            .collect()
    }

    /// Increment the request counter for a route and the global total.
    pub fn record_request(&self, route_id: &str) {
        self.inner.total_requests.fetch_add(1, Ordering::Relaxed);
        if let Some(s) = self.inner.stats.read().unwrap().get(route_id) {
            s.increment();
        }
    }

    // ── Health ───────────────────────────────────────────────────────────────

    /// Uptime in whole seconds since the admin state was created.
    pub fn uptime_secs(&self) -> u64 {
        self.inner.started_at.elapsed().as_secs()
    }

    /// Total requests served across all routes.
    pub fn total_requests(&self) -> u64 {
        self.inner.total_requests.load(Ordering::Relaxed)
    }

    /// Number of registered routes.
    pub fn routes_count(&self) -> usize {
        self.inner.routes.read().unwrap().len()
    }

    /// Mark the hot-reload watcher as active or inactive.
    pub fn set_hot_reload_active(&self, active: bool) {
        self.inner.hot_reload_active.store(active, Ordering::Relaxed);
    }

    /// Whether the hot-reload watcher is active.
    pub fn hot_reload_active(&self) -> bool {
        self.inner.hot_reload_active.load(Ordering::Relaxed)
    }
}

/// Parse an HTTP method string into `HttpMethod`, case-insensitive.
/// Falls back to `HttpMethod::Post` for unknown strings.
pub fn parse_method(s: &str) -> HttpMethod {
    HttpMethod::from_str_ci(s).unwrap_or(HttpMethod::Post)
}
