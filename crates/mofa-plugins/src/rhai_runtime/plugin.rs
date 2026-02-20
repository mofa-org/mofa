//! Rhai Plugin Implementation
//!
//! Implements AgentPlugin for Rhai scripts

use super::types::{PluginMetadata, RhaiPluginResult};
use mofa_extra::rhai::{RhaiScriptEngine, ScriptContext, ScriptEngineConfig};
use mofa_kernel::plugin::{
    AgentPlugin, PluginContext, PluginMetadata as KernelPluginMetadata, PluginResult, PluginState,
    PluginType,
};
use rhai::Dynamic;
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

// ============================================================================
// Rhai Plugin Configuration
// ============================================================================

/// Rhai plugin configuration
#[derive(Debug, Clone)]
pub struct RhaiPluginConfig {
    /// Plugin script content or path
    pub source: RhaiPluginSource,
    /// Engine configuration
    pub engine_config: ScriptEngineConfig,
    /// Initial plugin context
    pub initial_context: HashMap<String, Dynamic>,
    /// Plugin dependencies
    pub dependencies: Vec<String>,
    /// Plugin ID
    pub plugin_id: String,
}

impl Default for RhaiPluginConfig {
    fn default() -> Self {
        Self {
            source: RhaiPluginSource::Inline("".to_string()),
            engine_config: ScriptEngineConfig::default(),
            initial_context: HashMap::new(),
            dependencies: Vec::new(),
            plugin_id: uuid::Uuid::now_v7().to_string(),
        }
    }
}

impl RhaiPluginConfig {
    /// Create a new plugin config from inline script
    pub fn new_inline(plugin_id: &str, script_content: &str) -> Self {
        Self {
            source: RhaiPluginSource::Inline(script_content.to_string()),
            plugin_id: plugin_id.to_string(),
            ..Default::default()
        }
    }

    /// Create a new plugin config from file path
    pub fn new_file(plugin_id: &str, file_path: &PathBuf) -> Self {
        Self {
            source: RhaiPluginSource::File(file_path.clone()),
            plugin_id: plugin_id.to_string(),
            ..Default::default()
        }
    }

    /// With engine configuration
    pub fn with_engine_config(mut self, config: ScriptEngineConfig) -> Self {
        self.engine_config = config;
        self
    }

    /// With initial context variable
    pub fn with_context_var(mut self, key: &str, value: Dynamic) -> Self {
        self.initial_context.insert(key.to_string(), value);
        self
    }
}

/// Rhai plugin source type
#[derive(Debug, Clone)]
pub enum RhaiPluginSource {
    /// Inline script content
    Inline(String),
    /// File path to script
    File(PathBuf),
}

impl RhaiPluginSource {
    /// Get script content from source
    pub async fn get_content(&self) -> RhaiPluginResult<String> {
        match self {
            RhaiPluginSource::Inline(content) => Ok(content.clone()),
            RhaiPluginSource::File(path) => Ok(std::fs::read_to_string(path)?),
        }
    }
}

// ============================================================================
// Rhai Plugin State
// ============================================================================

/// Rhai plugin state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RhaiPluginState {
    /// Plugin is unloaded
    Unloaded,
    /// Plugin is loading
    Loading,
    /// Plugin is loaded but not initialized
    Loaded,
    /// Plugin is initializing
    Initializing,
    /// Plugin is running
    Running,
    /// Plugin is paused
    Paused,
    /// Plugin has encountered an error
    Error(String),
}

impl From<RhaiPluginState> for PluginState {
    fn from(state: RhaiPluginState) -> Self {
        match state {
            RhaiPluginState::Unloaded => PluginState::Unloaded,
            RhaiPluginState::Loading => PluginState::Loading,
            RhaiPluginState::Loaded => PluginState::Loaded,
            RhaiPluginState::Initializing => PluginState::Loading,
            RhaiPluginState::Running => PluginState::Running,
            RhaiPluginState::Paused => PluginState::Paused,
            RhaiPluginState::Error(err) => PluginState::Error(err),
        }
    }
}

// ============================================================================
// Rhai Plugin
// ============================================================================

/// Rhai plugin wrapper
pub struct RhaiPlugin {
    /// Plugin ID
    id: String,
    /// Plugin configuration
    config: RhaiPluginConfig,
    /// Rhai script engine instance
    engine: Arc<RhaiScriptEngine>,
    /// Plugin metadata
    metadata: PluginMetadata,
    /// Current plugin state
    state: RwLock<RhaiPluginState>,
    /// Plugin context
    plugin_context: RwLock<Option<PluginContext>>,
    /// Last modification time (for hot reload)
    last_modified: u64,
    /// Cached script content
    cached_content: String,
}

