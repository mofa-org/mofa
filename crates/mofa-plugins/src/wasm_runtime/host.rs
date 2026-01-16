//! Host Functions for WASM Plugins
//!
//! Defines the host functions that WASM plugins can call

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use super::types::{PluginCapability, WasmError, WasmResult, WasmValue};

/// Log level for plugin logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl From<u32> for LogLevel {
    fn from(v: u32) -> Self {
        match v {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            _ => LogLevel::Error,
        }
    }
}

/// Message direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageDirection {
    Incoming,
    Outgoing,
}

/// Host callback for custom functions
pub type HostCallback = Arc<dyn Fn(&str, Vec<WasmValue>) -> WasmResult<WasmValue> + Send + Sync>;

/// Host context provided to plugins
pub struct HostContext {
    /// Plugin ID
    pub plugin_id: String,
    /// Plugin capabilities
    pub capabilities: Vec<PluginCapability>,
    /// Configuration values
    config: Arc<RwLock<HashMap<String, WasmValue>>>,
    /// Storage backend
    storage: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// Message queue (outgoing)
    message_queue: Arc<RwLock<Vec<HostMessage>>>,
    /// Custom host functions
    custom_functions: HashMap<String, HostCallback>,
    /// Execution metrics
    metrics: Arc<RwLock<HostMetrics>>,
}

/// Host message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMessage {
    pub target: String,
    pub payload: Vec<u8>,
    pub timestamp: u64,
}

/// Host execution metrics
#[derive(Debug, Clone, Default)]
pub struct HostMetrics {
    pub log_calls: u64,
    pub config_reads: u64,
    pub config_writes: u64,
    pub messages_sent: u64,
    pub tool_calls: u64,
    pub storage_reads: u64,
    pub storage_writes: u64,
    pub total_execution_time_ns: u64,
}

impl HostContext {
    pub fn new(plugin_id: &str, capabilities: Vec<PluginCapability>) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            capabilities,
            config: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(RwLock::new(HashMap::new())),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            custom_functions: HashMap::new(),
            metrics: Arc::new(RwLock::new(HostMetrics::default())),
        }
    }

    /// Check if plugin has a capability
    pub fn has_capability(&self, cap: &PluginCapability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Require a capability, returning error if not present
    pub fn require_capability(&self, cap: &PluginCapability) -> WasmResult<()> {
        if self.has_capability(cap) {
            Ok(())
        } else {
            Err(WasmError::HostFunctionError(format!(
                "Plugin {} lacks required capability: {}",
                self.plugin_id, cap
            )))
        }
    }

    /// Register a custom host function
    pub fn register_function(&mut self, name: &str, callback: HostCallback) {
        self.custom_functions.insert(name.to_string(), callback);
    }

    /// Set configuration value
    pub async fn set_config(&self, key: &str, value: WasmValue) {
        self.config.write().await.insert(key.to_string(), value);
    }

    /// Get configuration value
    pub async fn get_config(&self, key: &str) -> Option<WasmValue> {
        self.config.read().await.get(key).cloned()
    }

    /// Get all pending messages
    pub async fn drain_messages(&self) -> Vec<HostMessage> {
        let mut queue = self.message_queue.write().await;
        std::mem::take(&mut *queue)
    }

    /// Get metrics
    pub async fn metrics(&self) -> HostMetrics {
        self.metrics.read().await.clone()
    }
}

/// Host functions interface that plugins can call
#[async_trait]
pub trait HostFunctions: Send + Sync {
    // === Logging ===

    /// Log a message
    async fn log(&self, level: LogLevel, message: &str) -> WasmResult<()>;

    // === Configuration ===

    /// Get configuration value
    async fn get_config(&self, key: &str) -> WasmResult<Option<WasmValue>>;

    /// Set configuration value (requires WriteConfig capability)
    async fn set_config(&self, key: &str, value: WasmValue) -> WasmResult<()>;

    // === Messaging ===

    /// Send a message to another agent/plugin
    async fn send_message(&self, target: &str, payload: &[u8]) -> WasmResult<()>;

    // === Tools ===

    /// Call a tool
    async fn call_tool(&self, tool_name: &str, args: WasmValue) -> WasmResult<WasmValue>;

    // === Storage ===

    /// Get from storage
    async fn storage_get(&self, key: &str) -> WasmResult<Option<Vec<u8>>>;

    /// Set in storage
    async fn storage_set(&self, key: &str, value: &[u8]) -> WasmResult<()>;

    /// Delete from storage
    async fn storage_delete(&self, key: &str) -> WasmResult<()>;

    // === Utilities ===

    /// Get current timestamp (milliseconds)
    async fn now_ms(&self) -> WasmResult<u64>;

    /// Generate random bytes
    async fn random_bytes(&self, len: u32) -> WasmResult<Vec<u8>>;

