//! Fluent builder for composing `AgentExecutor` instances.
//!
//! This module provides three types that together implement the agent
//! composition layer described in the AgentForge design:
//!
//! - [`AgentBuilder`]: fluent API for constructing an [`AgentExecutor`] from
//!   its components (LLM provider, tools, system prompt, workspace, config).
//! - [`AgentProfile`]: TOML/YAML-serializable configuration for an agent that
//!   can be stored in a file, version-controlled, and loaded at runtime.
//! - [`AgentRegistry`]: runtime registry mapping names to running
//!   [`AgentExecutor`] instances so orchestrators can look up agents without
//!   holding direct references.
//!
//! # Example
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use mofa_foundation::agent::builder::{AgentBuilder, AgentProfile, AgentRegistry};
//!
//! // Build from code
//! let agent = AgentBuilder::new()
//!     .name("analyst")
//!     .system_prompt("You are a financial analyst.")
//!     .llm(llm_provider)
//!     .with_tool(HttpTool::new())
//!     .model("gpt-4o")
//!     .build()
//!     .await?;
//!
//! // Build from a TOML profile
//! let profile = AgentProfile::from_toml(include_str!("analyst.toml"))?;
//! let agent2 = profile.to_builder().llm(llm_provider).build().await?;
//!
//! // Register both in a registry
//! let mut registry = AgentRegistry::new();
//! registry.register("analyst", agent);
//! registry.register("analyst2", agent2);
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use mofa_kernel::agent::components::tool::{
    DynTool, LLMTool, Tool, ToolExt, ToolInput, ToolMetadata, ToolResult,
};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::agent::types::LLMProvider;

use crate::agent::executor::{AgentExecutor, AgentExecutorConfig};

// ============================================================================
// ============================================================================
// AgentBuilder
// ============================================================================

/// Fluent builder for constructing [`AgentExecutor`] instances.
///
/// Validates that all required components are present at [`build`](AgentBuilder::build)
/// time and returns a descriptive error if mandatory fields are missing.
///
/// # Required fields
/// - `llm`: an [`Arc<dyn LLMProvider>`] must be supplied via [`.llm()`](AgentBuilder::llm).
///
/// # Optional fields
/// All other fields have sensible defaults (see individual methods).
pub struct AgentBuilder {
    /// Agent display name
    pub(crate) name: String,
    /// Human-readable description
    pub(crate) description: Option<String>,
    /// Inline system prompt. Overrides the workspace bootstrap files when set.
    pub(crate) system_prompt: Option<String>,
    /// LLM provider (required)
    llm: Option<Arc<dyn LLMProvider>>,
    /// Tools to register on the executor (dynamic tool objects)
    tools: Vec<Arc<dyn DynTool>>,
    /// Executor configuration (model, temperature, iterations, …)
    pub(crate) config: AgentExecutorConfig,
    /// Workspace directory for sessions and context files.
    /// Defaults to `.mofa` inside the current working directory.
    workspace: Option<PathBuf>,
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            name: "agent".to_string(),
            description: None,
            system_prompt: None,
            llm: None,
            tools: Vec::new(),
            config: AgentExecutorConfig::default(),
            workspace: None,
        }
    }

    /// Set the agent display name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set a human-readable description for the agent.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set an inline system prompt.
    ///
    /// When provided this overrides the workspace-based bootstrap files so the
    /// agent does not need a populated workspace directory to produce a system
    /// prompt.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set the LLM provider. **Required** — `build()` returns an error if omitted.
    pub fn llm(mut self, llm: Arc<dyn LLMProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Register a tool on the resulting executor.
    ///
    /// Can be called multiple times to register several tools.
    pub fn with_tool<T>(mut self, tool: T) -> Self
    where
        T: Tool<serde_json::Value, serde_json::Value> + Send + Sync + 'static,
    {
        self.tools.push(tool.into_dynamic());
        self
    }

    /// Set the workspace directory for sessions and context files.
    ///
    /// Defaults to `.mofa` inside the current working directory when omitted.
    pub fn workspace(mut self, path: impl AsRef<Path>) -> Self {
        self.workspace = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the default LLM model identifier (e.g. `"gpt-4o"`).
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.default_model = Some(model.into());
        self
    }

    /// Set the maximum number of tool-call iterations per message.
    pub fn max_iterations(mut self, n: usize) -> Self {
        self.config.max_iterations = n;
        self
    }

    /// Set the LLM sampling temperature.
    pub fn temperature(mut self, temp: f32) -> Self {
        self.config.temperature = Some(temp);
        self
    }

    /// Set the maximum number of tokens for LLM responses.
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.config.max_tokens = Some(tokens);
        self
    }

    /// Build the [`AgentExecutor`].
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::ConfigError`] if no LLM provider was supplied.
    /// Returns [`AgentError::IoError`] if the workspace directory cannot be created.
    pub async fn build(self) -> AgentResult<AgentExecutor> {
        let llm = self.llm.ok_or_else(|| {
            AgentError::ConfigError(
                "AgentBuilder requires an LLM provider; call .llm() before .build()".to_string(),
            )
        })?;

        let workspace = match self.workspace {
            Some(ws) => ws,
            None => std::env::current_dir()
                .map_err(|e| {
                    AgentError::IoError(format!("Cannot determine current directory: {}", e))
                })?
                .join(".mofa"),
        };

        tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
            AgentError::IoError(format!("Failed to create workspace directory: {}", e))
        })?;

        let mut executor = AgentExecutor::with_config(llm, &workspace, self.config).await?;

        // Inject inline system prompt if provided so the agent works without
        // any workspace bootstrap files.
        if let Some(prompt) = self.system_prompt {
            executor.context().write().await.set_inline_prompt(prompt);
        }

        for tool in self.tools {
            executor.register_tool(tool).await?;
        }

        Ok(executor)
    }
}

// ============================================================================
// AgentProfile
// ============================================================================

/// TOML/YAML-serializable static configuration for an agent.
///
/// An `AgentProfile` captures everything needed to describe an agent except
/// the runtime LLM provider and workspace path.  It can be stored in a file,
/// version-controlled, and loaded at startup to drive [`AgentBuilder`].
///
/// # File format
///
/// TOML example:
/// ```toml
/// name = "analyst"
/// system_prompt = "You are a financial analyst."
/// model = "gpt-4o"
/// tool_names = ["http", "file_read"]
/// max_iterations = 5
/// temperature = 0.3
/// max_tokens = 2048
/// ```
///
/// YAML example:
/// ```yaml
/// name: researcher
/// system_prompt: "You are a research assistant."
/// model: gpt-4o-mini
/// tool_names:
///   - http
///   - file_read
/// max_iterations: 8
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Agent display name
    pub name: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Inline system prompt
    pub system_prompt: Option<String>,
    /// LLM model identifier (e.g. `"gpt-4o"`)
    pub model: Option<String>,
    /// Names of tools to register (resolved by the caller against a tool registry)
    #[serde(default)]
    pub tool_names: Vec<String>,
    /// Maximum tool-call iterations per message
    pub max_iterations: Option<usize>,
    /// LLM sampling temperature
    pub temperature: Option<f32>,
    /// Maximum tokens for LLM responses
    pub max_tokens: Option<u32>,
}

