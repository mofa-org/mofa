//! Plugin registry
//!
//! Manages plugin registration, versioning, and metadata

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::state::PluginState;

/// Plugin version information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginVersion {
    /// Major version
    pub major: u32,
    /// Minor version
    pub minor: u32,
    /// Patch version
    pub patch: u32,
    /// Pre-release tag (e.g., "alpha", "beta")
    pub prerelease: Option<String>,
    /// Build metadata
    pub build: Option<String>,
}

impl PluginVersion {
    /// Create a new version
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
            build: None,
        }
    }

    /// Parse from string (e.g., "1.2.3-alpha+build123")
    pub fn parse(version: &str) -> Result<Self, String> {
        let version = version.trim();

        // Split build metadata
        let (version_pre, build) = if let Some(idx) = version.find('+') {
            (&version[..idx], Some(version[idx + 1..].to_string()))
        } else {
            (version, None)
        };

        // Split prerelease
        let (version_core, prerelease) = if let Some(idx) = version_pre.find('-') {
            (
                &version_pre[..idx],
                Some(version_pre[idx + 1..].to_string()),
            )
        } else {
            (version_pre, None)
        };

        // Parse core version
        let parts: Vec<&str> = version_core.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(format!("Invalid version format: {}", version));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("Invalid major version: {}", parts[0]))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| format!("Invalid minor version: {}", parts[1]))?;
        let patch = if parts.len() > 2 {
            parts[2]
                .parse::<u32>()
                .map_err(|_| format!("Invalid patch version: {}", parts[2]))?
        } else {
            0
        };

        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
            build,
        })
    }

    /// Check if this version is compatible with another (same major version)
    pub fn is_compatible(&self, other: &PluginVersion) -> bool {
        self.major == other.major
    }

    /// Check if this version is newer than another
    pub fn is_newer_than(&self, other: &PluginVersion) -> bool {
        if self.major != other.major {
            return self.major > other.major;
        }
        if self.minor != other.minor {
            return self.minor > other.minor;
        }
        self.patch > other.patch
    }
}

