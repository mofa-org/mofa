use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod error;
pub use error::PluginError;

/// Plugin execution result type using the typed [`PluginError`].
pub type PluginResult<T> = Result<T, PluginError>;

// ============================================================================
// 热加载相关定义 (Hot-reload related definitions)
// ============================================================================

/// 热加载策略
/// Hot-reload strategy
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReloadStrategy {
    /// 立即热加载
    /// Immediate hot-reload
    Immediate,
    /// 防抖热加载
    /// Debounced hot-reload
    Debounced(std::time::Duration),
    /// 手动热加载
    /// Manual hot-reload
    Manual,
    /// 空闲时热加载
    /// Hot-reload on idle
    OnIdle,
}

impl Default for ReloadStrategy {
    fn default() -> Self {
        Self::Debounced(std::time::Duration::from_secs(1))
    }
}

/// 热加载配置
/// Hot-reload configuration
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// 热加载策略
    /// Hot-reload strategy
    pub strategy: ReloadStrategy,
    /// 是否保存状态
    /// Whether to preserve state
    pub preserve_state: bool,
    /// 自动回滚失败的热加载
    /// Auto-rollback failed hot-reloads
    pub auto_rollback: bool,
    /// 最大热加载尝试次数
    /// Maximum reload attempt count
    pub max_reload_attempts: u32,
    /// 热加载尝试间隔
    /// Hot-reload retry interval
    pub reload_cooldown: std::time::Duration,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            strategy: ReloadStrategy::default(),
            preserve_state: true,
            auto_rollback: true,
            max_reload_attempts: 3,
            reload_cooldown: std::time::Duration::from_secs(5),
        }
    }
}

impl HotReloadConfig {
    /// 创建新配置
    /// Create new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置热加载策略
    /// Set hot-reload strategy
    pub fn with_strategy(mut self, strategy: ReloadStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// 设置是否保存状态
    /// Set whether to preserve state
    pub fn with_preserve_state(mut self, preserve: bool) -> Self {
        self.preserve_state = preserve;
        self
    }

    /// 设置是否自动回滚
    /// Set whether to auto-rollback
    pub fn with_auto_rollback(mut self, auto_rollback: bool) -> Self {
        self.auto_rollback = auto_rollback;
        self
    }

    /// 设置最大尝试次数
    /// Set maximum attempt count
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_reload_attempts = max_attempts;
        self
    }

    /// 设置热加载间隔
    /// Set hot-reload interval
    pub fn with_reload_cooldown(mut self, cooldown: std::time::Duration) -> Self {
        self.reload_cooldown = cooldown;
        self
    }
}

/// 热加载事件
/// Hot-reload event
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ReloadEvent {
    /// 热加载开始
    /// Hot-reload started
    ReloadStarted {
        plugin_id: String,
        path: std::path::PathBuf,
    },
    /// 热加载完成
    /// Hot-reload completed
    ReloadCompleted {
        plugin_id: String,
        path: std::path::PathBuf,
        success: bool,
        duration: std::time::Duration,
    },
    /// 热加载失败
    /// Hot-reload failed
    ReloadFailed {
        plugin_id: String,
        path: std::path::PathBuf,
        error: String,
        attempt: u32,
    },
    /// 回滚已触发
    /// Rollback triggered
    RollbackTriggered { plugin_id: String, reason: String },
    /// 插件已发现
    /// Plugin discovered
    PluginDiscovered { path: std::path::PathBuf },
    /// 插件已移除
    /// Plugin removed
    PluginRemoved {
        plugin_id: String,
        path: std::path::PathBuf,
    },
    /// 插件状态已保存
    /// Plugin state preserved
    StatePreserved { plugin_id: String },
    /// 插件状态已恢复
    /// Plugin state restored
    StateRestored { plugin_id: String },
}

/// 支持热加载的插件 trait
/// Plugin trait supporting hot-reload
#[async_trait::async_trait]
pub trait HotReloadable: Send + Sync {
    /// 刷新插件内容
    /// Refresh plugin content
    async fn refresh(&self) -> PluginResult<()>;

    /// 保存当前状态
    /// Save current state
    async fn save_state(&self) -> PluginResult<()> {
        Ok(())
    }

    /// 恢复状态
    /// Restore state
    async fn restore_state(&self) -> PluginResult<()> {
        Ok(())
    }
}

