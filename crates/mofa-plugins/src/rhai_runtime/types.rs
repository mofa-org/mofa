//! Rhai Plugin Types
//!
//! Type definitions for Rhai runtime plugins

use rhai::Dynamic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Plugin Metadata
// ============================================================================

/// Plugin metadata extracted from script
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: Option<String>,
    /// Plugin homepage
    pub homepage: Option<String>,
    /// Required capabilities
    pub capabilities: Vec<String>,
    /// Plugin dependencies
    pub dependencies: Vec<String>,
    /// Creation time
    pub created_at: u64,
    /// Last modified time
    pub modified_at: u64,
    /// Script path (if loaded from file)
    pub path: Option<PathBuf>,
}

impl Default for PluginMetadata {
    fn default() -> Self {
        let now = chrono::Utc::now().timestamp() as u64;
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: "unknown".to_string(),
            version: "0.0.0".to_string(),
            description: "".to_string(),
            author: None,
            homepage: None,
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            created_at: now,
            modified_at: now,
            path: None,
        }
    }
}

impl PluginMetadata {
    /// Create new plugin metadata with minimal required fields
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            ..Default::default()
        }
    }

    /// Load metadata from Rhai global variables
    pub fn from_rhai_vars(_vars: &HashMap<String, Dynamic>) -> RhaiPluginResult<Self> {
        // Simplified for now - will implement properly later
        Ok(Self::default())
    }
}

// ============================================================================
// Plugin Error Type
// ============================================================================

/// Rhai plugin error type
#[derive(thiserror::Error, Debug)]
pub enum RhaiPluginError {
    /// Script compilation error
    #[error("Compilation error: {0}")]
    CompilationError(String),

    /// Script execution error
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// Missing required function
    #[error("Missing required function: {0}")]
    MissingFunction(String),

    /// Invalid metadata format
    #[error("Invalid metadata format: {0}")]
    InvalidMetadata(String),

    /// File IO error
    #[error("File IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Rhai engine error
    #[error("Rhai error: {0}")]
    RhaiError(String),

    /// JSON ser/de error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Other error
    #[error("Other: {0}")]
    Other(String),
}

impl From<mofa_extra::rhai::RhaiError> for RhaiPluginError {
    fn from(err: mofa_extra::rhai::RhaiError) -> Self {
        RhaiPluginError::RhaiError(err.to_string())
    }
}

/// Rhai plugin result type
pub type RhaiPluginResult<T = ()> = Result<T, RhaiPluginError>;

// ============================================================================
// Plugin Capabilities
// ============================================================================

/// Plugin capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginCapability {
    /// Basic execution capability
    Execution,
    /// File system access
    FileSystem,
    /// Network access
    Network,
    /// System command execution
    SystemCommand,
    /// Event subscription
    EventSubscription,
    /// Plugin management
    PluginManagement,
}

impl std::fmt::Display for PluginCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginCapability::Execution => write!(f, "execution"),
            PluginCapability::FileSystem => write!(f, "file_system"),
            PluginCapability::Network => write!(f, "network"),
            PluginCapability::SystemCommand => write!(f, "system_command"),
            PluginCapability::EventSubscription => write!(f, "event_subscription"),
            PluginCapability::PluginManagement => write!(f, "plugin_management"),
        }
    }
}
