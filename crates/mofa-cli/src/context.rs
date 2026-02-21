//! CLI context providing access to backend services

use crate::utils::paths;
use mofa_foundation::agent::session::SessionManager;
use mofa_foundation::agent::tools::registry::ToolRegistry;
use mofa_runtime::agent::plugins::SimplePluginRegistry;
use mofa_runtime::agent::registry::AgentRegistry;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared context for CLI commands, holding references to backend services
pub struct CliContext {
    /// Session manager with file-based persistence
    pub session_manager: SessionManager,
    /// In-memory agent registry
    pub agent_registry: AgentRegistry,
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

        let sessions_dir = data_dir.join("sessions");
        let session_manager = SessionManager::with_jsonl(&sessions_dir)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize session manager: {}", e))?;

        Ok(Self {
            session_manager,
            agent_registry: AgentRegistry::new(),
            plugin_registry: Arc::new(SimplePluginRegistry::new()),
            tool_registry: ToolRegistry::new(),
            data_dir,
            config_dir,
        })
    }
}
