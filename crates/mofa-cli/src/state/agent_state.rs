//! Agent state persistence layer
//!
//! Manages persistent storage and lifecycle of agents on the local system.

use crate::CliError;

type Result<T> = std::result::Result<T, CliError>;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Agent runtime state (in-memory process tracking)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentProcessState {
    /// Agent is not running
    Stopped,
    /// Agent is starting up
    Starting,
    /// Agent is running
    Running,
    /// Agent is stopping
    Stopping,
    /// Agent has encountered an error
    Error,
}

impl std::fmt::Display for AgentProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "Stopped"),
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::Stopping => write!(f, "Stopping"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Persistent agent metadata (stored on disk)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    /// Unique agent identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Agent description
    pub description: Option<String>,
    /// Path to agent configuration file
    pub config_path: Option<PathBuf>,
    /// Last known state
    pub last_state: AgentProcessState,
    /// Timestamp when agent was registered (ms since epoch)
    pub registered_at: u64,
    /// Timestamp when agent was last started (ms since epoch)
    pub last_started: Option<u64>,
    /// Timestamp when agent was last stopped (ms since epoch)
    pub last_stopped: Option<u64>,
    /// Process ID if running
    pub process_id: Option<u32>,
    /// Number of times agent has been started
    pub start_count: u32,
    /// Custom metadata
    pub tags: Vec<String>,
}

impl AgentMetadata {
    /// Create new agent metadata
    pub fn new(id: String, name: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id,
            name,
            description: None,
            config_path: None,
            last_state: AgentProcessState::Stopped,
            registered_at: now,
            last_started: None,
            last_stopped: None,
            process_id: None,
            start_count: 0,
            tags: Vec::new(),
        }
    }

    /// Set configuration path
    pub fn with_config(mut self, path: PathBuf) -> Self {
        self.config_path = Some(path);
        self
    }

    /// Add tag
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Mark as started
    pub fn mark_started(&mut self, pid: u32) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_started = Some(now);
        self.process_id = Some(pid);
        self.last_state = AgentProcessState::Running;
        self.start_count += 1;
    }

    /// Mark as stopped
    pub fn mark_stopped(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_stopped = Some(now);
        self.process_id = None;
        self.last_state = AgentProcessState::Stopped;
    }

    /// Mark as error
    pub fn mark_error(&mut self) {
        self.last_state = AgentProcessState::Error;
    }
}

/// Persistent agent registry - stores agents to disk
pub struct PersistentAgentRegistry {
    /// Directory where agent metadata is stored
    agents_dir: PathBuf,
    /// In-memory cache of agent metadata
    metadata_cache: Arc<RwLock<HashMap<String, AgentMetadata>>>,
    /// Map of running processes (agent_id -> Child process)
    running_processes: Arc<RwLock<HashMap<String, RunningAgent>>>,
}

/// Represents a running agent process
pub struct RunningAgent {
    /// Agent metadata
    pub metadata: AgentMetadata,
    /// Process handle (may be None if process handle is lost)
    pub process: Option<Child>,
}

