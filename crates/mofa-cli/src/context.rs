//! CLI context providing access to backend services

use crate::store::PersistedStore;
use crate::utils::paths;
use mofa_foundation::agent::session::SessionManager;
use mofa_foundation::agent::tools::registry::ToolRegistry;
use mofa_runtime::agent::plugins::SimplePluginRegistry;
use mofa_runtime::agent::registry::AgentRegistry;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigEntry {
    pub id: String,
    pub name: String,
    pub state: String,
}

/// Shared context for CLI commands, holding references to backend services
pub struct CliContext {
    /// Session manager with file-based persistence
    pub session_manager: SessionManager,
    /// In-memory agent registry
    pub agent_registry: AgentRegistry,
    /// Persistent agent metadata store
    pub agent_store: PersistedStore<AgentConfigEntry>,
    /// In-memory plugin registry
    pub plugin_registry: Arc<SimplePluginRegistry>,
    /// In-memory tool registry
    pub tool_registry: ToolRegistry,
    /// Platform-specific data directory (~/.local/share/mofa or equivalent)
    pub data_dir: PathBuf,
    /// Platform-specific config directory (~/.config/mofa or equivalent)
    pub config_dir: PathBuf,
}

impl CliContext {
    /// Initialize the CLI context with default backend services
    pub async fn new() -> anyhow::Result<Self> {
        let data_dir = paths::ensure_mofa_data_dir()?;
        let config_dir = paths::ensure_mofa_config_dir()?;
        migrate_legacy_nested_sessions(&data_dir)?;

        let session_manager = SessionManager::with_jsonl(&data_dir)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize session manager: {}", e))?;
        let agent_store = PersistedStore::new(data_dir.join("agents"))?;

        Ok(Self {
            session_manager,
            agent_registry: AgentRegistry::new(),
            agent_store,
            plugin_registry: Arc::new(SimplePluginRegistry::new()),
            tool_registry: ToolRegistry::new(),
            data_dir,
            config_dir,
        })
    }
}

#[cfg(test)]
impl CliContext {
    pub async fn with_temp_dir(temp_dir: &std::path::Path) -> anyhow::Result<Self> {
        let data_dir = temp_dir.join("data");
        let config_dir = temp_dir.join("config");
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&config_dir)?;
        migrate_legacy_nested_sessions(&data_dir)?;

        let session_manager = SessionManager::with_jsonl(&data_dir)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let agent_store = PersistedStore::new(data_dir.join("agents"))?;

        Ok(Self {
            session_manager,
            agent_registry: AgentRegistry::new(),
            agent_store,
            plugin_registry: Arc::new(SimplePluginRegistry::new()),
            tool_registry: ToolRegistry::new(),
            data_dir,
            config_dir,
        })
    }
}

fn migrate_legacy_nested_sessions(data_dir: &Path) -> anyhow::Result<()> {
    let sessions_dir = data_dir.join("sessions");
    let legacy_dir = sessions_dir.join("sessions");
    if !legacy_dir.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(&sessions_dir)?;
    for entry in std::fs::read_dir(&legacy_dir)? {
        let entry = entry?;
        let src = entry.path();
        let dst = sessions_dir.join(entry.file_name());

        if dst.exists() {
            continue;
        }

        std::fs::rename(&src, &dst)?;
    }

    let _ = std::fs::remove_dir(&legacy_dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_foundation::agent::session::Session;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_store_persists_across_context_instances() {
        let temp = TempDir::new().unwrap();

        let ctx1 = CliContext::with_temp_dir(temp.path()).await.unwrap();
        let entry = AgentConfigEntry {
            id: "persisted-agent".to_string(),
            name: "Persisted Agent".to_string(),
            state: "Running".to_string(),
        };
        ctx1.agent_store.save("persisted-agent", &entry).unwrap();
        drop(ctx1);

        let ctx2 = CliContext::with_temp_dir(temp.path()).await.unwrap();
        let loaded = ctx2.agent_store.get("persisted-agent").unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "persisted-agent");
    }

    #[tokio::test]
    async fn test_legacy_nested_sessions_are_migrated() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();

        let legacy_manager = SessionManager::with_jsonl(data_dir.join("sessions"))
            .await
            .unwrap();
        let mut session = Session::new("legacy-session");
        session.add_message("user", "hello");
        legacy_manager.save(&session).await.unwrap();

        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        let loaded = ctx.session_manager.get("legacy-session").await.unwrap();

        assert!(loaded.is_some());
        assert!(
            data_dir
                .join("sessions")
                .join("legacy-session.jsonl")
                .exists()
        );
    }
}