    /// Sleep for specified milliseconds
    async fn sleep_ms(&self, ms: u64) -> WasmResult<()>;

    // === Custom ===

    /// Call a custom host function
    async fn call_custom(&self, name: &str, args: Vec<WasmValue>) -> WasmResult<WasmValue>;
}

/// Default implementation of host functions
pub struct DefaultHostFunctions {
    context: Arc<HostContext>,
}

impl DefaultHostFunctions {
    pub fn new(context: Arc<HostContext>) -> Self {
        Self { context }
    }

    async fn inc_metric(&self, f: impl FnOnce(&mut HostMetrics)) {
        let mut metrics = self.context.metrics.write().await;
        f(&mut metrics);
    }
}

#[async_trait]
impl HostFunctions for DefaultHostFunctions {
    async fn log(&self, level: LogLevel, message: &str) -> WasmResult<()> {
        self.inc_metric(|m| m.log_calls += 1).await;

        let plugin_id = &self.context.plugin_id;
        match level {
            LogLevel::Trace => tracing::trace!(plugin_id, "{}", message),
            LogLevel::Debug => tracing::debug!(plugin_id, "{}", message),
            LogLevel::Info => tracing::info!(plugin_id, "{}", message),
            LogLevel::Warn => tracing::warn!(plugin_id, "{}", message),
            LogLevel::Error => tracing::error!(plugin_id, "{}", message),
        }
        Ok(())
    }

    async fn get_config(&self, key: &str) -> WasmResult<Option<WasmValue>> {
        self.context
            .require_capability(&PluginCapability::ReadConfig)?;
        self.inc_metric(|m| m.config_reads += 1).await;

        Ok(self.context.get_config(key).await)
    }

    async fn set_config(&self, key: &str, value: WasmValue) -> WasmResult<()> {
        self.context
            .require_capability(&PluginCapability::WriteConfig)?;
        self.inc_metric(|m| m.config_writes += 1).await;

        self.context.set_config(key, value).await;
        Ok(())
    }