impl RhaiPlugin {
    /// Get last modification time
    pub fn last_modified(&self) -> u64 {
        self.last_modified
    }

    /// Create a new Rhai plugin from config
    pub async fn new(config: RhaiPluginConfig) -> RhaiPluginResult<Self> {
        let content = config.source.get_content().await?;
        let last_modified = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create engine instance
        let engine = Arc::new(RhaiScriptEngine::new(config.engine_config.clone())?);

        // Parse metadata from script - TODO
        let _script_metadata: HashMap<String, String> = HashMap::new();

        // Initialize with default metadata
        let mut metadata = PluginMetadata::default();
        metadata.id = config.plugin_id.clone();

        // Create plugin
        Ok(Self {
            id: config.plugin_id.clone(),
            config,
            engine,
            metadata,
            state: RwLock::new(RhaiPluginState::Unloaded),
            plugin_context: RwLock::new(None),
            last_modified,
            cached_content: content,
        })
    }

    /// Create a new Rhai plugin from file path
    pub async fn from_file(plugin_id: &str, path: &PathBuf) -> RhaiPluginResult<Self> {
        let config = RhaiPluginConfig::new_file(plugin_id, path);
        Self::new(config).await
    }

    /// Create a new Rhai plugin from inline script content
    pub async fn from_content(plugin_id: &str, content: &str) -> RhaiPluginResult<Self> {
        let config = RhaiPluginConfig::new_inline(plugin_id, content);
        Self::new(config).await
    }

    /// Reload plugin content
    pub async fn reload(&mut self) -> RhaiPluginResult<()> {
        let new_content = self.config.source.get_content().await?;
        self.cached_content = new_content;

        // Update last modified time from file metadata if available
        self.last_modified = match &self.config.source {
            RhaiPluginSource::File(path) => std::fs::metadata(path)?
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .expect("时间转换失败")
                .as_secs(),
            _ => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        // Re-extract metadata
        self.extract_metadata().await?;

        Ok(())
    }

    /// Extract metadata from Rhai script
    async fn extract_metadata(&mut self) -> RhaiPluginResult<()> {
        // Simplified metadata extraction for now - just keep the default
        Ok(())
    }

    /// Call a script function if it exists
    async fn call_script_function(
        &self,
        _function_name: &str,
        _args: &[Dynamic],
    ) -> RhaiPluginResult<Option<Dynamic>> {
        // TODO: Implement proper function calling
        // Current RhaiScriptEngine doesn't support calling specific functions,
        // only executing entire scripts

        Ok(None)
    }
}

// ============================================================================
// AgentPlugin Implementation for RhaiPlugin
// ============================================================================

#[async_trait::async_trait]
impl AgentPlugin for RhaiPlugin {
    fn metadata(&self) -> &KernelPluginMetadata {
        // Return a static reference - this is a temporary fix
        // In production, we should store KernelPluginMetadata in the struct
        Box::leak(Box::new(KernelPluginMetadata::new(
            &self.id,
            &self.metadata.name,
            PluginType::Tool,
        )))
    }

    fn state(&self) -> PluginState {
        // 在 Tokio 运行时内部使用 blocking 操作必须通过 block_in_place 或 spawn_blocking
        tokio::task::block_in_place(|| {
            let state = self.state.blocking_read();
            state.clone().into()
        })
    }

    async fn load(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        let mut state = self.state.write().await;
        *state = RhaiPluginState::Loading;
        drop(state);

        // Save plugin context
        *self.plugin_context.write().await = Some(ctx.clone());

        // Extract metadata from script
        self.extract_metadata().await?;

        let mut state = self.state.write().await;
        *state = RhaiPluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        let mut state = self.state.write().await;
        if *state != RhaiPluginState::Loaded {
            return Err(anyhow::anyhow!("Plugin not loaded"));
        }

        *state = RhaiPluginState::Initializing;
        drop(state);

        // Call init function if exists
        match self.call_script_function("init", &[]).await {
            Ok(_) => {
                info!("Rhai plugin {}: init function called", self.id);
            }
            Err(e) => {
                warn!("Rhai plugin {}: init function failed: {}", self.id, e);
            }
        }

        let mut state = self.state.write().await;
        *state = RhaiPluginState::Running;
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        let mut state = self.state.write().await;
        if *state != RhaiPluginState::Running && *state != RhaiPluginState::Paused {
            return Err(anyhow::anyhow!("Plugin not ready to start"));
        }

        // Call start function if exists
        match self.call_script_function("start", &[]).await {
            Ok(_) => {
                info!("Rhai plugin {}: start function called", self.id);
            }
            Err(e) => {
                warn!("Rhai plugin {}: start function failed: {}", self.id, e);
            }
        }

        *state = RhaiPluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        let mut state = self.state.write().await;
        if *state != RhaiPluginState::Running {
            return Err(anyhow::anyhow!("Plugin not running"));
        }

        // Call stop function if exists
        match self.call_script_function("stop", &[]).await {
            Ok(_) => {
                info!("Rhai plugin {}: stop function called", self.id);
            }
            Err(e) => {
                warn!("Rhai plugin {}: stop function failed: {}", self.id, e);
            }
        }

        *state = RhaiPluginState::Paused;
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        let mut state = self.state.write().await;
        *state = RhaiPluginState::Unloaded;

        // Call unload function if exists
        match self.call_script_function("unload", &[]).await {
            Ok(_) => {
                info!("Rhai plugin {}: unload function called", self.id);
            }
            Err(e) => {
                warn!("Rhai plugin {}: unload function failed: {}", self.id, e);
            }
        }

        Ok(())
    }

