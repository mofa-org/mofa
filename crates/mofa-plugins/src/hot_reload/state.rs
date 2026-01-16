//! Plugin state management
//!
//! Handles state preservation and restoration during hot-reload

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Plugin hot-reload state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PluginState {
    /// Not loaded
    #[default]
    Unloaded,
    /// Loading in progress
    Loading,
    /// Loaded and ready
    Loaded,
    /// Running
    Running,
    /// Reloading in progress
    Reloading,
    /// Failed to load/reload
    Failed(String),
    /// Unloading in progress
    Unloading,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginState::Unloaded => write!(f, "Unloaded"),
            PluginState::Loading => write!(f, "Loading"),
            PluginState::Loaded => write!(f, "Loaded"),
            PluginState::Running => write!(f, "Running"),
            PluginState::Reloading => write!(f, "Reloading"),
            PluginState::Failed(err) => write!(f, "Failed: {}", err),
            PluginState::Unloading => write!(f, "Unloading"),
        }
    }
}

/// A snapshot of plugin state that can be preserved across reloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Plugin ID
    pub plugin_id: String,
    /// Snapshot timestamp
    pub timestamp: u64,
    /// Serialized state data
    pub data: HashMap<String, serde_json::Value>,
    /// Plugin version at snapshot time
    pub plugin_version: String,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl StateSnapshot {
    /// Create a new state snapshot
    pub fn new(plugin_id: &str, plugin_version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            data: HashMap::new(),
            plugin_version: plugin_version.to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Add state data
    pub fn with_data<T: Serialize>(mut self, key: &str, value: &T) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.data.insert(key.to_string(), json_value);
        }
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Get state data
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Check if snapshot is compatible with a plugin version
    pub fn is_compatible(&self, plugin_version: &str) -> bool {
        // Simple semantic version major check
        let snapshot_major = self.plugin_version.split('.').next();
        let plugin_major = plugin_version.split('.').next();
        snapshot_major == plugin_major
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

/// State manager for plugin hot-reload
pub struct StateManager {
    /// Active snapshots by plugin ID
    snapshots: Arc<RwLock<HashMap<String, StateSnapshot>>>,
    /// Historical snapshots (for rollback)
    history: Arc<RwLock<HashMap<String, Vec<StateSnapshot>>>>,
    /// Maximum history entries per plugin
    max_history: usize,
    /// Enable state persistence
    persist_enabled: bool,
    /// Persistence directory
    persist_dir: Option<std::path::PathBuf>,
}

impl StateManager {
    /// Create a new state manager
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            max_history: 10,
            persist_enabled: false,
            persist_dir: None,
        }
    }

    /// Set maximum history entries
    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// Enable state persistence
    pub fn with_persistence<P: AsRef<std::path::Path>>(mut self, dir: P) -> Self {
        self.persist_enabled = true;
        self.persist_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Save a state snapshot
    pub async fn save_snapshot(&self, snapshot: StateSnapshot) -> Result<(), String> {
        let plugin_id = snapshot.plugin_id.clone();

        info!("Saving state snapshot for plugin: {}", plugin_id);

        // Move current snapshot to history
        let mut snapshots = self.snapshots.write().await;
        if let Some(current) = snapshots.remove(&plugin_id) {
            let mut history = self.history.write().await;
            let entry = history.entry(plugin_id.clone()).or_insert_with(Vec::new);
            entry.push(current);

            // Trim history
            if entry.len() > self.max_history {
                let to_remove = entry.len() - self.max_history;
                entry.drain(0..to_remove);
            }
        }

        // Save new snapshot
        if self.persist_enabled
            && let Err(e) = self.persist_snapshot(&snapshot).await
        {
            warn!("Failed to persist snapshot: {}", e);
        }

        snapshots.insert(plugin_id, snapshot);
        Ok(())
    }

    /// Load a state snapshot
    pub async fn load_snapshot(&self, plugin_id: &str) -> Option<StateSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.get(plugin_id).cloned()
    }

    /// Get the latest snapshot for a plugin
    pub async fn get_latest(&self, plugin_id: &str) -> Option<StateSnapshot> {
        // First try current snapshots
        if let Some(snapshot) = self.load_snapshot(plugin_id).await {
            return Some(snapshot);
        }

        // Try to load from persistence
        if self.persist_enabled
            && let Ok(snapshot) = self.load_persisted_snapshot(plugin_id).await
        {
            return Some(snapshot);
        }

        None
    }

    /// Rollback to a previous snapshot
    pub async fn rollback(&self, plugin_id: &str) -> Option<StateSnapshot> {
        let mut history = self.history.write().await;

        if let Some(entry) = history.get_mut(plugin_id)
            && let Some(snapshot) = entry.pop()
        {
            info!(
                "Rolling back plugin {} to snapshot from {}",
                plugin_id, snapshot.timestamp
            );

            // Update current snapshot
            let mut snapshots = self.snapshots.write().await;
            snapshots.insert(plugin_id.to_string(), snapshot.clone());

            return Some(snapshot);
        }

        None
    }

    /// Clear all snapshots for a plugin
    pub async fn clear(&self, plugin_id: &str) {
        debug!("Clearing snapshots for plugin: {}", plugin_id);

        let mut snapshots = self.snapshots.write().await;
        snapshots.remove(plugin_id);

        let mut history = self.history.write().await;
        history.remove(plugin_id);
    }

    /// Clear all snapshots
    pub async fn clear_all(&self) {
        debug!("Clearing all snapshots");

        let mut snapshots = self.snapshots.write().await;
        snapshots.clear();

        let mut history = self.history.write().await;
        history.clear();
    }

    /// Get history for a plugin
    pub async fn get_history(&self, plugin_id: &str) -> Vec<StateSnapshot> {
        let history = self.history.read().await;
        history.get(plugin_id).cloned().unwrap_or_default()
    }

    /// Get all managed plugin IDs
    pub async fn plugin_ids(&self) -> Vec<String> {
        let snapshots = self.snapshots.read().await;
        snapshots.keys().cloned().collect()
    }

    /// Persist snapshot to disk
    async fn persist_snapshot(&self, snapshot: &StateSnapshot) -> Result<(), String> {
        let dir = self
            .persist_dir
            .as_ref()
            .ok_or_else(|| "Persistence directory not set".to_string())?;

        // Ensure directory exists
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create persistence directory: {}", e))?;

        let file_path = dir.join(format!("{}.json", snapshot.plugin_id));

        let json = serde_json::to_string_pretty(snapshot)
            .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;

        std::fs::write(&file_path, json)
            .map_err(|e| format!("Failed to write snapshot file: {}", e))?;

        debug!("Persisted snapshot to {:?}", file_path);
        Ok(())
    }

    /// Load persisted snapshot from disk
    async fn load_persisted_snapshot(&self, plugin_id: &str) -> Result<StateSnapshot, String> {
        let dir = self
            .persist_dir
            .as_ref()
            .ok_or_else(|| "Persistence directory not set".to_string())?;

        let file_path = dir.join(format!("{}.json", plugin_id));

        let json = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read snapshot file: {}", e))?;

        serde_json::from_str(&json).map_err(|e| format!("Failed to deserialize snapshot: {}", e))
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for plugins that support state preservation
pub trait StatefulPlugin {
    /// Create a state snapshot
    fn create_snapshot(&self) -> StateSnapshot;

    /// Restore from a state snapshot
    fn restore_snapshot(&mut self, snapshot: &StateSnapshot) -> Result<(), String>;

    /// Check if a snapshot is compatible
    fn is_snapshot_compatible(&self, snapshot: &StateSnapshot) -> bool {
        snapshot.is_compatible(&self.plugin_version())
    }

    /// Get plugin version
    fn plugin_version(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_state_display() {
        assert_eq!(PluginState::Unloaded.to_string(), "Unloaded");
        assert_eq!(PluginState::Running.to_string(), "Running");
        assert_eq!(
            PluginState::Failed("test".to_string()).to_string(),
            "Failed: test"
        );
    }

    #[test]
    fn test_state_snapshot() {
        let snapshot = StateSnapshot::new("test-plugin", "1.0.0")
            .with_data("counter", &42)
            .with_data("name", &"test")
            .with_metadata("author", "developer");

        assert_eq!(snapshot.plugin_id, "test-plugin");
        assert_eq!(snapshot.plugin_version, "1.0.0");
        assert_eq!(snapshot.get::<i32>("counter"), Some(42));
        assert_eq!(snapshot.get::<String>("name"), Some("test".to_string()));
        assert_eq!(snapshot.get_metadata("author"), Some("developer"));
    }

    #[test]
    fn test_snapshot_compatibility() {
        let snapshot = StateSnapshot::new("test", "1.0.0");

        assert!(snapshot.is_compatible("1.0.0"));
        assert!(snapshot.is_compatible("1.1.0"));
        assert!(snapshot.is_compatible("1.2.3"));
        assert!(!snapshot.is_compatible("2.0.0"));
    }

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = StateSnapshot::new("test", "1.0.0").with_data("value", &123);

        let bytes = snapshot.to_bytes().unwrap();
        let restored = StateSnapshot::from_bytes(&bytes).unwrap();

        assert_eq!(restored.plugin_id, snapshot.plugin_id);
        assert_eq!(restored.get::<i32>("value"), Some(123));
    }

    #[tokio::test]
    async fn test_state_manager() {
        let manager = StateManager::new();

        let snapshot = StateSnapshot::new("plugin-1", "1.0.0").with_data("state", &"active");

        manager.save_snapshot(snapshot.clone()).await.unwrap();

        let loaded = manager.load_snapshot("plugin-1").await.unwrap();
        assert_eq!(loaded.plugin_id, "plugin-1");
        assert_eq!(loaded.get::<String>("state"), Some("active".to_string()));
    }

    #[tokio::test]
    async fn test_state_manager_rollback() {
        let manager = StateManager::new();

        // Save first snapshot
        let snapshot1 = StateSnapshot::new("plugin-1", "1.0.0").with_data("version", &1);
        manager.save_snapshot(snapshot1).await.unwrap();

        // Save second snapshot (moves first to history)
        let snapshot2 = StateSnapshot::new("plugin-1", "1.0.0").with_data("version", &2);
        manager.save_snapshot(snapshot2).await.unwrap();

        // Current should be version 2
        let current = manager.load_snapshot("plugin-1").await.unwrap();
        assert_eq!(current.get::<i32>("version"), Some(2));

        // Rollback to version 1
        let rolled_back = manager.rollback("plugin-1").await.unwrap();
        assert_eq!(rolled_back.get::<i32>("version"), Some(1));

        // Current should now be version 1
        let current = manager.load_snapshot("plugin-1").await.unwrap();
        assert_eq!(current.get::<i32>("version"), Some(1));
    }
}