impl std::fmt::Display for PluginVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.prerelease {
            write!(f, "-{}", pre)?;
        }
        if let Some(ref build) = self.build {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

impl Default for PluginVersion {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

/// Plugin information stored in registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: PluginVersion,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: Option<String>,
    /// Library path
    pub library_path: Option<PathBuf>,
    /// Current state
    pub state: PluginState,
    /// Load timestamp
    pub loaded_at: Option<u64>,
    /// Last reload timestamp
    pub last_reload: Option<u64>,
    /// Reload count
    pub reload_count: u32,
    /// Dependencies
    pub dependencies: Vec<String>,
    /// Capabilities/features
    pub capabilities: Vec<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
    /// File hash (for change detection)
    pub file_hash: Option<String>,
}

impl PluginInfo {
    /// Create new plugin info
    pub fn new(id: &str, name: &str, version: PluginVersion) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            version,
            description: String::new(),
            author: None,
            library_path: None,
            state: PluginState::Unloaded,
            loaded_at: None,
            last_reload: None,
            reload_count: 0,
            dependencies: Vec::new(),
            capabilities: Vec::new(),
            metadata: HashMap::new(),
            file_hash: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Set author
    pub fn with_author(mut self, author: &str) -> Self {
        self.author = Some(author.to_string());
        self
    }

    /// Set library path
    pub fn with_library_path<P: AsRef<std::path::Path>>(mut self, path: P) -> Self {
        self.library_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Add dependency
    pub fn with_dependency(mut self, dep: &str) -> Self {
        self.dependencies.push(dep.to_string());
        self
    }

    /// Add capability
    pub fn with_capability(mut self, cap: &str) -> Self {
        self.capabilities.push(cap.to_string());
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Mark as loaded
    pub fn mark_loaded(&mut self) {
        self.state = PluginState::Loaded;
        self.loaded_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }

    /// Mark as reloaded
    pub fn mark_reloaded(&mut self) {
        self.state = PluginState::Loaded;
        self.last_reload = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.reload_count += 1;
    }

    /// Check if dependencies are satisfied
    pub fn check_dependencies(&self, available: &[String]) -> Vec<String> {
        self.dependencies
            .iter()
            .filter(|dep| !available.contains(dep))
            .cloned()
            .collect()
    }

    /// Check if has capability
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }
}

/// Plugin registry for managing loaded plugins
pub struct PluginRegistry {
    /// Registered plugins
    plugins: Arc<RwLock<HashMap<String, PluginInfo>>>,
    /// Plugin path to ID mapping
    path_to_id: Arc<RwLock<HashMap<PathBuf, String>>>,
    /// Enable auto-registration
    auto_register: bool,
}

impl PluginRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            path_to_id: Arc::new(RwLock::new(HashMap::new())),
            auto_register: true,
        }
    }

    /// Set auto-registration mode
    pub fn with_auto_register(mut self, enabled: bool) -> Self {
        self.auto_register = enabled;
        self
    }

    /// Register a plugin
    pub async fn register(&self, info: PluginInfo) -> Result<(), String> {
        let plugin_id = info.id.clone();

        info!("Registering plugin: {} v{}", info.name, info.version);

        let mut plugins = self.plugins.write().await;

        if plugins.contains_key(&plugin_id) {
            return Err(format!("Plugin {} already registered", plugin_id));
        }

        // Update path mapping
        if let Some(ref path) = info.library_path {
            let mut path_map = self.path_to_id.write().await;
            path_map.insert(path.clone(), plugin_id.clone());
        }

        plugins.insert(plugin_id, info);
        Ok(())
    }

    /// Update a plugin registration
    pub async fn update(&self, info: PluginInfo) -> Result<(), String> {
        let plugin_id = info.id.clone();

        debug!("Updating plugin registration: {}", plugin_id);

        let mut plugins = self.plugins.write().await;

        if !plugins.contains_key(&plugin_id) {
            return Err(format!("Plugin {} not registered", plugin_id));
        }

        // Update path mapping if changed
        if let Some(ref path) = info.library_path {
            let mut path_map = self.path_to_id.write().await;
            path_map.insert(path.clone(), plugin_id.clone());
        }

        plugins.insert(plugin_id, info);
        Ok(())
    }

    /// Unregister a plugin
    pub async fn unregister(&self, plugin_id: &str) -> Result<PluginInfo, String> {
        info!("Unregistering plugin: {}", plugin_id);

        let mut plugins = self.plugins.write().await;

        let info = plugins
            .remove(plugin_id)
            .ok_or_else(|| format!("Plugin {} not found", plugin_id))?;

        // Remove path mapping
        if let Some(ref path) = info.library_path {
            let mut path_map = self.path_to_id.write().await;
            path_map.remove(path);
        }

        Ok(info)
    }

    /// Get plugin info
    pub async fn get(&self, plugin_id: &str) -> Option<PluginInfo> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_id).cloned()
    }

    /// Get plugin by path
    pub async fn get_by_path<P: AsRef<std::path::Path>>(&self, path: P) -> Option<PluginInfo> {
        let path = path.as_ref().to_path_buf();

        let path_map = self.path_to_id.read().await;
        if let Some(plugin_id) = path_map.get(&path) {
            let plugins = self.plugins.read().await;
            return plugins.get(plugin_id).cloned();
        }

        None
    }

    /// Check if a plugin is registered
    pub async fn contains(&self, plugin_id: &str) -> bool {
        let plugins = self.plugins.read().await;
        plugins.contains_key(plugin_id)
    }

    /// List all registered plugins
    pub async fn list(&self) -> Vec<PluginInfo> {
        let plugins = self.plugins.read().await;
        plugins.values().cloned().collect()
    }

    /// List plugin IDs
    pub async fn plugin_ids(&self) -> Vec<String> {
        let plugins = self.plugins.read().await;
        plugins.keys().cloned().collect()
    }

    /// Find plugins by capability
    pub async fn find_by_capability(&self, capability: &str) -> Vec<PluginInfo> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|p| p.has_capability(capability))
            .cloned()
            .collect()
    }

    /// Find plugins by state
    pub async fn find_by_state(&self, state: PluginState) -> Vec<PluginInfo> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|p| p.state == state)
            .cloned()
            .collect()
    }

    /// Update plugin state
    pub async fn set_state(&self, plugin_id: &str, state: PluginState) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;

        if let Some(info) = plugins.get_mut(plugin_id) {
            debug!(
                "Updating plugin {} state: {:?} -> {:?}",
                plugin_id, info.state, state
            );
            info.state = state;
            Ok(())
        } else {
            Err(format!("Plugin {} not found", plugin_id))
        }
    }

    /// Get dependency order for loading
    pub async fn get_load_order(&self) -> Result<Vec<String>, String> {
        let plugins = self.plugins.read().await;

        // Build dependency graph
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for (id, info) in plugins.iter() {
            in_degree.entry(id.clone()).or_insert(0);

            for dep in &info.dependencies {
                dependents.entry(dep.clone()).or_default().push(id.clone());
                *in_degree.entry(id.clone()).or_insert(0) += 1;
            }
        }

        // Topological sort (Kahn's algorithm)
        let mut result = Vec::new();
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, deg)| deg == &&0)
            .map(|(id, _)| id.clone())
            .collect();

        while let Some(id) = queue.pop() {
            result.push(id.clone());

            if let Some(deps) = dependents.get(&id) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }

        if result.len() != plugins.len() {
            return Err("Circular dependency detected".to_string());
        }

        Ok(result)
    }

    /// Clear all registrations
    pub async fn clear(&self) {
        let mut plugins = self.plugins.write().await;
        plugins.clear();

        let mut path_map = self.path_to_id.write().await;
        path_map.clear();
    }

    /// Get registry statistics
    pub async fn stats(&self) -> RegistryStats {
        let plugins = self.plugins.read().await;

        let mut stats = RegistryStats {
            total_plugins: plugins.len(),
            ..RegistryStats::default()
        };

        for info in plugins.values() {
            match info.state {
                PluginState::Loaded | PluginState::Running => stats.loaded_plugins += 1,
                PluginState::Failed(_) => stats.failed_plugins += 1,
                _ => {}
            }
            stats.total_reloads += info.reload_count as usize;
        }

        stats
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry statistics
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    /// Total registered plugins
    pub total_plugins: usize,
    /// Currently loaded plugins
    pub loaded_plugins: usize,
    /// Failed plugins
    pub failed_plugins: usize,
    /// Total reload count
    pub total_reloads: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse() {
        let v = PluginVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);

        let v = PluginVersion::parse("1.2.3-alpha").unwrap();
        assert_eq!(v.prerelease, Some("alpha".to_string()));

        let v = PluginVersion::parse("1.2.3-beta+build123").unwrap();
        assert_eq!(v.prerelease, Some("beta".to_string()));
        assert_eq!(v.build, Some("build123".to_string()));
    }

    #[test]
    fn test_version_comparison() {
        let v1 = PluginVersion::new(1, 0, 0);
        let v2 = PluginVersion::new(1, 1, 0);
        let v3 = PluginVersion::new(2, 0, 0);

        assert!(v1.is_compatible(&v2));
        assert!(!v1.is_compatible(&v3));
        assert!(v2.is_newer_than(&v1));
        assert!(v3.is_newer_than(&v2));
    }

    #[test]
    fn test_version_display() {
        let v = PluginVersion::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");

        let mut v = PluginVersion::new(1, 0, 0);
        v.prerelease = Some("alpha".to_string());
        assert_eq!(v.to_string(), "1.0.0-alpha");
    }

    #[test]
    fn test_plugin_info() {
        let info = PluginInfo::new("test", "Test Plugin", PluginVersion::new(1, 0, 0))
            .with_description("A test plugin")
            .with_author("Developer")
            .with_capability("feature_a")
            .with_capability("feature_b");

        assert_eq!(info.id, "test");
        assert!(info.has_capability("feature_a"));
        assert!(!info.has_capability("feature_c"));
    }

    #[tokio::test]
    async fn test_registry() {
        let registry = PluginRegistry::new();

        let info = PluginInfo::new("plugin-1", "Plugin 1", PluginVersion::new(1, 0, 0));
        registry.register(info).await.unwrap();

        assert!(registry.contains("plugin-1").await);
        assert!(!registry.contains("plugin-2").await);

        let loaded = registry.get("plugin-1").await.unwrap();
        assert_eq!(loaded.name, "Plugin 1");

        registry.unregister("plugin-1").await.unwrap();
        assert!(!registry.contains("plugin-1").await);
    }

    #[tokio::test]
    async fn test_registry_load_order() {
        let registry = PluginRegistry::new();

        // Plugin A has no dependencies
        let a = PluginInfo::new("a", "A", PluginVersion::new(1, 0, 0));
        registry.register(a).await.unwrap();

        // Plugin B depends on A
        let b = PluginInfo::new("b", "B", PluginVersion::new(1, 0, 0)).with_dependency("a");
        registry.register(b).await.unwrap();

        // Plugin C depends on B
        let c = PluginInfo::new("c", "C", PluginVersion::new(1, 0, 0)).with_dependency("b");
        registry.register(c).await.unwrap();

        let order = registry.get_load_order().await.unwrap();

        // A should come before B, B before C
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_c = order.iter().position(|x| x == "c").unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }
}
