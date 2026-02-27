//! Prompt Template Plugin
//!
//! 提供基于插件的动态 Prompt 模板管理功能
//! Provides dynamic Prompt template management based on plugins
//!
//! # 示例
//! # Example
//!
//! ```rust,ignore
//! // 创建一个基于 Rhai 脚本的 Prompt 模板插件
//! // Create a Rhai script-based Prompt template plugin
//! let plugin = RhaiScriptPromptPlugin::new(Path::new("./prompts/"));
//!
//! // 添加到 Agent
//! // Add to Agent
//! agent.add_plugin(Box::new(plugin));
//! ```
use crate::prompt::{PromptRegistry, PromptTemplate};
use mofa_kernel::plugin::PluginResult;
use rhai::Engine;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Prompt 模板插件 trait
/// Prompt Template Plugin trait
#[async_trait::async_trait]
pub trait PromptTemplatePlugin: Send + Sync {
    /// 获取当前场景的 Prompt 模板
    /// Get the Prompt template for the current scenario
    async fn get_prompt_template(&self, scenario: &str) -> Option<Arc<PromptTemplate>>;

    /// 获取当前活动场景的模板
    /// Get the template of the currently active scenario
    async fn get_current_template(&self) -> Option<Arc<PromptTemplate>> {
        let active = self.get_active_scenario().await;
        self.get_prompt_template(&active).await
    }

    /// 获取当前活动场景
    /// Get the currently active scenario
    async fn get_active_scenario(&self) -> String;

    /// 设置当前活动的场景
    /// Set the currently active scenario
    async fn set_active_scenario(&self, scenario: &str);

    /// 获取所有可用的场景
    /// Get all available scenarios
    async fn get_available_scenarios(&self) -> Vec<String>;

    /// 刷新模板
    /// Refresh templates
    async fn refresh_templates(&self) -> PluginResult<()>;
}

/// 基于 Rhai 脚本的 Prompt 模板插件
/// Rhai script-based Prompt template plugin
pub struct RhaiScriptPromptPlugin {
    /// 脚本文件夹路径
    /// Script folder path
    script_path: PathBuf,
    /// Prompt 注册中心
    /// Prompt registry center
    registry: Arc<RwLock<PromptRegistry>>,
    /// 当前活动的场景
    /// Currently active scenario
    active_scenario: RwLock<String>,
}

impl RhaiScriptPromptPlugin {
    /// 创建新的 Rhai 脚本 Prompt 模板插件
    /// Create a new Rhai script Prompt template plugin
    pub fn new(script_path: impl Into<PathBuf>) -> Self {
        Self {
            script_path: script_path.into(),
            registry: Arc::new(RwLock::new(PromptRegistry::new())),
            active_scenario: RwLock::new("default".to_string()),
        }
    }

    /// 设置当前活动的场景
    /// Set the currently active scenario
    pub async fn set_active_scenario(&self, scenario: impl Into<String>) {
        let mut active = self.active_scenario.write().await;
        *active = scenario.into();
    }

    /// 获取当前活动场景的模板
    /// Get the template of the currently active scenario
    pub async fn get_current_template(&self) -> Option<Arc<PromptTemplate>> {
        let active = self.active_scenario.read().await;
        self.get_prompt_template(&active).await
    }

    /// 获取脚本文件夹路径
    /// Get script folder path
    pub fn script_path(&self) -> &PathBuf {
        &self.script_path
    }
}

#[async_trait::async_trait]
impl PromptTemplatePlugin for RhaiScriptPromptPlugin {
    async fn get_prompt_template(&self, scenario: &str) -> Option<Arc<PromptTemplate>> {
        let registry = self.registry.read().await;
        registry.get(scenario).cloned().ok().map(Arc::new)
    }

    async fn get_active_scenario(&self) -> String {
        let active = self.active_scenario.read().await;
        active.clone()
    }

    async fn set_active_scenario(&self, scenario: &str) {
        let mut active = self.active_scenario.write().await;
        *active = scenario.to_string();
    }

    async fn get_available_scenarios(&self) -> Vec<String> {
        let registry = self.registry.read().await;
        registry
            .list_ids()
            .into_iter()
            .map(|id| id.to_string())
            .collect()
    }

    async fn refresh_templates(&self) -> PluginResult<()> {
        use std::fs;

        let mut registry = self.registry.write().await;

        // Clear the registry to avoid duplicates
        registry.clear();

        // Check if the script path exists
        if !self.script_path.exists() {
            tracing::warn!("Script path does not exist: {:?}", self.script_path);
            return Ok(());
        }

        // Read all .rhai files from the directory
        let entries = fs::read_dir(&self.script_path)
            .map_err(|e| mofa_kernel::plugin::PluginError::Other(e.to_string()))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| mofa_kernel::plugin::PluginError::Other(e.to_string()))?;
            let path = entry.path();

