//! TOML-based gateway configuration schema with atomic hot-reload.
//!
//! # Usage
//!
//! ```toml
//! # gateway.toml
//! default_auth_provider = "api_key"
//!
//! [[routes]]
//! id            = "chat"
//! path_pattern  = "/v1/chat"
//! method        = "POST"
//! agent_id      = "agent-chat"
//! strategy      = "weighted_round_robin"
//! timeout_ms    = 5000
//! rate_limit    = "default"
//!
//! [rate_limit_profiles.default]
//! capacity     = 100
//! refill_rate  = 10
//! strategy     = "PerClient"
//! ```
//!
//! Load once:
//! ```rust,ignore
//! let cfg = GatewayConfigLoader::load_from_file(Path::new("gateway.toml"))?;
//! ```
//!
//! Hot-reload:
//! ```rust,ignore
//! let (tx, rx) = tokio::sync::mpsc::channel(8);
//! GatewayConfigLoader::watch(Path::new("gateway.toml"), tx)?;
//! // rx receives a new GatewayConfig on every debounced file change.
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tracing::{error, info, warn};

use super::rate_limiter::KeyStrategy;
use mofa_kernel::gateway::route::{GatewayRoute, HttpMethod};

// ─────────────────────────────────────────────────────────────────────────────
// Config schema
// ─────────────────────────────────────────────────────────────────────────────

/// Rate-limit profile referenced by name from route definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimitProfile {
    /// Burst capacity (max tokens).
    pub capacity: u32,
    /// Tokens added per second.
    pub refill_rate: u32,
    /// Keying strategy.
    pub strategy: KeyStrategy,
}

impl Default for RateLimitProfile {
    fn default() -> Self {
        Self {
            capacity: 100,
            refill_rate: 10,
            strategy: KeyStrategy::PerClient,
        }
    }
}

/// A single route definition inside the TOML config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Unique stable route identifier.
    pub id: String,
    /// URL path pattern (must start with `/`).
    pub path_pattern: String,
    /// HTTP method string, e.g. `"POST"`.
    pub method: String,
    /// Target agent ID.
    pub agent_id: String,
    /// Routing strategy name (`"weighted_round_robin"` or `"capability_match"`).
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// Per-route timeout in milliseconds. `0` means no timeout.
    #[serde(default)]
    pub timeout_ms: u64,
    /// Name of a rate limit profile defined in `rate_limit_profiles`.
    /// `None` means no rate limiting for this route.
    #[serde(default)]
    pub rate_limit: Option<String>,
    /// Whether the route is enabled. Defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_strategy() -> String {
    "weighted_round_robin".to_string()
}

fn default_enabled() -> bool {
    true
}

impl RouteConfig {
    /// Convert to a [`GatewayRoute`] kernel type.
    ///
    /// Returns `None` when `method` is not a recognised HTTP method string.
    pub fn to_gateway_route(&self) -> Option<GatewayRoute> {
        let method = HttpMethod::from_str_ci(&self.method)?;
        Some(
            GatewayRoute::new(
                self.id.clone(),
                self.agent_id.clone(),
                self.path_pattern.clone(),
                method,
            )
            .with_priority(0),
        )
    }
}

/// Top-level gateway configuration parsed from TOML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Name of the default auth provider.
    #[serde(default)]
    pub default_auth_provider: Option<String>,

    /// Route definitions.
    #[serde(default)]
    pub routes: Vec<RouteConfig>,

    /// Named rate-limit profiles routes can reference.
    #[serde(default)]
    pub rate_limit_profiles: HashMap<String, RateLimitProfile>,
}