impl PersistentAgentRegistry {
    /// Create or load agent registry from disk
    pub async fn new(agents_dir: PathBuf) -> Result<Self> {
        // Ensure directory exists
        tokio::fs::create_dir_all(&agents_dir).await?;

        let mut metadata_cache = HashMap::new();

        // Load existing agents from disk
        let mut entries = tokio::fs::read_dir(&agents_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        match serde_json::from_str::<AgentMetadata>(&content) {
                            Ok(mut metadata) => {
                                // Reset running state to stopped (process was not preserved across restarts)
                                metadata.last_state = AgentProcessState::Stopped;
                                metadata.process_id = None;
                                metadata_cache.insert(metadata.id.clone(), metadata);
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to parse agent metadata from {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to read agent metadata from {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(Self {
            agents_dir,
            metadata_cache: Arc::new(RwLock::new(metadata_cache)),
            running_processes: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Register a new agent
    pub async fn register(&self, metadata: AgentMetadata) -> Result<()> {
        let agent_id = metadata.id.clone();
        let file_path = self.agents_dir.join(format!("{}.json", agent_id));

        // Write to disk
        let json = serde_json::to_string_pretty(&metadata)?;
        tokio::fs::write(&file_path, json).await?;

        // Update cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.insert(agent_id.clone(), metadata);
        }

        info!("Registered agent: {} in {}", agent_id, file_path.display());
        Ok(())
    }

    /// Get agent metadata
    pub async fn get(&self, agent_id: &str) -> Option<AgentMetadata> {
        let cache = self.metadata_cache.read().await;
        cache.get(agent_id).cloned()
    }

    /// List all agents
    pub async fn list(&self) -> Vec<AgentMetadata> {
        let cache = self.metadata_cache.read().await;
        let mut agents: Vec<_> = cache.values().cloned().collect();
        agents.sort_by(|a, b| a.id.cmp(&b.id));
        agents
    }

    /// List running agents
    pub async fn list_running(&self) -> Vec<AgentMetadata> {
        self.list()
            .await
            .into_iter()
            .filter(|a| a.last_state == AgentProcessState::Running)
            .collect()
    }

    /// Update agent metadata
    pub async fn update(&self, metadata: AgentMetadata) -> Result<()> {
        let agent_id = metadata.id.clone();
        let file_path = self.agents_dir.join(format!("{}.json", agent_id));

        // Write to disk
        let json = serde_json::to_string_pretty(&metadata)?;
        tokio::fs::write(&file_path, json).await?;

        // Update cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.insert(agent_id.clone(), metadata);
        }

        debug!("Updated agent metadata: {}", agent_id);
        Ok(())
    }

    /// Remove agent
    pub async fn remove(&self, agent_id: &str) -> Result<bool> {
        let file_path = self.agents_dir.join(format!("{}.json", agent_id));

        // Try to remove file
        let removed = match tokio::fs::remove_file(&file_path).await {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
        };

        // Remove from cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.remove(agent_id);
        }

        if removed {
            info!("Removed agent: {} from {}", agent_id, file_path.display());
        }

        Ok(removed)
    }

    /// Check if agent exists
    pub async fn exists(&self, agent_id: &str) -> bool {
        let cache = self.metadata_cache.read().await;
        cache.contains_key(agent_id)
    }

    /// Get count of registered agents
    pub async fn count(&self) -> usize {
        let cache = self.metadata_cache.read().await;
        cache.len()
    }

    /// Track a running process
    pub async fn track_process(&self, agent_id: String, process: Child, metadata: AgentMetadata) {
        let mut processes = self.running_processes.write().await;
        processes.insert(
            agent_id,
            RunningAgent {
                metadata,
                process: Some(process),
            },
        );
    }

    /// Get running process
    pub async fn get_process(&self, agent_id: &str) -> Option<u32> {
        let processes = self.running_processes.read().await;
        processes.get(agent_id).and_then(|a| a.metadata.process_id)
    }

    /// Remove tracking of a process
    pub async fn untrack_process(&self, agent_id: &str) {
        let mut processes = self.running_processes.write().await;
        processes.remove(agent_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_metadata_lifecycle() {
        let mut metadata = AgentMetadata::new("agent-1".into(), "My Agent".into());
        assert_eq!(metadata.last_state, AgentProcessState::Stopped);
        assert_eq!(metadata.start_count, 0);

        metadata.mark_started(12345);
        assert_eq!(metadata.last_state, AgentProcessState::Running);
        assert_eq!(metadata.process_id, Some(12345));
        assert_eq!(metadata.start_count, 1);

        metadata.mark_stopped();
        assert_eq!(metadata.last_state, AgentProcessState::Stopped);
        assert_eq!(metadata.process_id, None);
    }

    #[tokio::test]
    async fn test_registry_persistence() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let registry = PersistentAgentRegistry::new(temp_dir.path().to_path_buf()).await?;

        // Register agent
        let metadata = AgentMetadata::new("agent-1".into(), "Test Agent".into());
        registry.register(metadata.clone()).await?;

        // Verify it exists
        assert!(registry.exists("agent-1").await);
        let retrieved = registry.get("agent-1").await;
        assert_eq!(retrieved.map(|m| m.name), Some("Test Agent".into()));

        // Reload registry (simulating restart)
        let registry2 = PersistentAgentRegistry::new(temp_dir.path().to_path_buf()).await?;
        assert!(registry2.exists("agent-1").await);
        assert_eq!(registry2.count().await, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_list_running_agents() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let registry = PersistentAgentRegistry::new(temp_dir.path().to_path_buf()).await?;

        // Register multiple agents
        let mut agent1 = AgentMetadata::new("agent-1".into(), "Agent 1".into());
        agent1.mark_started(111);
        registry.register(agent1).await?;

        let mut agent2 = AgentMetadata::new("agent-2".into(), "Agent 2".into());
        agent2.mark_started(222);
        registry.register(agent2).await?;

        let agent3 = AgentMetadata::new("agent-3".into(), "Agent 3".into());
        registry.register(agent3).await?;

        // Only 2 are running
        let running = registry.list_running().await;
        assert_eq!(running.len(), 2);

        Ok(())
    }
}
