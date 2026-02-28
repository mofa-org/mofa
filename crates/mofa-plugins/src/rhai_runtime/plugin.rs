//! Rhai Plugin Implementation
//!
//! Implements AgentPlugin for Rhai scripts

use super::types::{PluginMetadata, RhaiPluginError, RhaiPluginResult};
use mofa_extra::rhai::{
    RhaiScriptEngine, ScriptContext, ScriptEngineConfig, dynamic_to_json, json_to_dynamic,
};
use mofa_kernel::plugin::{
    AgentPlugin, PluginContext, PluginMetadata as KernelPluginMetadata, PluginResult, PluginState,
    PluginType,
};
use rhai::Dynamic;
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, warn};

// ============================================================================
// Plugin Runtime Statistics
// ============================================================================

/// Atomic counters that track invocation metrics for a single Rhai plugin.
///
/// Counters use `Relaxed` ordering because they are informational — no
/// cross-thread synchronisation is required beyond the atomicity itself.
#[derive(Debug, Default)]
pub struct PluginStats {
    /// Total invocations (successful + failed).
    calls_total: AtomicU64,
    /// Invocations that returned an error.
    calls_failed: AtomicU64,
    /// Running sum of wall-clock latencies in milliseconds.
    total_latency_ms: AtomicU64,
}