            // Process only Rhai files
            if path.is_file() && path.extension().is_some_and(|ext| ext == "rhai") {
                tracing::info!("Loading prompt template from: {:?}", path);

                // Read the script content
                let script = fs::read_to_string(&path)
                    .map_err(|e| mofa_kernel::plugin::PluginError::Other(e.to_string()))?;

                // Create a Rhai engine
                let engine = Engine::new();

                // Wrap the script to return the template object
                let script = format!(
                    "
                    let template = {};
                    template
                ",
                    script
                );

                // Evaluate the script to get Rhai Dynamic object
                let template_dyn: rhai::Dynamic = match engine.eval(&script) {
                    Ok(obj) => obj,
                    Err(e) => {
                        tracing::warn!("Failed to evaluate Rhai script: {:?}, error: {}", path, e);
                        continue;
                    }
                };

                // Convert Dynamic to Map
                let template_obj = match template_dyn.as_map_ref() {
                    Ok(map) => map,
                    Err(_) => {
                        tracing::warn!("Rhai script did not return a Map: {:?}", path);
                        continue;
                    }
                };

                // Convert Rhai Map to JSON string
                let json_str = rhai::format_map_as_json(&template_obj);

                // Parse into PromptTemplate
                match serde_json::from_str::<PromptTemplate>(&json_str) {
                    Ok(template) => {
                        // Register the template in the registry
                        registry.register(template.clone());
                        tracing::info!("Successfully registered prompt template: {}", template.id);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse prompt template: {:?}, error: {}", path, e);
                        continue;
                    }
                }
            }
        }

        tracing::info!(
            "Successfully refreshed prompt templates from path: {:?}",
            self.script_path
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl mofa_kernel::plugin::AgentPlugin for RhaiScriptPromptPlugin {
    fn metadata(&self) -> &mofa_kernel::plugin::PluginMetadata {
        use mofa_kernel::plugin::{PluginMetadata, PluginType};

        lazy_static::lazy_static! {
            static ref METADATA: PluginMetadata = PluginMetadata::new(
                "rhai-prompt-template-plugin",
                "Rhai Prompt Template Plugin",
                PluginType::Tool
            )
            .with_capability("prompt-template");
        }

        &METADATA
    }

    fn state(&self) -> mofa_kernel::plugin::PluginState {
        mofa_kernel::plugin::PluginState::Loaded
    }

    async fn load(
        &mut self,
        _ctx: &mofa_kernel::plugin::PluginContext,
    ) -> mofa_kernel::plugin::PluginResult<()> {
        // Load templates on plugin load
        self.refresh_templates().await?;
        Ok(())
    }

    async fn init_plugin(&mut self) -> mofa_kernel::plugin::PluginResult<()> {
        Ok(())
    }

    async fn start(&mut self) -> mofa_kernel::plugin::PluginResult<()> {
        Ok(())
    }

    async fn stop(&mut self) -> mofa_kernel::plugin::PluginResult<()> {
        Ok(())
    }

    async fn unload(&mut self) -> mofa_kernel::plugin::PluginResult<()> {
        Ok(())
    }

    async fn execute(&mut self, input: String) -> mofa_kernel::plugin::PluginResult<String> {
        // Parse input to decide what to do
        // This could support commands like "set_scenario:promotion" or "get_template:outage"
        if input.starts_with("set_scenario:") {
            let scenario = input
                .strip_prefix("set_scenario:")
                .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Invalid scenario".into()))?;
            self.set_active_scenario(scenario).await;
            Ok(format!("Successfully switched to scenario: {}", scenario))
        } else if input.starts_with("get_template:") {
            let scenario = input
                .strip_prefix("get_template:")
                .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Invalid scenario".into()))?;
            if let Some(template) = self.get_prompt_template(scenario).await {
                Ok(serde_json::to_string(&template)
                    .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?)
            } else {
                Ok(format!("Template not found: {}", scenario))
            }
        } else if input == "list_scenarios" {
            let scenarios = self.get_available_scenarios().await;
            let scenarios_json = serde_json::to_string(&scenarios)
                .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;
            Ok(scenarios_json)
        } else if input == "refresh_templates" {
            self.refresh_templates().await?;
            Ok("Successfully refreshed templates".to_string())
        } else {
            // Default: return current template
            if let Some(template) = self.get_current_template().await {
                Ok(template.content.clone())
            } else {
                Ok("No active template found".to_string())
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}
