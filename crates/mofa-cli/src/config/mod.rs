//! Configuration handling module
//!
//! Provides configuration file discovery, loading, validation, and merging.

pub mod loader;
pub mod merge;
pub mod validator;

pub use loader::ConfigLoader;
pub use validator::{ConfigValidationResult, ConfigValidator};

use crate::CliError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration structure for MoFA agents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentConfig {
    /// Agent identity
    pub agent: AgentIdentity,
    /// LLM configuration
    pub llm: Option<LlmConfig>,
    /// Runtime configuration
    pub runtime: Option<RuntimeConfig>,
    /// Additional node-specific configuration
    #[serde(flatten)]
    pub node_config: HashMap<String, serde_json::Value>,
}

/// Agent identity configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentIdentity {
    /// Unique agent ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Agent description
    pub description: Option<String>,
    /// Agent capabilities
    pub capabilities: Option<Vec<String>>,
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LlmConfig {
    /// LLM provider type
    pub provider: String,
    /// Model name
    pub model: String,
    /// API key (supports ${ENV_VAR} syntax)
    pub api_key: Option<String>,
    /// Custom base URL
    pub base_url: Option<String>,
    /// Generation temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// System prompt
    pub system_prompt: Option<String>,
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeConfig {
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: Option<usize>,
    /// Default timeout in seconds
    pub default_timeout_secs: Option<u64>,
    /// Enable Dora runtime
    pub dora_enabled: Option<bool>,
    /// Persistence configuration
    pub persistence: Option<PersistenceConfig>,
}

/// Persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistenceConfig {
    /// Enable persistence
    pub enabled: Option<bool>,
    /// Database URL
    pub database_url: Option<String>,
    /// Session TTL in seconds
    pub session_ttl_secs: Option<u64>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent: AgentIdentity {
                id: "agent-001".to_string(),
                name: "MyAgent".to_string(),
                description: None,
                capabilities: None,
            },
            llm: None,
            runtime: None,
            node_config: HashMap::new(),
        }
    }
}

impl AgentConfig {
    /// Load configuration from a file
    pub fn from_file<P: Into<PathBuf>>(path: P) -> Result<Self, CliError> {
        let loader = ConfigLoader::new();
        loader.load(path.into())
    }

    /// Load configuration with merge from multiple sources
    pub fn load_merged(
        file_path: Option<PathBuf>,
        env_prefix: Option<&str>,
    ) -> Result<Self, CliError> {
        let loader = ConfigLoader::new();
        loader.load_merged(file_path, env_prefix)
    }

    /// Validate the configuration
    pub fn validate(&self) -> ConfigValidationResult {
        let validator = ConfigValidator::new();
        validator.validate(self)
    }

    /// Resolve environment variables in configuration values
    pub fn resolve_env_vars(&mut self) -> Result<(), CliError> {
        // Resolve API key
        if let Some(ref mut llm) = self.llm
            && let Some(ref api_key) = llm.api_key
        {
            llm.api_key = Some(resolve_env_value(api_key)?);
        }

        // Resolve database URL
        if let Some(ref mut runtime) = self.runtime
            && let Some(ref mut persistence) = runtime.persistence
            && let Some(ref database_url) = persistence.database_url
        {
            persistence.database_url = Some(resolve_env_value(database_url)?);
        }

        Ok(())
    }
}

/// Resolve environment variable references in a string value
/// Supports ${VAR} and $VAR syntax
fn resolve_env_value(value: &str) -> Result<String, CliError> {
    let trimmed = value.trim();

    // Check for ${VAR} syntax
    if trimmed.starts_with("${") && trimmed.ends_with('}') {
        let var_name = &trimmed[2..trimmed.len() - 1];
        return std::env::var(var_name)
            .map_err(|_| CliError::ConfigError(format!("Environment variable '{}' not found", var_name)));
    }

    // Check for $VAR syntax
    if let Some(var_name) = trimmed.strip_prefix('$') {
        return std::env::var(var_name)
            .map_err(|_| CliError::ConfigError(format!("Environment variable '{}' not found", var_name)));
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_value() {
        unsafe { std::env::set_var("TEST_VAR", "test_value") };

        assert_eq!(resolve_env_value("${TEST_VAR}").unwrap(), "test_value");
        assert_eq!(resolve_env_value("$TEST_VAR").unwrap(), "test_value");
        assert_eq!(resolve_env_value("plain_value").unwrap(), "plain_value");

        resolve_env_value("$NONEXISTENT_VAR").unwrap_err();
        resolve_env_value("${NONEXISTENT_VAR}").unwrap_err();

        unsafe { std::env::remove_var("TEST_VAR") };
    }
}
