use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Stopped,
    Running,
    Error,
}

impl AgentStatus {
    pub fn display(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Error => "error",
        }
    }

    pub fn colored_display(&self) -> String {
        use colored::Colorize;
        match self {
            Self::Running => "running".green().to_string(),
            Self::Stopped => "stopped".yellow().to_string(),
            Self::Error => "error".red().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub pid: Option<u32>,
    pub config_path: Option<String>,
    pub start_time: Option<u64>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl AgentState {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            status: AgentStatus::Stopped,
            pid: None,
            config_path: None,
            start_time: None,
            error: None,
            metadata: HashMap::new(),
        }
    }

    pub fn start_running(&mut self, pid: u32, config_path: Option<String>) {
        self.status = AgentStatus::Running;
        self.pid = Some(pid);
        self.config_path = config_path;
        self.start_time = Some(now_secs());
        self.error = None;
    }

    pub fn stop(&mut self) {
        self.status = AgentStatus::Stopped;
        self.pid = None;
        self.start_time = None;
    }

    pub fn set_error(&mut self, message: String) {
        self.status = AgentStatus::Error;
        self.pid = None;
        self.error = Some(message);
    }

    pub fn uptime_string(&self) -> Option<String> {
        let started = self.start_time?;
        let elapsed = now_secs().saturating_sub(started);
        Some(format_duration(elapsed))
    }
}

/// File-based registry persisting agent state to `$MOFA_HOME/agents/`.
pub struct AgentRegistry {
    dir: PathBuf,
}

impl AgentRegistry {
    pub fn new() -> Result<Self> {
        let dir = mofa_home().join("agents");
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create agents dir: {}", dir.display()))?;
        Ok(Self { dir })
    }

    pub fn save_agent(&self, agent: &AgentState) -> Result<()> {
        let path = self.agent_path(&agent.id);
        let json = serde_json::to_string_pretty(agent)?;
        fs::write(&path, json)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn load_agent(&self, id: &str) -> Result<Option<AgentState>> {
        let path = self.agent_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let json = fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&json)?))
    }

    pub fn delete_agent(&self, id: &str) -> Result<()> {
        let path = self.agent_path(id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn exists(&self, id: &str) -> Result<bool> {
        Ok(self.agent_path(id).exists())
    }

    pub fn list_all(&self) -> Result<Vec<AgentState>> {
        self.load_entries(|_| true)
    }

    pub fn list_running(&self) -> Result<Vec<AgentState>> {
        self.load_entries(|s| s.status == AgentStatus::Running)
    }

    fn agent_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", id))
    }

    fn load_entries<F>(&self, predicate: F) -> Result<Vec<AgentState>>
    where
        F: Fn(&AgentState) -> bool,
    {
        let mut results = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let json = fs::read_to_string(&path)?;
            if let Ok(state) = serde_json::from_str::<AgentState>(&json) {
                if predicate(&state) {
                    results.push(state);
                }
            }
        }
        Ok(results)
    }
}

fn mofa_home() -> PathBuf {
    if let Ok(val) = std::env::var("MOFA_HOME") {
        PathBuf::from(val)
    } else {
        dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mofa")
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
