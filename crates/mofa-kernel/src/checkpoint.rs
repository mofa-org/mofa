//! Checkpoint persistence traits

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Auto-checkpoint policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointPolicy {
    Never,
    EveryNode,
    EveryN(usize),
}

impl Default for CheckpointPolicy {
    fn default() -> Self {
        Self::Never
    }
}

/// Stored checkpoint metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSummary {
    pub execution_id: String,
    pub workflow_id: String,
    pub created_at: u64,
    pub node_count: usize,
    pub label: String,
}

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("checkpoint not found: {0}")]
    NotFound(String),
}

/// Pluggable checkpoint persistence backend
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    async fn save(
        &self,
        execution_id: &str,
        workflow_id: &str,
        label: &str,
        data: &serde_json::Value,
    ) -> Result<(), CheckpointError>;

    async fn load(&self, execution_id: &str) -> Result<Option<serde_json::Value>, CheckpointError>;

    async fn list(
        &self,
        workflow_id: Option<&str>,
    ) -> Result<Vec<CheckpointSummary>, CheckpointError>;

    async fn delete(&self, execution_id: &str) -> Result<bool, CheckpointError>;
}
