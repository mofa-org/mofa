//! Agent state management and persistence
//!
//! This module provides a persistent store for agent state,
//! allowing the CLI to track running/stopped agents without
//! requiring a running daemon.

pub mod store;
pub mod sqlite;

pub use store::{AgentStateStore, AgentRecord, AgentStatus};
pub use sqlite::SqliteAgentStateStore;

use anyhow::Result;
use std::path::PathBuf;

/// Get the default agent state store location
pub fn get_default_state_dir() -> Result<PathBuf> {
    let mofa_dir = if let Ok(custom_dir) = std::env::var("MOFA_STATE_DIR") {
        PathBuf::from(custom_dir)
    } else {
        let home = dirs_next::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        home.join(".mofa")
    };

    // Create directory if it doesn't exist
    if !mofa_dir.exists() {
        std::fs::create_dir_all(&mofa_dir)?;
    }

    Ok(mofa_dir)
}

/// Get the default agent state database path
pub fn get_default_state_db_path() -> Result<PathBuf> {
    let state_dir = get_default_state_dir()?;
    Ok(state_dir.join("agents.db"))
}

/// Get or create the default agent state store
pub async fn get_agent_store() -> Result<SqliteAgentStateStore> {
    let db_path = get_default_state_db_path()?;
    SqliteAgentStateStore::new(db_path).await
}