/// 核心插件 trait
/// Core plugin trait
#[async_trait::async_trait]
pub trait AgentPlugin: Send + Sync {
    /// 获取插件元数据
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;

    /// 获取插件 ID（便捷方法）
    /// Get plugin ID (convenience method)
    fn plugin_id(&self) -> &str {
        &self.metadata().id
    }

    /// 获取插件类型
    /// Get plugin type
    fn plugin_type(&self) -> PluginType {
        self.metadata().plugin_type.clone()
    }

    /// 获取插件状态
    /// Get plugin state
    fn state(&self) -> PluginState;

    /// 插件加载（分配资源）
    /// Plugin load (allocate resources)
    async fn load(&mut self, ctx: &PluginContext) -> PluginResult<()>;

    /// 插件初始化（配置初始化）
    /// Plugin initialization (config init)
    async fn init_plugin(&mut self) -> PluginResult<()>;

    /// 插件启动
    /// Plugin start
    async fn start(&mut self) -> PluginResult<()>;

    /// 插件暂停
    /// Plugin pause
    async fn pause(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// 插件恢复
    /// Plugin resume
    async fn resume(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// 插件停止
    /// Plugin stop
    async fn stop(&mut self) -> PluginResult<()>;

    /// 插件卸载（释放资源）
    /// Plugin unload (release resources)
    async fn unload(&mut self) -> PluginResult<()>;

    /// 执行插件功能
    /// Execute plugin function
    async fn execute(&mut self, input: String) -> PluginResult<String>;

    /// 健康检查
    /// Health check
    async fn health_check(&self) -> PluginResult<bool> {
        Ok(self.state() == PluginState::Running)
    }

    /// 获取插件统计信息
    /// Get plugin statistics
    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    /// 转换为 Any（用于向下转型）
    /// Convert to Any (for downcasting)
    fn as_any(&self) -> &dyn Any;

    /// 转换为可变 Any
    /// Convert to mutable Any
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// 消费并转换为 Any（用于提取具体的插件类型）
    /// Consume and convert to Any (for extracting concrete type)
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

// ============================================================================
// 插件类型 definition
// ============================================================================

/// 插件类型枚举
/// Plugin type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PluginType {
    /// LLM 能力插件
    /// LLM capability plugin
    LLM,
    /// 工具调用插件
    /// Tool calling plugin
    Tool,
    /// 存储插件
    /// Storage plugin
    Storage,
    /// 记忆管理插件
    /// Memory management plugin
    Memory,
    /// 向量数据库插件
    /// Vector database plugin
    VectorDB,
    /// 通信插件
    /// Communication plugin
    Communication,
    /// 监控插件
    /// Monitor plugin
    Monitor,
    /// Agent Skills 插件
    /// Agent Skills plugin
    Skill,
    /// 自定义插件
    /// Custom plugin
    Custom(String),
}

/// 插件状态
/// Plugin state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PluginState {
    /// 未初始化
    /// Unloaded
    Unloaded,
    /// 正在加载
    /// Loading
    Loading,
    /// 已加载（就绪）
    /// Loaded (ready)
    Loaded,
    /// 运行中
    /// Running
    Running,
    /// 已暂停
    /// Paused
    Paused,
    /// 错误状态
    /// Error state
    Error(String),
}

/// 插件优先级（用于确定执行顺序）
/// Plugin priority (for execution order)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum PluginPriority {
    Low = 0,
    #[default]
    Normal = 50,
    High = 100,
    Critical = 200,
}

// ============================================================================
// 插件元数据
// ============================================================================

/// 插件元数据
/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// 插件唯一标识
    /// Unique plugin identifier
    pub id: String,
    /// 插件名称
    /// Plugin name
    pub name: String,
    /// 插件版本
    /// Plugin version
    pub version: String,
    /// 插件描述
    /// Plugin description
    pub description: String,
    /// 插件类型
    /// Plugin type
    pub plugin_type: PluginType,
    /// 插件优先级
    /// Plugin priority
    pub priority: PluginPriority,
    /// 依赖的其他插件 ID
    /// IDs of dependent plugins
    pub dependencies: Vec<String>,
    /// 插件能力标签
    /// Plugin capability tags
    pub capabilities: Vec<String>,
    /// 插件作者
    /// Plugin author
    pub author: Option<String>,
}