impl AgentProfile {
    /// Parse a profile from TOML text.
    pub fn from_toml(content: &str) -> AgentResult<Self> {
        toml::from_str(content).map_err(|e| {
            AgentError::SerializationError(format!("Failed to parse TOML profile: {}", e))
        })
    }

    /// Parse a profile from YAML text.
    pub fn from_yaml(content: &str) -> AgentResult<Self> {
        serde_yaml::from_str(content).map_err(|e| {
            AgentError::SerializationError(format!("Failed to parse YAML profile: {}", e))
        })
    }

    /// Load a profile from a `.toml`, `.yaml`, or `.yml` file.
    pub async fn from_file(path: impl AsRef<Path>) -> AgentResult<Self> {
        let path = path.as_ref();
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            AgentError::IoError(format!("Failed to read profile file {:?}: {}", path, e))
        })?;

        match path.extension().and_then(|s| s.to_str()) {
            Some("toml") => Self::from_toml(&content),
            Some("yaml") | Some("yml") => Self::from_yaml(&content),
            ext => Err(AgentError::ConfigError(format!(
                "Unsupported profile format: {:?}. Use .toml or .yaml",
                ext
            ))),
        }
    }

    /// Convert this profile to an [`AgentBuilder`].
    ///
    /// The LLM provider (and optionally the workspace) must still be supplied
    /// on the returned builder before calling `.build()`.
    pub fn to_builder(&self) -> AgentBuilder {
        let mut builder = AgentBuilder::new().name(&self.name);

        if let Some(ref desc) = self.description {
            builder = builder.description(desc);
        }
        if let Some(ref prompt) = self.system_prompt {
            builder = builder.system_prompt(prompt);
        }
        if let Some(ref model) = self.model {
            builder = builder.model(model);
        }
        if let Some(n) = self.max_iterations {
            builder = builder.max_iterations(n);
        }
        if let Some(t) = self.temperature {
            builder = builder.temperature(t);
        }
        if let Some(tok) = self.max_tokens {
            builder = builder.max_tokens(tok);
        }

        builder
    }
}

// ============================================================================
// AgentRegistry
// ============================================================================

