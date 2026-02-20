//! Internal events for TUI component communication

use std::fmt;

/// Events that can be sent between TUI components
#[derive(Debug, Clone)]
pub enum AppEvent {
    // Navigation events
    SwitchView(View),
    ShowCommandPalette,
    HideOverlay,

    // Agent events
    AgentListUpdated,
    AgentStatusChanged(String, AgentStatus),
    StartAgent(String),
    StopAgent(String),
    RestartAgent(String),
    CreateAgent,

    // Session events
    SessionListUpdated,
    SessionSelected(String),
    DeleteSession(String),
    ExportSession(String),

    // Config events
    ConfigUpdated,
    ConfigValueChanged(String, String),

    // Plugin events
    PluginListUpdated,
    PluginInfo(String),
    UninstallPlugin(String),

    // Draw trigger
    Draw,

    // Exit
    Exit(ExitMode),
}

/// Available views in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Dashboard,
    Agents,
    Sessions,
    Config,
    Plugins,
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            View::Dashboard => write!(f, "Dashboard"),
            View::Agents => write!(f, "Agents"),
            View::Sessions => write!(f, "Sessions"),
            View::Config => write!(f, "Config"),
            View::Plugins => write!(f, "Plugins"),
        }
    }
}

/// Agent runtime status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStatus::Starting => write!(f, "Starting"),
            AgentStatus::Running => write!(f, "Running"),
            AgentStatus::Stopping => write!(f, "Stopping"),
            AgentStatus::Stopped => write!(f, "Stopped"),
            AgentStatus::Error => write!(f, "Error"),
        }
    }
}

impl AgentStatus {
    /// Returns the symbol for this status
    pub fn symbol(self) -> &'static str {
        match self {
            AgentStatus::Starting => "",
            AgentStatus::Running => "",
            AgentStatus::Stopping => "",
            AgentStatus::Stopped => "",
            AgentStatus::Error => "",
        }
    }
}

/// Exit mode for the TUI
#[derive(Debug, Clone)]
pub enum ExitMode {
    Clean,
    Error(String),
}