    async fn execute(&mut self, input: String) -> PluginResult<String> {
        let state = self.state.read().await;
        if *state != RhaiPluginState::Running {
            return Err(anyhow::anyhow!("Plugin not running"));
        }
        drop(state);

        // Create context with input
        let mut context = ScriptContext::new();
        context = context.with_variable("input", input.clone())?;

        // Add input to the script and call execute function
        // Properly escape the JSON string as a Rhai string literal
        let full_script = format!(
            "{}\n\n// Call the execute function with the input\nreturn execute({:?});",
            self.cached_content, input
        );

        // Execute script
        let result = self.engine.execute(&full_script, &context).await;

        match result {
            Ok(script_result) => {
                info!("Rhai plugin {} executed successfully", self.id);
                Ok(serde_json::to_string_pretty(&script_result.value)?)
            }
            Err(e) => {
                // Log the exact error and full script to debug
                error!("Failed to execute script with execute function: {}", e);
                error!("Full script content: {}", full_script);

                // If calling named function fails, try just executing the script directly
                warn!(
                    "Failed to call execute function: {}, trying direct execution",
                    e
                );

                let result = self.engine.execute(&self.cached_content, &context).await;

                match result {
                    Ok(script_result) => Ok(serde_json::to_string_pretty(&script_result.value)?),
                    Err(e) => Err(anyhow::anyhow!("Execution error: {}", e)),
                }
            }
        }
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new() // TODO: Implement stats
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_PLUGIN_SCRIPT: &str = r#"
        plugin_name = "test_rhai_plugin";
        plugin_version = "1.0.0";
        plugin_description = "Test Rhai plugin";

        fn init() {
            print("Test plugin initialized");
        }

        fn execute(input) {
            "Hello from Rhai plugin! You said: " + input
        }
    "#;

    #[tokio::test]
    async fn test_rhai_plugin_from_content() {
        let plugin = RhaiPlugin::from_content("test-plugin", TEST_PLUGIN_SCRIPT)
            .await
            .unwrap();

        assert_eq!(plugin.id, "test-plugin");
        assert_eq!(plugin.metadata.name, "test_rhai_plugin");
        assert_eq!(plugin.metadata.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_rhai_plugin_lifecycle() {
        let mut plugin = RhaiPlugin::from_content("test-plugin", TEST_PLUGIN_SCRIPT)
            .await
            .unwrap();

        let ctx = PluginContext::default();
        plugin.load(&ctx).await.unwrap();
        assert!(matches!(
            *plugin.state.read().await,
            RhaiPluginState::Loaded
        ));

        plugin.init_plugin().await.unwrap();
        assert!(matches!(
            *plugin.state.read().await,
            RhaiPluginState::Running
        ));

        plugin.stop().await.unwrap();
        assert!(matches!(
            *plugin.state.read().await,
            RhaiPluginState::Paused
        ));

        plugin.start().await.unwrap();
        assert!(matches!(
            *plugin.state.read().await,
            RhaiPluginState::Running
        ));

        plugin.unload().await.unwrap();
        assert!(matches!(
            *plugin.state.read().await,
            RhaiPluginState::Unloaded
        ));
    }

    #[tokio::test]
    async fn test_rhai_plugin_execute() {
        let mut plugin = RhaiPlugin::from_content("test-plugin", TEST_PLUGIN_SCRIPT)
            .await
            .unwrap();

        let ctx = PluginContext::default();
        plugin.load(&ctx).await.unwrap();
        plugin.init_plugin().await.unwrap();

        let result = plugin.execute("Hello World!".to_string()).await.unwrap();
        // Result should contain the JSON response with our message
        assert!(result.contains("Hello from Rhai plugin!"));
        assert!(result.contains("Hello World!"));

        plugin.unload().await.unwrap();
    }
}