impl PluginMetadata {
    pub fn new(id: &str, name: &str, plugin_type: PluginType) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            plugin_type,
            priority: PluginPriority::Normal,
            dependencies: Vec::new(),
            capabilities: Vec::new(),
            author: None,
        }
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_priority(mut self, priority: PluginPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_dependency(mut self, dep_id: &str) -> Self {
        self.dependencies.push(dep_id.to_string());
        self
    }

    pub fn with_capability(mut self, cap: &str) -> Self {
        self.capabilities.push(cap.to_string());
        self
    }
}

// ============================================================================
// 插件配置
// ============================================================================

/// 插件配置
/// Plugin configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginConfig {
    /// 配置项
    /// Configuration items
    pub settings: HashMap<String, serde_json::Value>,
    /// 是否启用
    /// Whether enabled
    pub enabled: bool,
    /// 自动启动
    /// Auto start
    pub auto_start: bool,
}

impl PluginConfig {
    pub fn new() -> Self {
        Self {
            settings: HashMap::new(),
            enabled: true,
            auto_start: true,
        }
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.settings
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn set<T: Serialize>(&mut self, key: &str, value: T) {
        if let Ok(v) = serde_json::to_value(value) {
            self.settings.insert(key.to_string(), v);
        }
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get(key)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key)
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key)
    }
}

// ============================================================================
// 插件上下文
// ============================================================================

/// 插件执行上下文
/// Plugin execution context
#[derive(Debug, Default)]
pub struct PluginContext {
    /// 智能体 ID
    /// Agent ID
    pub agent_id: String,
    /// 共享状态
    /// Shared state
    shared_state: Arc<RwLock<HashMap<String, Box<dyn Any + Send + Sync>>>>,
    /// 插件配置
    /// Plugin configuration
    pub config: PluginConfig,
    /// 事件发送器
    /// Event sender
    event_tx: Option<tokio::sync::mpsc::Sender<PluginEvent>>,
}

impl PluginContext {
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            shared_state: Arc::new(RwLock::new(HashMap::new())),
            config: PluginConfig::new(),
            event_tx: None,
        }
    }

    pub fn with_config(mut self, config: PluginConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_event_sender(mut self, tx: tokio::sync::mpsc::Sender<PluginEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// 获取共享状态
    /// Get shared state
    pub async fn get_state<T: Clone + Send + Sync + 'static>(&self, key: &str) -> Option<T> {
        let state = self.shared_state.read().await;
        state.get(key).and_then(|v| v.downcast_ref::<T>().cloned())
    }

    /// 设置共享状态
    /// Set shared state
    pub async fn set_state<T: Clone + Send + Sync + 'static>(&self, key: &str, value: T) {
        let mut state = self.shared_state.write().await;
        state.insert(key.to_string(), Box::new(value));
    }

    /// 发送插件事件
    /// Emit plugin event
    pub async fn emit_event(&self, event: PluginEvent) -> PluginResult<()> {
        if let Some(ref tx) = self.event_tx {
            tx.send(event)
                .await
                .map_err(|e| PluginError::Other(format!("Failed to send event: {}", e)))?;
        }
        Ok(())
    }
}

impl Clone for PluginContext {
    fn clone(&self) -> Self {
        Self {
            agent_id: self.agent_id.clone(),
            shared_state: self.shared_state.clone(),
            config: self.config.clone(),
            event_tx: self.event_tx.clone(),
        }
    }
}

// ============================================================================
// 插件事件
// ============================================================================

/// 插件事件
/// Plugin event
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PluginEvent {
    /// 插件已加载
    /// Plugin loaded
    PluginLoaded { plugin_id: String },
    /// 插件已卸载
    /// Plugin unloaded
    PluginUnloaded { plugin_id: String },
    /// 插件状态变化
    /// Plugin state changed
    StateChanged {
        plugin_id: String,
        old_state: PluginState,
        new_state: PluginState,
    },
    /// 插件错误
    /// Plugin error
    PluginError { plugin_id: String, error: String },
    /// 自定义事件
    /// Custom event
    Custom {
        plugin_id: String,
        event_type: String,
        data: Vec<u8>,
    },
}

#[cfg(test)]
mod tests;