    async fn send_message(&self, target: &str, payload: &[u8]) -> WasmResult<()> {
        self.context
            .require_capability(&PluginCapability::SendMessage)?;
        self.inc_metric(|m| m.messages_sent += 1).await;

        let msg = HostMessage {
            target: target.to_string(),
            payload: payload.to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        self.context.message_queue.write().await.push(msg);
        debug!(
            "Plugin {} sent message to {}",
            self.context.plugin_id, target
        );
        Ok(())
    }

    async fn call_tool(&self, tool_name: &str, _args: WasmValue) -> WasmResult<WasmValue> {
        self.context
            .require_capability(&PluginCapability::CallTool)?;
        self.inc_metric(|m| m.tool_calls += 1).await;

        // For now, return a mock response
        // In real implementation, this would call the actual tool executor
        debug!(
            "Plugin {} calling tool: {}",
            self.context.plugin_id, tool_name
        );
        Ok(WasmValue::Map(HashMap::from([
            ("tool".to_string(), WasmValue::String(tool_name.to_string())),
            (
                "status".to_string(),
                WasmValue::String("success".to_string()),
            ),
        ])))
    }

    async fn storage_get(&self, key: &str) -> WasmResult<Option<Vec<u8>>> {
        self.context
            .require_capability(&PluginCapability::Storage)?;
        self.inc_metric(|m| m.storage_reads += 1).await;

        Ok(self.context.storage.read().await.get(key).cloned())
    }

    async fn storage_set(&self, key: &str, value: &[u8]) -> WasmResult<()> {
        self.context
            .require_capability(&PluginCapability::Storage)?;
        self.inc_metric(|m| m.storage_writes += 1).await;

        self.context
            .storage
            .write()
            .await
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn storage_delete(&self, key: &str) -> WasmResult<()> {
        self.context
            .require_capability(&PluginCapability::Storage)?;

        self.context.storage.write().await.remove(key);
        Ok(())
    }

    async fn now_ms(&self) -> WasmResult<u64> {
        Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64)
    }

    async fn random_bytes(&self, len: u32) -> WasmResult<Vec<u8>> {
        self.context.require_capability(&PluginCapability::Random)?;

        use rand::RngCore;
        let mut bytes = vec![0u8; len as usize];
        rand::thread_rng().fill_bytes(&mut bytes);
        Ok(bytes)
    }

    async fn sleep_ms(&self, ms: u64) -> WasmResult<()> {
        self.context.require_capability(&PluginCapability::Timer)?;

        tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
        Ok(())
    }

    async fn call_custom(&self, name: &str, args: Vec<WasmValue>) -> WasmResult<WasmValue> {
        if let Some(callback) = self.context.custom_functions.get(name) {
            callback(name, args)
        } else {
            Err(WasmError::HostFunctionError(format!(
                "Custom function not found: {}",
                name
            )))
        }
    }
}

/// Host function registry for wasmtime integration
pub struct HostFunctionRegistry {
    /// Function name -> (module, function signature)
    functions: HashMap<String, HostFunctionInfo>,
}

/// Host function info
#[derive(Debug, Clone)]
pub struct HostFunctionInfo {
    pub name: String,
    pub module: String,
    pub params: Vec<String>,
    pub returns: Vec<String>,
    pub required_capability: Option<PluginCapability>,
}

impl HostFunctionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };
        registry.register_builtin_functions();
        registry
    }

    fn register_builtin_functions(&mut self) {
        // Logging
        self.register("host_log", "env", vec!["i32", "i32", "i32"], vec![], None);

        // Configuration
        self.register(
            "host_get_config",
            "env",
            vec!["i32", "i32", "i32"],
            vec!["i32"],
            Some(PluginCapability::ReadConfig),
        );
        self.register(
            "host_set_config",
            "env",
            vec!["i32", "i32", "i32", "i32"],
            vec!["i32"],
            Some(PluginCapability::WriteConfig),
        );

        // Messaging
        self.register(
            "host_send_message",
            "env",
            vec!["i32", "i32", "i32", "i32"],
            vec!["i32"],
            Some(PluginCapability::SendMessage),
        );

        // Tools
        self.register(
            "host_call_tool",
            "env",
            vec!["i32", "i32", "i32", "i32", "i32"],
            vec!["i32"],
            Some(PluginCapability::CallTool),
        );

        // Storage
        self.register(
            "host_storage_get",
            "env",
            vec!["i32", "i32", "i32", "i32"],
            vec!["i32"],
            Some(PluginCapability::Storage),
        );
        self.register(
            "host_storage_set",
            "env",
            vec!["i32", "i32", "i32", "i32"],
            vec!["i32"],
            Some(PluginCapability::Storage),
        );

        // Utilities
        self.register("host_now_ms", "env", vec![], vec!["i64"], None);
        self.register(
            "host_random_bytes",
            "env",
            vec!["i32", "i32"],
            vec!["i32"],
            Some(PluginCapability::Random),
        );
        self.register(
            "host_sleep_ms",
            "env",
            vec!["i64"],
            vec![],
            Some(PluginCapability::Timer),
        );

        // Memory management
        self.register("host_alloc", "env", vec!["i32"], vec!["i32"], None);
        self.register("host_free", "env", vec!["i32"], vec![], None);
    }

    fn register(
        &mut self,
        name: &str,
        module: &str,
        params: Vec<&str>,
        returns: Vec<&str>,
        required_capability: Option<PluginCapability>,
    ) {
        self.functions.insert(
            name.to_string(),
            HostFunctionInfo {
                name: name.to_string(),
                module: module.to_string(),
                params: params.into_iter().map(String::from).collect(),
                returns: returns.into_iter().map(String::from).collect(),
                required_capability,
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&HostFunctionInfo> {
        self.functions.get(name)
    }

    pub fn list(&self) -> Vec<&HostFunctionInfo> {
        self.functions.values().collect()
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }
}

impl Default for HostFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(LogLevel::from(0), LogLevel::Trace);
        assert_eq!(LogLevel::from(2), LogLevel::Info);
        assert_eq!(LogLevel::from(99), LogLevel::Error);
    }

    #[test]
    fn test_host_context() {
        let ctx = HostContext::new(
            "test-plugin",
            vec![PluginCapability::ReadConfig, PluginCapability::SendMessage],
        );

        assert!(ctx.has_capability(&PluginCapability::ReadConfig));
        assert!(!ctx.has_capability(&PluginCapability::Storage));
    }

    #[tokio::test]
    async fn test_host_context_config() {
        let ctx = HostContext::new("test", vec![PluginCapability::ReadConfig]);

        ctx.set_config("key1", WasmValue::String("value1".into()))
            .await;

        let val = ctx.get_config("key1").await;
        assert_eq!(val, Some(WasmValue::String("value1".into())));
    }

    #[tokio::test]
    async fn test_default_host_functions() {
        let ctx = Arc::new(HostContext::new(
            "test",
            vec![PluginCapability::ReadConfig, PluginCapability::Timer],
        ));
        let host = DefaultHostFunctions::new(ctx.clone());

        // Test logging (always allowed)
        host.log(LogLevel::Info, "Test message").await.unwrap();

        // Test now_ms (always allowed)
        let ts = host.now_ms().await.unwrap();
        assert!(ts > 0);

        // Test sleep (requires Timer)
        host.sleep_ms(1).await.unwrap();

        // Test storage should fail (no capability)
        let result = host.storage_get("key").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_host_function_registry() {
        let registry = HostFunctionRegistry::new();

        assert!(registry.has_function("host_log"));
        assert!(registry.has_function("host_get_config"));
        assert!(!registry.has_function("nonexistent"));

        let info = registry.get("host_log").unwrap();
        assert_eq!(info.module, "env");
    }
}
