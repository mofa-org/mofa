use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 插件执行结果
pub type PluginResult<T> = anyhow::Result<T>;

// ============================================================================
// 热加载相关定义 (Hot-reload related definitions)
// ============================================================================

/// 热加载策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadStrategy {
    /// 立即热加载
    Immediate,
    /// 防抖热加载
    Debounced(std::time::Duration),
    /// 手动热加载
    Manual,
    /// 空闲时热加载
    OnIdle,
}

impl Default for ReloadStrategy {
    fn default() -> Self {
        Self::Debounced(std::time::Duration::from_secs(1))
    }
}

/// 热加载配置
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// 热加载策略
    pub strategy: ReloadStrategy,
    /// 是否保存状态
    pub preserve_state: bool,
    /// 自动回滚失败的热加载
    pub auto_rollback: bool,
    /// 最大热加载尝试次数
    pub max_reload_attempts: u32,
    /// 热加载尝试间隔
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
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置热加载策略
    pub fn with_strategy(mut self, strategy: ReloadStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// 设置是否保存状态
    pub fn with_preserve_state(mut self, preserve: bool) -> Self {
        self.preserve_state = preserve;
        self
    }

    /// 设置是否自动回滚
    pub fn with_auto_rollback(mut self, auto_rollback: bool) -> Self {
        self.auto_rollback = auto_rollback;
        self
    }

    /// 设置最大尝试次数
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_reload_attempts = max_attempts;
        self
    }

    /// 设置热加载间隔
    pub fn with_reload_cooldown(mut self, cooldown: std::time::Duration) -> Self {
        self.reload_cooldown = cooldown;
        self
    }
}

/// 热加载事件
#[derive(Debug, Clone)]
pub enum ReloadEvent {
    /// 热加载开始
    ReloadStarted { plugin_id: String, path: std::path::PathBuf },
    /// 热加载完成
    ReloadCompleted {
        plugin_id: String,
        path: std::path::PathBuf,
        success: bool,
        duration: std::time::Duration
    },
    /// 热加载失败
    ReloadFailed {
        plugin_id: String,
        path: std::path::PathBuf,
        error: String,
        attempt: u32,
    },
    /// 回滚已触发
    RollbackTriggered {
        plugin_id: String,
        reason: String,
    },
    /// 插件已发现
    PluginDiscovered { path: std::path::PathBuf },
    /// 插件已移除
    PluginRemoved { plugin_id: String, path: std::path::PathBuf },
    /// 插件状态已保存
    StatePreserved { plugin_id: String },
    /// 插件状态已恢复
    StateRestored { plugin_id: String },
}

/// 支持热加载的插件 trait
#[async_trait::async_trait]
pub trait HotReloadable: Send + Sync {
    /// 刷新插件内容
    async fn refresh(&self) -> PluginResult<()>;

    /// 保存当前状态
    async fn save_state(&self) -> PluginResult<()> {
        Ok(())
    }

    /// 恢复状态
    async fn restore_state(&self) -> PluginResult<()> {
        Ok(())
    }
}

/// 核心插件 trait
#[async_trait::async_trait]
pub trait AgentPlugin: Send + Sync {
    /// 获取插件元数据
    fn metadata(&self) -> &PluginMetadata;

    /// 获取插件 ID（便捷方法）
    fn plugin_id(&self) -> &str {
        &self.metadata().id
    }

    /// 获取插件类型
    fn plugin_type(&self) -> PluginType {
        self.metadata().plugin_type.clone()
    }

    /// 获取插件状态
    fn state(&self) -> PluginState;

    /// 插件加载（分配资源）
    async fn load(&mut self, ctx: &PluginContext) -> PluginResult<()>;

    /// 插件初始化（配置初始化）
    async fn init_plugin(&mut self) -> PluginResult<()>;

    /// 插件启动
    async fn start(&mut self) -> PluginResult<()>;

    /// 插件暂停
    async fn pause(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// 插件恢复
    async fn resume(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// 插件停止
    async fn stop(&mut self) -> PluginResult<()>;

    /// 插件卸载（释放资源）
    async fn unload(&mut self) -> PluginResult<()>;

    /// 执行插件功能
    async fn execute(&mut self, input: String) -> PluginResult<String>;

    /// 健康检查
    async fn health_check(&self) -> PluginResult<bool> {
        Ok(self.state() == PluginState::Running)
    }

    /// 获取插件统计信息
    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    /// 转换为 Any（用于向下转型）
    fn as_any(&self) -> &dyn Any;

    /// 转换为可变 Any
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// 消费并转换为 Any（用于提取具体的插件类型）
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

// ============================================================================
// 插件类型定义
// ============================================================================

/// 插件类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginType {
    /// LLM 能力插件
    LLM,
    /// 工具调用插件
    Tool,
    /// 存储插件
    Storage,
    /// 记忆管理插件
    Memory,
    /// 向量数据库插件
    VectorDB,
    /// 通信插件
    Communication,
    /// 监控插件
    Monitor,
    /// Agent Skills 插件
    Skill,
    /// 自定义插件
    Custom(String),
}

/// 插件状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// 未初始化
    Unloaded,
    /// 正在加载
    Loading,
    /// 已加载（就绪）
    Loaded,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 错误状态
    Error(String),
}

/// 插件优先级（用于确定执行顺序）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// 插件唯一标识
    pub id: String,
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: String,
    /// 插件描述
    pub description: String,
    /// 插件类型
    pub plugin_type: PluginType,
    /// 插件优先级
    pub priority: PluginPriority,
    /// 依赖的其他插件 ID
    pub dependencies: Vec<String>,
    /// 插件能力标签
    pub capabilities: Vec<String>,
    /// 插件作者
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginConfig {
    /// 配置项
    pub settings: HashMap<String, serde_json::Value>,
    /// 是否启用
    pub enabled: bool,
    /// 自动启动
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
#[derive(Debug, Default)]
pub struct PluginContext {
    /// 智能体 ID
    pub agent_id: String,
    /// 共享状态
    shared_state: Arc<RwLock<HashMap<String, Box<dyn Any + Send + Sync>>>>,
    /// 插件配置
    pub config: PluginConfig,
    /// 事件发送器
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
    pub async fn get_state<T: Clone + Send + Sync + 'static>(&self, key: &str) -> Option<T> {
        let state = self.shared_state.read().await;
        state.get(key).and_then(|v| v.downcast_ref::<T>().cloned())
    }

    /// 设置共享状态
    pub async fn set_state<T: Clone + Send + Sync + 'static>(&self, key: &str, value: T) {
        let mut state = self.shared_state.write().await;
        state.insert(key.to_string(), Box::new(value));
    }

    /// 发送插件事件
    pub async fn emit_event(&self, event: PluginEvent) -> anyhow::Result<()> {
        if let Some(ref tx) = self.event_tx {
            tx.send(event)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send event: {}", e))?;
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
#[derive(Debug, Clone)]
pub enum PluginEvent {
    /// 插件已加载
    PluginLoaded { plugin_id: String },
    /// 插件已卸载
    PluginUnloaded { plugin_id: String },
    /// 插件状态变化
    StateChanged {
        plugin_id: String,
        old_state: PluginState,
        new_state: PluginState,
    },
    /// 插件错误
    PluginError { plugin_id: String, error: String },
    /// 自定义事件
    Custom {
        plugin_id: String,
        event_type: String,
        data: Vec<u8>,
    },
}