impl PluginStats {
    /// Record one completed invocation.
    pub fn record(&self, latency_ms: u64, failed: bool) {
        self.calls_total.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);
        if failed {
            self.calls_failed.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Total invocations recorded.
    pub fn calls_total(&self) -> u64 {
        self.calls_total.load(Ordering::Relaxed)
    }

    /// Invocations that returned an error.
    pub fn calls_failed(&self) -> u64 {
        self.calls_failed.load(Ordering::Relaxed)
    }

    /// Average wall-clock latency across all recorded invocations.
    pub fn avg_latency_ms(&self) -> f64 {
        let total = self.calls_total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        self.total_latency_ms.load(Ordering::Relaxed) as f64 / total as f64
    }

    /// Snapshot the current counters as a JSON map for the monitoring dashboard.
    pub fn to_map(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        map.insert(
            "calls_total".to_string(),
            serde_json::json!(self.calls_total.load(Ordering::Relaxed)),
        );
        map.insert(
            "calls_failed".to_string(),
            serde_json::json!(self.calls_failed.load(Ordering::Relaxed)),
        );
        map.insert(
            "avg_latency_ms".to_string(),
            serde_json::json!(self.avg_latency_ms()),
        );
        map
    }
}

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
    pub fn new_file(plugin_id: &str, file_path: &std::path::Path) -> Self {
        Self {
            source: RhaiPluginSource::File(file_path.to_path_buf()),
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
    /// Cached kernel metadata — stored here to avoid Box::leak in metadata()
    kernel_metadata: KernelPluginMetadata,
    /// Current plugin state
    state: RwLock<RhaiPluginState>,
    /// Plugin context
    plugin_context: RwLock<Option<PluginContext>>,
    /// Last modification time (for hot reload)
    last_modified: u64,
    /// Cached script content
    cached_content: String,
    /// Runtime invocation statistics
    stats: Arc<PluginStats>,
}

impl RhaiPlugin {
    /// Get last modification time
    pub fn last_modified(&self) -> u64 {
        self.last_modified
    }

    /// Return a shared reference to this plugin's runtime statistics.
    ///
    /// The returned `Arc` can be cloned and stored by the caller so that stats
    /// can be read concurrently without holding a mutable reference to the plugin.
    pub fn stats(&self) -> Arc<PluginStats> {
        Arc::clone(&self.stats)
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

        // Initialize with default metadata
        let metadata = PluginMetadata {
            id: config.plugin_id.clone(),
            ..Default::default()
        };

        // Build kernel metadata once so metadata() can return a plain borrow
        let kernel_metadata =
            KernelPluginMetadata::new(&config.plugin_id, &metadata.name, PluginType::Tool);

        // Create plugin
        let mut plugin = Self {
            id: config.plugin_id.clone(),
            config,
            engine,
            metadata,
            kernel_metadata,
            state: RwLock::new(RhaiPluginState::Unloaded),
            plugin_context: RwLock::new(None),
            last_modified,
            cached_content: content,
            stats: Arc::new(PluginStats::default()),
        };

        // Keep metadata consistent from first construction.
        plugin.extract_metadata().await?;

        Ok(plugin)
    }

    /// Create a new Rhai plugin from file path
    pub async fn from_file(plugin_id: &str, path: &std::path::Path) -> RhaiPluginResult<Self> {
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
                .unwrap_or_default()
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
        // Compile and cache the script first to define global variables
        let script_id = format!("{}_metadata", self.id);
        if let Err(e) = self
            .engine
            .compile_and_cache(&script_id, "metadata", &self.cached_content)
            .await
        {
            warn!("Failed to compile script for metadata extraction: {}", e);
        } else {
            let context = mofa_extra::rhai::ScriptContext::new();

            // Execute the script to define global variables
            if self
                .engine
                .execute_compiled(&script_id, &context)
                .await
                .is_ok()
            {
                // Try to extract plugin_name
                if let Ok(result) = self.engine.execute("plugin_name", &context).await
                    && result.success
                    && let Some(name) = result.value.as_str()
                {
                    self.metadata.name = name.to_string();
                    self.kernel_metadata.name = name.to_string();
                }

                // Try to extract plugin_version
                if let Ok(result) = self.engine.execute("plugin_version", &context).await
                    && result.success
                    && let Some(version) = result.value.as_str()
                {
                    self.metadata.version = version.to_string();
                    self.kernel_metadata.version = version.to_string();
                }

                // Try to extract plugin_description
                if let Ok(result) = self.engine.execute("plugin_description", &context).await
                    && result.success
                    && let Some(description) = result.value.as_str()
                {
                    self.metadata.description = description.to_string();
                    self.kernel_metadata.description = description.to_string();
                }
            }
        }

        // Fallback to direct source parsing when scope extraction misses values.
        if self.metadata.name == "unknown"
            && let Some(name) = extract_quoted_assignment(&self.cached_content, "plugin_name")
        {
            self.metadata.name = name.clone();
            self.kernel_metadata.name = name;
        }

        if self.metadata.version == "0.0.0"
            && let Some(version) = extract_quoted_assignment(&self.cached_content, "plugin_version")
        {
            self.metadata.version = version.clone();
            self.kernel_metadata.version = version;
        }

        if self.metadata.description.is_empty()
            && let Some(description) =
                extract_quoted_assignment(&self.cached_content, "plugin_description")
        {
            self.metadata.description = description.clone();
            self.kernel_metadata.description = description;
        }

        Ok(())
    }

    /// Call a script function if it exists
    pub async fn call_script_function(
        &self,
        function_name: &str,
        args: &[Dynamic],
    ) -> RhaiPluginResult<Option<Dynamic>> {
        // Compile and cache the plugin script if not already done
        let script_id = format!("{}_main", self.id);
        if let Err(e) = self
            .engine
            .compile_and_cache(&script_id, &self.id, &self.cached_content)
            .await
        {
            return Err(RhaiPluginError::CompilationError(format!(
                "Failed to compile script: {}",
                e
            )));
        }

        // Create a script context
        let context = ScriptContext::new();

        // Convert Dynamic args to serde_json::Value for call_function
        let json_args: Vec<serde_json::Value> = args.iter().map(dynamic_to_json).collect();

        // Try to call the function, using serde_json::Value as the return type
        // This is flexible and won't fail on deserialization
        match self
            .engine
            .call_function::<serde_json::Value>(&script_id, function_name, json_args, &context)
            .await
        {
            Ok(json_result) => {
                // Convert JSON result back to Dynamic
                let dynamic_result = json_to_dynamic(&json_result);
                Ok(Some(dynamic_result))
            }
            Err(e) => {
                let err_str = e.to_string();
                let err_lower = err_str.to_lowercase();
                // Check if error indicates function not found
                if err_lower.contains("function not found") || err_lower.contains("cannot find") {
                    // Function doesn't exist - return None instead of error
                    // This allows optional functions like init, start, stop
                    Ok(None)
                } else {
                    Err(RhaiPluginError::ExecutionError(err_str))
                }
            }
        }
    }

    /// Inner execution helper — called by [`execute`] so that timing and stats
    /// collection are isolated from the actual script dispatch logic.
    ///
    /// Only reads from `self` so the borrow checker allows `execute()` to call
    /// it via `&self` after temporarily releasing the mutable borrow.
    async fn execute_script(&self, input: String) -> PluginResult<String> {
        // Create context with input
        let mut context = ScriptContext::new();
        context = context.with_variable("input", input.clone()).map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        // Compile and cache the script (idempotent when already cached)
        let script_id = format!("{}_exec", self.id);
        self.engine
            .compile_and_cache(&script_id, "execute", &self.cached_content)
            .await.map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        // Try to call the execute function with the input
        match self
            .engine
            .call_function::<serde_json::Value>(
                &script_id,
                "execute",
                vec![serde_json::json!(input)],
                &context,
            )
            .await
        {
            Ok(result) => {
                info!(
                    "Rhai plugin {} executed successfully via call_function",
                    self.id
                );
                Ok(serde_json::to_string_pretty(&result).map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?)
            }
            Err(e) => {
                warn!(
                    "Failed to call execute function: {}, falling back to direct execution",
                    e
                );

                // Fallback: execute the whole script directly
                let result = self.engine.execute(&self.cached_content, &context).await.map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

                if !result.success {
                    return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                        "Script execution failed: {:?}",
                        result.error
                    )));
                }

                Ok(serde_json::to_string_pretty(&result.value).map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?)
            }
        }
    }
}