/// Runtime registry mapping names to running [`AgentExecutor`] instances.
///
/// Orchestrators use the registry to route tasks to agents by name without
/// holding direct references to specific agent instances.  This decouples the
/// orchestration logic from the agent construction details.
pub struct AgentRegistry {
    agents: HashMap<String, AgentExecutor>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register a named agent.
    ///
    /// Overwrites any existing agent registered under the same name.
    pub fn register(&mut self, name: impl Into<String>, agent: AgentExecutor) {
        self.agents.insert(name.into(), agent);
    }

    /// Look up an agent by name (immutable).
    pub fn get(&self, name: &str) -> Option<&AgentExecutor> {
        self.agents.get(name)
    }

    /// Look up an agent by name (mutable) for calling `process_message`.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut AgentExecutor> {
        self.agents.get_mut(name)
    }

    /// List all registered agent names.
    pub fn list(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }

    /// Remove and return a named agent.
    pub fn remove(&mut self, name: &str) -> Option<AgentExecutor> {
        self.agents.remove(name)
    }

    /// Return the number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Return `true` if the registry contains no agents.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_profile_from_toml() {
        let toml = r#"
name = "analyst"
system_prompt = "You are a financial analyst."
model = "gpt-4o"
tool_names = ["http", "file_read"]
max_iterations = 5
temperature = 0.3
max_tokens = 2048
"#;
        let profile = AgentProfile::from_toml(toml).unwrap();
        assert_eq!(profile.name, "analyst");
        assert_eq!(profile.model.as_deref(), Some("gpt-4o"));
        assert_eq!(profile.tool_names, vec!["http", "file_read"]);
        assert_eq!(profile.max_iterations, Some(5));
        assert_eq!(profile.temperature, Some(0.3));
        assert_eq!(profile.max_tokens, Some(2048));
    }

    #[test]
    fn test_agent_profile_from_yaml() {
        let yaml = "
name: researcher
system_prompt: \"You are a research assistant.\"
model: gpt-4o-mini
tool_names:
  - http
  - file_read
max_iterations: 8
";
        let profile = AgentProfile::from_yaml(yaml).unwrap();
        assert_eq!(profile.name, "researcher");
        assert_eq!(profile.model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(profile.max_iterations, Some(8));
        assert_eq!(profile.tool_names, vec!["http", "file_read"]);
    }

    #[test]
    fn test_profile_to_builder_propagates_fields() {
        let profile = AgentProfile {
            name: "coder".to_string(),
            description: None,
            system_prompt: Some("You write Rust.".to_string()),
            model: Some("gpt-4o".to_string()),
            tool_names: vec![],
            max_iterations: Some(3),
            temperature: Some(0.1),
            max_tokens: None,
        };
        let builder = profile.to_builder();
        assert_eq!(builder.name, "coder");
        assert_eq!(builder.system_prompt.as_deref(), Some("You write Rust."));
        assert_eq!(builder.config.default_model.as_deref(), Some("gpt-4o"));
        assert_eq!(builder.config.max_iterations, 3);
        assert_eq!(builder.config.temperature, Some(0.1));
    }

    #[test]
    fn test_profile_to_builder_preserves_defaults_when_fields_absent() {
        let profile = AgentProfile {
            name: "minimal".to_string(),
            description: None,
            system_prompt: None,
            model: None,
            tool_names: vec![],
            max_iterations: None,
            temperature: None,
            max_tokens: None,
        };
        let builder = profile.to_builder();
        assert_eq!(builder.name, "minimal");
        assert!(builder.system_prompt.is_none());
        assert!(builder.config.default_model.is_none());
        // Default max_iterations should be preserved
        assert_eq!(
            builder.config.max_iterations,
            AgentExecutorConfig::default().max_iterations
        );
    }

    #[test]
    fn test_registry_empty_state() {
        let registry = AgentRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.list().is_empty());
        assert!(registry.get("anything").is_none());
    }

    #[tokio::test]
    async fn test_builder_fails_without_llm() {
        let result = AgentBuilder::new().build().await;
        assert!(result.is_err(), "expected Err when no LLM is set");
        if let Err(e) = result {
            assert!(
                matches!(e, AgentError::ConfigError(_)),
                "expected ConfigError, got {:?}",
                e
            );
        }
    }

    #[tokio::test]
    async fn test_builder_profile_roundtrip_toml() {
        let toml = r#"
name = "echo"
system_prompt = "Repeat everything."
model = "gpt-4o"
max_iterations = 2
temperature = 0.0
"#;
        let profile = AgentProfile::from_toml(toml).unwrap();
        let builder = profile.to_builder();
        // Verify fields survived the roundtrip without building (no LLM needed).
        assert_eq!(builder.name, "echo");
        assert_eq!(builder.system_prompt.as_deref(), Some("Repeat everything."));
        assert_eq!(builder.config.default_model.as_deref(), Some("gpt-4o"));
        assert_eq!(builder.config.max_iterations, 2);
        assert_eq!(builder.config.temperature, Some(0.0));
    }
}
