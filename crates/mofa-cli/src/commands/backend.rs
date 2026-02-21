//! Shared backend repository for CLI command handlers.

use crate::utils::paths::mofa_data_dir;
use mofa_sdk::react::tools::prelude::all_builtin_tools;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

const BACKEND_DIR: &str = "cli-backend";
const AGENTS_FILE: &str = "agents_state.json";
const PLUGINS_FILE: &str = "plugins_registry.json";
const TOOLS_FILE: &str = "tools_registry.json";
const SESSIONS_FILE: &str = "sessions_store.json";

#[derive(Debug)]
pub enum CliBackendError {
    CapabilityUnavailable {
        capability: &'static str,
        reason: String,
    },
    NotFound {
        kind: &'static str,
        id: String,
    },
    InvalidInput(String),
    Io(std::io::Error),
    Serde(serde_json::Error),
}

impl Display for CliBackendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityUnavailable { capability, reason } => {
                write!(
                    f,
                    "Backend capability '{}' unavailable: {}",
                    capability, reason
                )
            }
            Self::NotFound { kind, id } => write!(f, "{} '{}' not found", kind, id),
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Self::Io(err) => write!(f, "I/O error: {}", err),
            Self::Serde(err) => write!(f, "Serialization error: {}", err),
        }
    }
}

impl std::error::Error for CliBackendError {}

impl From<std::io::Error> for CliBackendError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for CliBackendError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub daemon: bool,
    pub config_path: Option<String>,
    pub uptime: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub agent_id: String,
    pub created_at: String,
    pub message_count: usize,
    pub status: String,
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Clone)]
pub struct CliBackend {
    root: PathBuf,
}

impl CliBackend {
    pub fn discover() -> Result<Self, CliBackendError> {
        let root = if let Ok(dir) = std::env::var("MOFA_CLI_DATA_DIR") {
            PathBuf::from(dir).join(BACKEND_DIR)
        } else {
            mofa_data_dir()
                .map_err(|err| CliBackendError::CapabilityUnavailable {
                    capability: "data_dir",
                    reason: err.to_string(),
                })?
                .join(BACKEND_DIR)
        };
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    #[cfg(test)]
    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn list_agents(
        &self,
        running_only: bool,
    ) -> Result<Vec<AgentRuntimeInfo>, CliBackendError> {
        let agents =
            self.read_or_default(self.path(AGENTS_FILE), Vec::<AgentRuntimeInfo>::new())?;
        if running_only {
            Ok(agents
                .into_iter()
                .filter(|agent| agent.status.eq_ignore_ascii_case("running"))
                .collect())
        } else {
            Ok(agents)
        }
    }

    pub fn get_agent(&self, agent_id: &str) -> Result<AgentRuntimeInfo, CliBackendError> {
        self.list_agents(false)?
            .into_iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| CliBackendError::NotFound {
                kind: "Agent",
                id: agent_id.to_string(),
            })
    }

    pub fn start_agent(
        &self,
        agent_id: &str,
        config_path: Option<&Path>,
        daemon: bool,
    ) -> Result<AgentRuntimeInfo, CliBackendError> {
        if let Some(path) = config_path
            && !path.exists()
        {
            return Err(CliBackendError::InvalidInput(format!(
                "Config path '{}' does not exist",
                path.display()
            )));
        }

        let mut agents =
            self.read_or_default(self.path(AGENTS_FILE), Vec::<AgentRuntimeInfo>::new())?;
        let now = now_utc_string();
        let updated = if let Some(existing) = agents.iter_mut().find(|agent| agent.id == agent_id) {
            existing.status = "running".to_string();
            existing.daemon = daemon;
            existing.config_path = config_path.map(|p| p.display().to_string());
            existing.uptime = Some("just started".to_string());
            existing.updated_at = now;
            existing.clone()
        } else {
            let info = AgentRuntimeInfo {
                id: agent_id.to_string(),
                name: agent_id.to_string(),
                status: "running".to_string(),
                daemon,
                config_path: config_path.map(|p| p.display().to_string()),
                uptime: Some("just started".to_string()),
                updated_at: now,
            };
            agents.push(info.clone());
            info
        };
        self.write_json(self.path(AGENTS_FILE), &agents)?;
        Ok(updated)
    }