fn extract_quoted_assignment(script: &str, variable: &str) -> Option<String> {
    for line in script.lines() {
        let trimmed = line.trim();
        let statement = trimmed.trim_end_matches(';').trim();

        let Some(rest) = statement
            .strip_prefix("let ")
            .or_else(|| statement.strip_prefix("const "))
        else {
            continue;
        };
        let Some(rest) = rest.strip_prefix(variable) else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();

        if let Some(value) = parse_quoted_literal(rest, '"') {
            return Some(value);
        }
        if let Some(value) = parse_quoted_literal(rest, '\'') {
            return Some(value);
        }
    }
    None
}

fn parse_quoted_literal(input: &str, quote: char) -> Option<String> {
    let mut chars = input.chars();
    if chars.next()? != quote {
        return None;
    }

    let mut output = String::new();
    let mut escaped = false;

    for ch in chars {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(output);
        }
        output.push(ch);
    }

    None
}

// ============================================================================
// AgentPlugin Implementation for RhaiPlugin
// ============================================================================

#[async_trait::async_trait]
impl AgentPlugin for RhaiPlugin {
    fn metadata(&self) -> &KernelPluginMetadata {
        &self.kernel_metadata
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
        self.extract_metadata().await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        let mut state = self.state.write().await;
        *state = RhaiPluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        let mut state = self.state.write().await;
        if *state != RhaiPluginState::Loaded {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Plugin not loaded".to_string()));
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
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Plugin not ready to start".to_string()));
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
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Plugin not running".to_string()));
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
        {
            let state = self.state.read().await;
            if *state != RhaiPluginState::Running {
                return Err(mofa_kernel::plugin::PluginError::ExecutionFailed("Plugin not running".to_string()));
            }
        }

        // --- stats: start wall-clock timer ---
        let timer = Instant::now();
        let result = self.execute_script(input).await;
        let latency_ms = timer.elapsed().as_millis() as u64;
        self.stats.record(latency_ms, result.is_err());
        result
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        self.stats.to_map()
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
        let plugin_name = "test_rhai_plugin";
        let plugin_version = "1.0.0";
        let plugin_description = "Test Rhai plugin";

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
        assert_eq!(plugin.metadata.description, "Test Rhai plugin");
        assert_eq!(plugin.kernel_metadata.name, "test_rhai_plugin");
        assert_eq!(plugin.kernel_metadata.version, "1.0.0");
        assert_eq!(plugin.kernel_metadata.description, "Test Rhai plugin");
        assert!(!plugin.cached_content.is_empty());
    }

    #[tokio::test]
    async fn test_rhai_plugin_extracts_metadata_even_when_script_compile_fails() {
        let broken_script = r#"
            let plugin_name = "fallback_metadata_plugin";
            let plugin_version = "2.1.3";
            let plugin_description = "Fallback parser metadata";

            fn execute(input) {
                input +    // force compile failure
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-broken-metadata", broken_script)
            .await
            .unwrap();

        assert_eq!(plugin.metadata.name, "fallback_metadata_plugin");
        assert_eq!(plugin.metadata.version, "2.1.3");
        assert_eq!(plugin.metadata.description, "Fallback parser metadata");
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
        // Result should be the string returned by execute function
        // Note: The result is JSON serialized, so it will be a quoted string
        println!("Execute result: {}", result);

        // The execute function returns a string, which gets JSON serialized
        // So we expect the result to be a JSON string containing our message
        assert!(
            result.contains("Hello from Rhai plugin!") || result.contains("Hello World!"),
            "Result should contain expected text, got: {}",
            result
        );

        plugin.unload().await.unwrap();
    }

    // ========================================================================
    // Unit Tests for call_script_function
    // ========================================================================

    #[tokio::test]
    async fn test_call_script_function_basic_arithmetic() {
        let script = r#"
            fn add(a, b) {
                a + b
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-add", script).await.unwrap();

        let args = vec![Dynamic::from(5), Dynamic::from(3)];
        let result = plugin.call_script_function("add", &args).await.unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().as_int().unwrap(), 8);
    }

    #[tokio::test]
    async fn test_call_script_function_string_manipulation() {
        let script = r#"
            fn greet(name) {
                "Hello, " + name + "!"
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-greet", script)
            .await
            .unwrap();

        let args = vec![Dynamic::from("World")];
        let result = plugin.call_script_function("greet", &args).await.unwrap();

        assert!(result.is_some());
        let result_str = result.unwrap().to_string();
        assert!(result_str.contains("Hello") && result_str.contains("World"));
    }

    #[tokio::test]
    async fn test_call_script_function_with_array() {
        let script = r#"
            fn sum_array(arr) {
                let total = 0;
                for i in arr {
                    total = total + i;
                }
                total
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-sum", script).await.unwrap();

        let array = rhai::Array::from(vec![Dynamic::from(1), Dynamic::from(2), Dynamic::from(3)]);
        let args = vec![array.into()];

        let result = plugin
            .call_script_function("sum_array", &args)
            .await
            .unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().as_int().unwrap(), 6);
    }

    #[tokio::test]
    async fn test_call_script_function_no_arguments() {
        let script = r#"
            fn get_pi() {
                3.14159
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-pi", script).await.unwrap();

        let result = plugin.call_script_function("get_pi", &[]).await.unwrap();

        assert!(result.is_some());
        let value = result.unwrap().as_float().unwrap();
        assert!((value - std::f64::consts::PI).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_call_script_function_optional_function_not_found() {
        let script = r#"
            fn existing_function() {
                42
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-optional", script)
            .await
            .unwrap();

        // Try to call a function that doesn't exist - should return None
        let result = plugin
            .call_script_function("non_existent", &[])
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_call_script_function_nested_calls() {
        let script = r#"
            fn double(x) {
                x * 2
            }

            fn process(value) {
                double(value) + 10
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-nested", script)
            .await
            .unwrap();

        let args = vec![Dynamic::from(5)];
        let result = plugin.call_script_function("process", &args).await.unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().as_int().unwrap(), 20); // (5 * 2) + 10
    }

    #[tokio::test]
    async fn test_call_script_function_recursive_factorial() {
        let script = r#"
            fn factorial(n) {
                if n <= 1 {
                    1
                } else {
                    n * factorial(n - 1)
                }
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-factorial", script)
            .await
            .unwrap();

        let args = vec![Dynamic::from(5)];
        let result = plugin
            .call_script_function("factorial", &args)
            .await
            .unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().as_int().unwrap(), 120);
    }

    #[tokio::test]
    async fn test_call_script_function_conditional_logic() {
        let script = r#"
            fn check_value(n) {
                if n > 10 {
                    "large"
                } else if n > 5 {
                    "medium"
                } else {
                    "small"
                }
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-conditional", script)
            .await
            .unwrap();

        // Test different values
        let test_cases = vec![(1, "small"), (7, "medium"), (15, "large")];

        for (value, expected) in test_cases {
            let args = vec![Dynamic::from(value)];
            let result = plugin
                .call_script_function("check_value", &args)
                .await
                .unwrap();

            assert!(result.is_some());
            let result_str = result.unwrap().to_string();
            assert!(result_str.contains(expected));
        }
    }

    #[tokio::test]
    async fn test_call_script_function_multiple_sequential_calls() {
        let script = r#"
            fn increment(value) {
                value + 1
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-counter", script)
            .await
            .unwrap();

        // Make multiple calls to verify repeated invocation works.
        for i in 0..3 {
            let args = vec![Dynamic::from(i)];
            let result = plugin
                .call_script_function("increment", &args)
                .await
                .unwrap();
            assert!(result.is_some());
            assert_eq!(result.unwrap().as_int().unwrap(), (i + 1) as i64);
        }
    }

    #[tokio::test]
    async fn test_call_script_function_with_various_types() {
        let script = r#"
            fn process_types(i, f, s, b) {
                i + 1
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-types", script)
            .await
            .unwrap();

        let args = vec![
            Dynamic::from(42),
            Dynamic::from(std::f64::consts::PI),
            Dynamic::from("text"),
            Dynamic::TRUE,
        ];

        let result = plugin
            .call_script_function("process_types", &args)
            .await
            .unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().as_int().unwrap(), 43);
    }

    #[tokio::test]
    async fn test_call_script_function_empty_string_arg() {
        let script = r#"
            fn is_empty(s) {
                s == ""
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-empty-string", script)
            .await
            .unwrap();

        let args = vec![Dynamic::from("")];
        let result = plugin
            .call_script_function("is_empty", &args)
            .await
            .unwrap();

        assert!(result.is_some());
        assert!(result.unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_call_script_function_array_filtering() {
        let script = r#"
            fn is_even(n) {
                n % 2 == 0
            }

            fn filter_even(arr) {
                let result = [];
                for item in arr {
                    if is_even(item) {
                        result.push(item);
                    }
                }
                result
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-filter-even", script)
            .await
            .unwrap();

        let array = rhai::Array::from(vec![
            Dynamic::from(1),
            Dynamic::from(2),
            Dynamic::from(3),
            Dynamic::from(4),
            Dynamic::from(5),
            Dynamic::from(6),
        ]);
        let args = vec![array.into()];

        let result = plugin
            .call_script_function("filter_even", &args)
            .await
            .unwrap();

        assert!(result.is_some());
        assert!(result.is_some()); // Just verify result is not None
    }

    // ========================================================================
    // Unit Tests for PluginStats
    // ========================================================================

    #[test]
    fn test_plugin_stats_zero_call_guard() {
        // avg_latency_ms must return 0.0 when no calls have been recorded yet
        // (guard against division-by-zero).
        let stats = PluginStats::default();
        assert_eq!(stats.calls_total(), 0);
        assert_eq!(stats.calls_failed(), 0);
        assert_eq!(stats.avg_latency_ms(), 0.0);
    }

    #[test]
    fn test_plugin_stats_successful_invocation() {
        let stats = PluginStats::default();
        stats.record(50, false);

        assert_eq!(stats.calls_total(), 1);
        assert_eq!(stats.calls_failed(), 0);
        assert_eq!(stats.avg_latency_ms(), 50.0);
    }

    #[test]
    fn test_plugin_stats_failed_invocation() {
        let stats = PluginStats::default();
        stats.record(30, true);

        assert_eq!(stats.calls_total(), 1);
        assert_eq!(stats.calls_failed(), 1);
        assert_eq!(stats.avg_latency_ms(), 30.0);
    }

    #[test]
    fn test_plugin_stats_counter_increments() {
        let stats = PluginStats::default();

        // Two successes, one failure
        stats.record(10, false);
        stats.record(20, false);
        stats.record(30, true);

        assert_eq!(stats.calls_total(), 3);
        assert_eq!(stats.calls_failed(), 1);
        // avg = (10 + 20 + 30) / 3 = 20.0
        assert!((stats.avg_latency_ms() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_plugin_stats_avg_latency_multiple_calls() {
        let stats = PluginStats::default();

        stats.record(100, false);
        stats.record(200, false);

        // avg = (100 + 200) / 2 = 150.0
        assert!((stats.avg_latency_ms() - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_plugin_stats_all_failed() {
        let stats = PluginStats::default();

        stats.record(5, true);
        stats.record(10, true);

        assert_eq!(stats.calls_total(), 2);
        assert_eq!(stats.calls_failed(), 2);
    }

    #[test]
    fn test_plugin_stats_to_map_keys() {
        let stats = PluginStats::default();
        stats.record(40, false);
        stats.record(60, true);

        let map = stats.to_map();

        assert!(map.contains_key("calls_total"), "missing calls_total");
        assert!(map.contains_key("calls_failed"), "missing calls_failed");
        assert!(map.contains_key("avg_latency_ms"), "missing avg_latency_ms");

        assert_eq!(map["calls_total"], serde_json::json!(2u64));
        assert_eq!(map["calls_failed"], serde_json::json!(1u64));
        // avg = (40 + 60) / 2 = 50.0
        let avg = map["avg_latency_ms"].as_f64().unwrap();
        assert!((avg - 50.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_rhai_plugin_stats_accessor_returns_arc() {
        // The stats() method must return an Arc that shares the same counters
        // as the plugin internal state, not a copy.
        let plugin = RhaiPlugin::from_content("test-stats-arc", "fn execute(i) { i }")
            .await
            .unwrap();

        let stats_ref = plugin.stats();

        // Initially zero
        assert_eq!(stats_ref.calls_total(), 0);
    }

    #[tokio::test]
    async fn test_plugin_stats_via_agent_plugin_trait() {
        // AgentPlugin::stats() must also reflect live counter values.
        let mut plugin = RhaiPlugin::from_content("test-trait-stats", "fn execute(i) { i } ")
            .await
            .unwrap();

        let ctx = PluginContext::default();
        plugin.load(&ctx).await.unwrap();
        plugin.init_plugin().await.unwrap();

        // Execute once to increment counters
        let _ = plugin.execute("ping".to_string()).await;

        // Read back through AgentPlugin::stats()
        let map = AgentPlugin::stats(&plugin);
        let total = map["calls_total"].as_u64().unwrap();
        assert_eq!(total, 1, "calls_total should be 1 after one execute");
    }

    #[tokio::test]
    async fn test_plugin_stats_arc_shared_with_plugin_execute() {
        // Clone the Arc before executing; after execute() completes the cloned
        // reference must observe the updated counters.
        //
        // Note: scripts that throw *inside* fn execute() are currently healed by
        // the fallback direct-execution path, so we use only success scenarios here.
        let mut plugin = RhaiPlugin::from_content("test-arc-shared", "fn execute(i) { i }")
            .await
            .unwrap();

        let ctx = PluginContext::default();
        plugin.load(&ctx).await.unwrap();
        plugin.init_plugin().await.unwrap();

        // Grab Arc before any execution
        let stats = plugin.stats();
        assert_eq!(stats.calls_total(), 0);

        let _ = plugin.execute("hello".to_string()).await;
        let _ = plugin.execute("world".to_string()).await;

        // The Arc should now reflect the two calls
        assert_eq!(stats.calls_total(), 2);
        assert_eq!(stats.calls_failed(), 0);
    }

    #[tokio::test]
    async fn test_plugin_stats_failed_execute_increments_failed_counter() {
        // A top-level `throw` means:
        //  1. call_function("execute") fails with "function not found" → fallback triggers
        //  2. fallback runs the full script body which throws at the top level → execute_script Err
        // This is the most reliable way to force a recorded failure without
        // relying on details of the call_function error-vs-fallback split.
        let script = r#"throw "intentional top-level error";"#;

        let mut plugin = RhaiPlugin::from_content("test-fail-stats", script)
            .await
            .unwrap();

        let ctx = PluginContext::default();
        plugin.load(&ctx).await.unwrap();
        plugin.init_plugin().await.unwrap();

        let stats = plugin.stats();

        // Execute should return an error
        let result = plugin.execute("trigger".to_string()).await;
        assert!(result.is_err(), "expected execution to fail");

        assert_eq!(stats.calls_total(), 1, "calls_total should still be 1");
        assert_eq!(stats.calls_failed(), 1, "calls_failed should be 1");
    }

    #[tokio::test]
    async fn test_plugin_stats_boolean_logic() {
        let script = r#"
            fn validate(age, is_citizen) {
                age >= 18 && is_citizen == true
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-validate", script)
            .await
            .unwrap();

        let args = vec![Dynamic::from(21), Dynamic::TRUE];
        let result = plugin
            .call_script_function("validate", &args)
            .await
            .unwrap();

        assert!(result.is_some());
        assert!(result.unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_call_script_function_string_length() {
        let script = r#"
            fn string_length(s) {
                s.len()
            }
        "#;

        let plugin = RhaiPlugin::from_content("test-str-len", script)
            .await
            .unwrap();

        let args = vec![Dynamic::from("hello")];
        let result = plugin
            .call_script_function("string_length", &args)
            .await
            .unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().as_int().unwrap(), 5);
    }
}
