//! LLM tool executor adapters for the SDK.

use mofa_foundation::config::AgentYamlConfig;
use mofa_foundation::llm::{LLMError, LLMResult, Tool, ToolExecutor};
use mofa_foundation::llm::LLMAgentBuilder;
use mofa_plugins::tools::create_builtin_tool_plugin_with_config;
use mofa_plugins::{ToolCall, ToolDefinition, ToolPlugin};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Adapter that exposes a ToolPlugin as an LLM ToolExecutor.
///
/// This lets LLM agents discover tools directly from a ToolPlugin without
/// manually constructing tool definitions.
pub struct ToolPluginExecutor {
    tool_plugin: Arc<RwLock<ToolPlugin>>,
    cached_tools: Arc<RwLock<Option<Vec<Tool>>>>,
}

impl ToolPluginExecutor {
    /// Create a new adapter from an owned ToolPlugin.
    pub fn new(tool_plugin: ToolPlugin) -> Self {
        Self {
            tool_plugin: Arc::new(RwLock::new(tool_plugin)),
            cached_tools: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new adapter from a shared ToolPlugin handle.
    pub fn with_shared(tool_plugin: Arc<RwLock<ToolPlugin>>) -> Self {
        Self {
            tool_plugin,
            cached_tools: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the underlying ToolPlugin handle.
    pub fn tool_plugin(&self) -> Arc<RwLock<ToolPlugin>> {
        self.tool_plugin.clone()
    }

    /// Clear the cached tool list so it will be refreshed on next access.
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cached_tools.write().await;
        *cache = None;
    }

    /// Refresh and return the latest tool list from the plugin.
    pub async fn refresh_tools(&self) -> LLMResult<Vec<Tool>> {
        let plugin = self.tool_plugin.read().await;
        let tools = plugin
            .list_tools()
            .into_iter()
            .map(|def| Self::definition_to_tool(&def))
            .collect::<Vec<_>>();

        let mut cache = self.cached_tools.write().await;
        *cache = Some(tools.clone());
        Ok(tools)
    }

    fn definition_to_tool(def: &ToolDefinition) -> Tool {
        Tool::function(&def.name, &def.description, def.parameters.clone())
    }
}

/// Build a built-in ToolPlugin from `agent.yml` tools configuration.
///
/// Only `enabled: true` tools are included in the generated config map.
pub fn builtin_tool_plugin_from_agent_yaml(
    plugin_id: &str,
    config: &AgentYamlConfig,
) -> LLMResult<ToolPlugin> {
    let mut tool_configs: HashMap<String, serde_json::Value> = HashMap::new();

    if let Some(tools) = &config.tools {
        for tool in tools.iter().filter(|t| t.enabled) {
            tool_configs.insert(
                tool.name.clone(),
                serde_json::to_value(&tool.config).map_err(|e| {
                    LLMError::Other(format!(
                        "Failed to serialize tool config for '{}': {}",
                        tool.name, e
                    ))
                })?,
            );
        }
    }

    create_builtin_tool_plugin_with_config(plugin_id, &tool_configs)
        .map_err(|e| LLMError::Other(format!("Failed to create builtin ToolPlugin: {}", e)))
}

/// Create an LLM ToolExecutor from `agent.yml` tools configuration.
pub fn tool_executor_from_agent_yaml(
    plugin_id: &str,
    config: &AgentYamlConfig,
) -> LLMResult<Arc<dyn ToolExecutor>> {
    let plugin = builtin_tool_plugin_from_agent_yaml(plugin_id, config)?;
    Ok(Arc::new(ToolPluginExecutor::new(plugin)))
}

/// Build an `LLMAgentBuilder` from yaml config and attach configured built-in tools.
pub fn llm_builder_from_yaml_with_builtin_tools(config: AgentYamlConfig) -> LLMResult<LLMAgentBuilder> {
    let mut builder = LLMAgentBuilder::from_yaml_config(config.clone())?;
    if config.tools.as_ref().is_some_and(|tools| tools.iter().any(|t| t.enabled)) {
        let executor = tool_executor_from_agent_yaml("builtin-tools", &config)?;
        builder = builder.with_tool_executor(executor);
    }
    Ok(builder)
}

/// Build an `LLMAgentBuilder` from config file and attach configured built-in tools.
pub fn llm_builder_from_config_file_with_builtin_tools(
    path: impl AsRef<std::path::Path>,
) -> LLMResult<LLMAgentBuilder> {
    let config = AgentYamlConfig::from_file(path)
        .map_err(|e| LLMError::ConfigError(format!("Failed to load config file: {}", e)))?;
    llm_builder_from_yaml_with_builtin_tools(config)
}

#[async_trait::async_trait]
impl ToolExecutor for ToolPluginExecutor {
    async fn execute(&self, name: &str, arguments: &str) -> LLMResult<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| LLMError::Other(format!("Failed to parse arguments: {}", e)))?;

        let call = ToolCall {
            call_id: uuid::Uuid::now_v7().to_string(),
            name: name.to_string(),
            arguments: args,
        };

        let mut plugin = self.tool_plugin.write().await;
        let result = plugin
            .call_tool(call)
            .await
            .map_err(|e| LLMError::Other(format!("Tool execution failed: {}", e)))?;

        if result.success {
            serde_json::to_string(&result.result)
                .map_err(|e| LLMError::Other(format!("Failed to serialize result: {}", e)))
        } else {
            Err(LLMError::Other(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn available_tools(&self) -> LLMResult<Vec<Tool>> {
        let cached = { self.cached_tools.read().await.clone() };
        if let Some(tools) = cached {
            return Ok(tools);
        }

        self.refresh_tools().await
    }
}
