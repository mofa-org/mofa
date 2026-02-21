//! Agent state store trait and types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Agent status states
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent is currently running
    Running,
    /// Agent is stopped
    Stopped,
    /// Agent is paused
    Paused,
    /// Agent encountered an error
    Error(String),
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Paused => write!(f, "paused"),
            AgentStatus::Error(e) => write!(f, "error: {}", e),
        }
    }
}

/// Agent record stored in the state database
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRecord {
    /// Unique agent identifier
    pub id: String,
    /// Human-readable agent name
    pub name: String,
    /// Current agent status
    pub status: AgentStatus,
    /// Unix timestamp when agent was started (if running)
    pub started_at: Option<u64>,
    /// Path to agent configuration file
    pub config_path: Option<String>,
    /// LLM provider name (e.g., "openai")
    pub provider: Option<String>,
    /// LLM model name (e.g., "gpt-4o")
    pub model: Option<String>,
}

impl AgentRecord {
    /// Create a new agent record
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            status: AgentStatus::Stopped,
            started_at: None,
            config_path: None,
            provider: None,
            model: None,
        }
    }

    /// Calculate uptime if agent is running
    pub fn uptime(&self) -> Option<String> {
        if let Some(started_at) = self.started_at {
            if self.status == AgentStatus::Running {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let elapsed = now.saturating_sub(started_at);

                let hours = elapsed / 3600;
                let minutes = (elapsed % 3600) / 60;
                let seconds = elapsed % 60;

                if hours > 0 {
                    return Some(format!("{}h {}m {}s", hours, minutes, seconds));
                } else if minutes > 0 {
                    return Some(format!("{}m {}s", minutes, seconds));
                } else {
                    return Some(format!("{}s", seconds));
                }
            }
        }
        None
    }
}

/// Trait for persistent storage of agent state
#[async_trait]
pub trait AgentStateStore: Send + Sync {
    /// List all agents
    async fn list(&self) -> anyhow::Result<Vec<AgentRecord>>;

    /// Get a specific agent by ID
    async fn get(&self, agent_id: &str) -> anyhow::Result<Option<AgentRecord>>;

    /// Create a new agent record
    async fn create(&self, record: AgentRecord) -> anyhow::Result<()>;

    /// Update an existing agent record
    async fn update(&self, record: AgentRecord) -> anyhow::Result<()>;

    /// Delete an agent record
    async fn delete(&self, agent_id: &str) -> anyhow::Result<()>;

    /// Check if an agent exists
    async fn exists(&self, agent_id: &str) -> anyhow::Result<bool> {
        Ok(self.get(agent_id).await?.is_some())
    }
}