    pub fn stop_agent(&self, agent_id: &str) -> Result<AgentRuntimeInfo, CliBackendError> {
        let mut agents =
            self.read_or_default(self.path(AGENTS_FILE), Vec::<AgentRuntimeInfo>::new())?;
        let agent = agents
            .iter_mut()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| CliBackendError::NotFound {
                kind: "Agent",
                id: agent_id.to_string(),
            })?;
        agent.status = "stopped".to_string();
        agent.uptime = None;
        agent.updated_at = now_utc_string();
        let snapshot = agent.clone();
        self.write_json(self.path(AGENTS_FILE), &agents)?;
        Ok(snapshot)
    }

    pub fn restart_agent(
        &self,
        agent_id: &str,
        config_path: Option<&Path>,
    ) -> Result<AgentRuntimeInfo, CliBackendError> {
        let _ = self.stop_agent(agent_id)?;
        self.start_agent(agent_id, config_path, false)
    }

    pub fn list_plugins(&self, installed_only: bool) -> Result<Vec<PluginInfo>, CliBackendError> {
        let plugins = self.read_or_default(self.path(PLUGINS_FILE), default_plugins())?;
        if installed_only {
            Ok(plugins.into_iter().filter(|p| p.installed).collect())
        } else {
            Ok(plugins)
        }
    }

    pub fn get_plugin(&self, name: &str) -> Result<PluginInfo, CliBackendError> {
        self.list_plugins(false)?
            .into_iter()
            .find(|plugin| plugin.name == name)
            .ok_or_else(|| CliBackendError::NotFound {
                kind: "Plugin",
                id: name.to_string(),
            })
    }

    pub fn uninstall_plugin(&self, name: &str) -> Result<PluginInfo, CliBackendError> {
        let mut plugins = self.read_or_default(self.path(PLUGINS_FILE), default_plugins())?;
        let plugin = plugins
            .iter_mut()
            .find(|plugin| plugin.name == name)
            .ok_or_else(|| CliBackendError::NotFound {
                kind: "Plugin",
                id: name.to_string(),
            })?;
        plugin.installed = false;
        let snapshot = plugin.clone();
        self.write_json(self.path(PLUGINS_FILE), &plugins)?;
        Ok(snapshot)
    }

    pub fn list_tools(&self, enabled_only: bool) -> Result<Vec<ToolInfo>, CliBackendError> {
        let tools = self.read_or_default(self.path(TOOLS_FILE), default_tools())?;
        if enabled_only {
            Ok(tools.into_iter().filter(|tool| tool.enabled).collect())
        } else {
            Ok(tools)
        }
    }

    pub fn get_tool(&self, name: &str) -> Result<ToolInfo, CliBackendError> {
        self.list_tools(false)?
            .into_iter()
            .find(|tool| tool.name == name)
            .ok_or_else(|| CliBackendError::NotFound {
                kind: "Tool",
                id: name.to_string(),
            })
    }

    pub fn list_sessions(
        &self,
        agent_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionInfo>, CliBackendError> {
        let sessions = self.read_or_default(self.path(SESSIONS_FILE), Vec::<SessionInfo>::new())?;
        let mut filtered: Vec<SessionInfo> = if let Some(agent) = agent_id {
            sessions
                .into_iter()
                .filter(|session| session.agent_id == agent)
                .collect()
        } else {
            sessions
        };
        if let Some(max) = limit {
            filtered.truncate(max);
        }
        Ok(filtered)
    }

    pub fn get_session(&self, session_id: &str) -> Result<SessionInfo, CliBackendError> {
        self.list_sessions(None, None)?
            .into_iter()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| CliBackendError::NotFound {
                kind: "Session",
                id: session_id.to_string(),
            })
    }

    pub fn delete_session(&self, session_id: &str) -> Result<(), CliBackendError> {
        let mut sessions =
            self.read_or_default(self.path(SESSIONS_FILE), Vec::<SessionInfo>::new())?;
        let before = sessions.len();
        sessions.retain(|session| session.session_id != session_id);
        if before == sessions.len() {
            return Err(CliBackendError::NotFound {
                kind: "Session",
                id: session_id.to_string(),
            });
        }
        self.write_json(self.path(SESSIONS_FILE), &sessions)?;
        Ok(())
    }

    fn path(&self, file: &str) -> PathBuf {
        self.root.join(file)
    }

    fn read_or_default<T>(&self, path: PathBuf, default_value: T) -> Result<T, CliBackendError>
    where
        T: for<'de> Deserialize<'de> + Clone + Serialize,
    {
        if !path.exists() {
            self.write_json(path.clone(), &default_value)?;
            return Ok(default_value);
        }
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    fn write_json<T>(&self, path: PathBuf, value: &T) -> Result<(), CliBackendError>
    where
        T: Serialize,
    {
        let data = serde_json::to_string_pretty(value)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

fn default_plugins() -> Vec<PluginInfo> {
    vec![
        PluginInfo {
            name: "http-server".to_string(),
            version: "0.1.0".to_string(),
            description: "HTTP server plugin for exposing agents via REST API".to_string(),
            author: "MoFA Team".to_string(),
            repository: Some("https://github.com/mofa-org/mofa".to_string()),
            license: Some("MIT".to_string()),
            installed: true,
        },
        PluginInfo {
            name: "postgres-persistence".to_string(),
            version: "0.1.0".to_string(),
            description: "PostgreSQL persistence plugin for session storage".to_string(),
            author: "MoFA Team".to_string(),
            repository: Some("https://github.com/mofa-org/mofa".to_string()),
            license: Some("MIT".to_string()),
            installed: true,
        },
        PluginInfo {
            name: "web-scraper".to_string(),
            version: "0.2.0".to_string(),
            description: "Web scraping tool for content extraction".to_string(),
            author: "Community".to_string(),
            repository: None,
            license: None,
            installed: false,
        },
    ]
}

fn default_tools() -> Vec<ToolInfo> {
    all_builtin_tools()
        .into_iter()
        .map(|tool| ToolInfo {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            version: "builtin".to_string(),
            enabled: true,
        })
        .collect()
}

fn now_utc_string() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn backend() -> (TempDir, CliBackend) {
        let temp = TempDir::new().unwrap();
        let backend = CliBackend::with_root(temp.path().join(BACKEND_DIR));
        std::fs::create_dir_all(&backend.root).unwrap();
        (temp, backend)
    }

    #[test]
    fn test_agent_start_stop_round_trip() {
        let (_temp, backend) = backend();
        let started = backend.start_agent("agent-1", None, true).unwrap();
        assert_eq!(started.status, "running");
        assert!(started.daemon);

        let stopped = backend.stop_agent("agent-1").unwrap();
        assert_eq!(stopped.status, "stopped");
    }

    #[test]
    fn test_plugin_uninstall_persists() {
        let (_temp, backend) = backend();
        let plugin = backend.uninstall_plugin("http-server").unwrap();
        assert!(!plugin.installed);
        let fetched = backend.get_plugin("http-server").unwrap();
        assert!(!fetched.installed);
    }

    #[test]
    fn test_builtin_tools_seeded() {
        let (_temp, backend) = backend();
        let tools = backend.list_tools(false).unwrap();
        assert!(!tools.is_empty());
        assert!(tools.iter().any(|tool| tool.name == "calculator"));
    }
}