impl GatewayConfig {
    /// Return the rate-limit profile for `name`, or the default profile if
    /// `name` is not found.
    pub fn rate_limit_profile(&self, name: &str) -> RateLimitProfile {
        self.rate_limit_profiles
            .get(name)
            .cloned()
            .unwrap_or_default()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config diff
// ─────────────────────────────────────────────────────────────────────────────

/// The diff between a previous config and a new one.
#[derive(Debug, Default, PartialEq)]
pub struct ConfigDiff {
    /// Route IDs that are new in the new config.
    pub added: Vec<String>,
    /// Route IDs that were removed in the new config.
    pub removed: Vec<String>,
    /// Route IDs whose definition changed between the two configs.
    pub modified: Vec<String>,
}

impl ConfigDiff {
    /// Returns `true` when there are no changes.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

/// Compute the diff between `old` and `new` configs.
pub fn diff_configs(old: &GatewayConfig, new: &GatewayConfig) -> ConfigDiff {
    let old_map: HashMap<&str, &RouteConfig> =
        old.routes.iter().map(|r| (r.id.as_str(), r)).collect();
    let new_map: HashMap<&str, &RouteConfig> =
        new.routes.iter().map(|r| (r.id.as_str(), r)).collect();

    let mut result = ConfigDiff::default();

    for (id, new_route) in &new_map {
        match old_map.get(id) {
            None => result.added.push(id.to_string()),
            Some(old_route) => {
                if old_route != new_route {
                    result.modified.push(id.to_string());
                }
            }
        }
    }

    for id in old_map.keys() {
        if !new_map.contains_key(id) {
            result.removed.push(id.to_string());
        }
    }

    result.added.sort();
    result.removed.sort();
    result.modified.sort();
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayConfigLoader
// ─────────────────────────────────────────────────────────────────────────────

/// Loads and watches a TOML gateway configuration file.
pub struct GatewayConfigLoader;

impl GatewayConfigLoader {
    /// Parse a [`GatewayConfig`] from a TOML file at `path`.
    pub fn load_from_file(path: &Path) -> Result<GatewayConfig, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(path.display().to_string(), e.to_string()))?;
        toml::from_str(&content)
            .map_err(|e| ConfigError::Parse(path.display().to_string(), e.to_string()))
    }

    /// Watch `path` for changes and emit a new [`GatewayConfig`] on `tx`
    /// after a 200 ms debounce window.
    ///
    /// The watcher runs on a background OS thread spawned by the `notify`
    /// crate and is kept alive as long as the returned `_guard` value is not
    /// dropped.  Drop the guard to stop watching.
    pub fn watch(
        path: &Path,
        tx: Sender<GatewayConfig>,
    ) -> Result<RecommendedWatcher, ConfigError> {
        let path_buf = path.to_path_buf();
        let last_event: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let debounce = Duration::from_millis(200);

        let watcher_tx = tx.clone();
        let watcher_path = path_buf.clone();
        let watcher_last = Arc::clone(&last_event);

        let mut watcher = notify::recommended_watcher(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    let is_write = matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_)
                    );
                    if !is_write {
                        return;
                    }

                    // Debounce: record the event time and only reload after the
                    // settling window.
                    let now = Instant::now();
                    {
                        let mut last = watcher_last.lock().unwrap();
                        *last = Some(now);
                    }

                    // Sleep for the debounce window, then check whether a
                    // newer event has arrived.
                    let sleep_last = Arc::clone(&watcher_last);
                    let reload_path = watcher_path.clone();
                    let reload_tx = watcher_tx.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(debounce);
                        let last = sleep_last.lock().unwrap();
                        // If another event arrived after ours, skip this reload.
                        if last.map(|t| t > now).unwrap_or(false) {
                            return;
                        }
                        drop(last);

                        match GatewayConfigLoader::load_from_file(&reload_path) {
                            Ok(cfg) => {
                                info!("gateway config reloaded from {:?}", reload_path);
                                if let Err(e) = reload_tx.blocking_send(cfg) {
                                    warn!("gateway config channel closed: {e}");
                                }
                            }
                            Err(e) => {
                                error!("failed to reload gateway config: {e}");
                            }
                        }
                    });
                }
                Err(e) => error!("file watcher error: {e}"),
            },
        )
        .map_err(|e| ConfigError::Watcher(e.to_string()))?;

        watcher
            .watch(&path_buf, RecursiveMode::NonRecursive)
            .map_err(|e| ConfigError::Watcher(e.to_string()))?;

        Ok(watcher)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConfigError
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that can occur when loading or watching the gateway config.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("failed to read '{0}': {1}")]
    Io(String, String),
    #[error("failed to parse '{0}': {1}")]
    Parse(String, String),
    #[error("file watcher error: {0}")]
    Watcher(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const MINIMAL_TOML: &str = r#"
default_auth_provider = "api_key"

[[routes]]
id           = "chat"
path_pattern = "/v1/chat"
method       = "POST"
agent_id     = "agent-chat"
strategy     = "weighted_round_robin"
timeout_ms   = 5000
rate_limit   = "default"

[[routes]]
id           = "search"
path_pattern = "/v1/search"
method       = "GET"
agent_id     = "agent-search"

[rate_limit_profiles.default]
capacity    = 100
refill_rate = 10
strategy    = "PerClient"
"#;

    fn parse(toml: &str) -> GatewayConfig {
        toml::from_str(toml).expect("valid TOML")
    }

    // ── Parse round-trip ─────────────────────────────────────────────────────

    #[test]
    fn parse_round_trip() {
        let cfg = parse(MINIMAL_TOML);
        let re_serialized = toml::to_string(&cfg).unwrap();
        let re_parsed: GatewayConfig = toml::from_str(&re_serialized).unwrap();
        assert_eq!(cfg, re_parsed);
    }

    #[test]
    fn parse_routes_count() {
        let cfg = parse(MINIMAL_TOML);
        assert_eq!(cfg.routes.len(), 2);
    }

    #[test]
    fn parse_route_fields() {
        let cfg = parse(MINIMAL_TOML);
        let chat = cfg.routes.iter().find(|r| r.id == "chat").unwrap();
        assert_eq!(chat.path_pattern, "/v1/chat");
        assert_eq!(chat.method, "POST");
        assert_eq!(chat.agent_id, "agent-chat");
        assert_eq!(chat.strategy, "weighted_round_robin");
        assert_eq!(chat.timeout_ms, 5000);
        assert_eq!(chat.rate_limit, Some("default".to_string()));
        assert!(chat.enabled);
    }

    #[test]
    fn parse_rate_limit_profile() {
        let cfg = parse(MINIMAL_TOML);
        let profile = cfg.rate_limit_profiles.get("default").unwrap();
        assert_eq!(profile.capacity, 100);
        assert_eq!(profile.refill_rate, 10);
        assert_eq!(profile.strategy, KeyStrategy::PerClient);
    }

    #[test]
    fn default_strategy_applied_when_missing() {
        let cfg = parse(MINIMAL_TOML);
        let search = cfg.routes.iter().find(|r| r.id == "search").unwrap();
        assert_eq!(search.strategy, "weighted_round_robin");
    }

    #[test]
    fn default_enabled_true_when_missing() {
        let cfg = parse(MINIMAL_TOML);
        assert!(cfg.routes.iter().all(|r| r.enabled));
    }

    #[test]
    fn to_gateway_route_conversion() {
        let cfg = parse(MINIMAL_TOML);
        let chat = cfg.routes.iter().find(|r| r.id == "chat").unwrap();
        let route = chat.to_gateway_route().unwrap();
        assert_eq!(route.id, "chat");
        assert_eq!(route.agent_id, "agent-chat");
        assert_eq!(route.method, HttpMethod::Post);
    }

    #[test]
    fn to_gateway_route_invalid_method_returns_none() {
        let mut cfg = parse(MINIMAL_TOML);
        cfg.routes[0].method = "INVALID".to_string();
        assert!(cfg.routes[0].to_gateway_route().is_none());
    }

    // ── load_from_file ───────────────────────────────────────────────────────

    #[test]
    fn load_from_file_success() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(MINIMAL_TOML.as_bytes()).unwrap();
        let cfg = GatewayConfigLoader::load_from_file(tmp.path()).unwrap();
        assert_eq!(cfg.routes.len(), 2);
    }

    #[test]
    fn load_from_file_missing_returns_error() {
        let result = GatewayConfigLoader::load_from_file(Path::new("/nonexistent/gateway.toml"));
        assert!(matches!(result, Err(ConfigError::Io(..))));
    }

    #[test]
    fn load_from_file_invalid_toml_returns_error() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"not valid toml ][").unwrap();
        let result = GatewayConfigLoader::load_from_file(tmp.path());
        assert!(matches!(result, Err(ConfigError::Parse(..))));
    }

    // ── diff_configs ─────────────────────────────────────────────────────────

    #[test]
    fn diff_no_changes() {
        let cfg = parse(MINIMAL_TOML);
        let diff = diff_configs(&cfg, &cfg);
        assert!(diff.is_empty());
    }

    #[test]
    fn diff_added_route() {
        let old = parse(MINIMAL_TOML);
        let mut new = old.clone();
        new.routes.push(RouteConfig {
            id: "new-route".to_string(),
            path_pattern: "/v1/new".to_string(),
            method: "DELETE".to_string(),
            agent_id: "agent-new".to_string(),
            strategy: default_strategy(),
            timeout_ms: 0,
            rate_limit: None,
            enabled: true,
        });
        let diff = diff_configs(&old, &new);
        assert_eq!(diff.added, vec!["new-route"]);
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn diff_removed_route() {
        let old = parse(MINIMAL_TOML);
        let mut new = old.clone();
        new.routes.retain(|r| r.id != "search");
        let diff = diff_configs(&old, &new);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["search"]);
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn diff_modified_route() {
        let old = parse(MINIMAL_TOML);
        let mut new = old.clone();
        new.routes.iter_mut().find(|r| r.id == "chat").unwrap().timeout_ms = 9999;
        let diff = diff_configs(&old, &new);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.modified, vec!["chat"]);
    }

    #[test]
    fn diff_combined_changes() {
        let old = parse(MINIMAL_TOML);
        let mut new = old.clone();
        // modify chat
        new.routes.iter_mut().find(|r| r.id == "chat").unwrap().agent_id = "new-agent".to_string();
        // remove search
        new.routes.retain(|r| r.id != "search");
        // add brand-new
        new.routes.push(RouteConfig {
            id: "admin".to_string(),
            path_pattern: "/admin".to_string(),
            method: "GET".to_string(),
            agent_id: "agent-admin".to_string(),
            strategy: default_strategy(),
            timeout_ms: 0,
            rate_limit: None,
            enabled: true,
        });
        let diff = diff_configs(&old, &new);
        assert_eq!(diff.added, vec!["admin"]);
        assert_eq!(diff.removed, vec!["search"]);
        assert_eq!(diff.modified, vec!["chat"]);
    }

    // ── debounce ─────────────────────────────────────────────────────────────

    #[test]
    fn debounce_ignores_second_write_within_200ms() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(MINIMAL_TOML.as_bytes()).unwrap();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<GatewayConfig>(8);
        let _watcher = GatewayConfigLoader::watch(tmp.path(), tx).unwrap();

        let reload_count = Arc::new(AtomicU32::new(0));
        let count_clone = Arc::clone(&reload_count);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            // Write twice within 50ms — should produce only one reload after
            // the 200ms debounce window.
            {
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(tmp.path())
                    .unwrap();
                f.write_all(MINIMAL_TOML.as_bytes()).unwrap();
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            {
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(tmp.path())
                    .unwrap();
                f.write_all(MINIMAL_TOML.as_bytes()).unwrap();
            }

            // Wait long enough for the debounce to fire once.
            tokio::time::sleep(Duration::from_millis(400)).await;

            while let Ok(cfg) = rx.try_recv() {
                let _ = cfg;
                count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        // At most one reload should have fired.
        assert!(
            reload_count.load(std::sync::atomic::Ordering::Relaxed) <= 1,
            "debounce failed: got {} reloads",
            reload_count.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}
