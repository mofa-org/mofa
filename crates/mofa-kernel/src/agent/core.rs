//! MoFA Agent 核心接口 - 统一抽象
//!
//! 本模块定义了 MoFA 框架的统一 Agent 抽象，遵循微内核架构原则：
//! - 核心统一：MoFAAgent 提供唯一的 Agent 接口
//! - 可选扩展：通过扩展 trait 提供额外功能
//! - 清晰层次：核心接口 + 可选扩展
//!
//! # 架构设计
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    MoFAAgent (统一核心接口)                          │
//! │  • id(), name(), capabilities()                                     │
//! │  • initialize(), execute(), shutdown()                              │
//! │  • state()                                                          │
//! └─────────────────────────────────────────────────────────────────────┘
//!                               │
//!         ┌─────────────────────┼─────────────────────┐
//!         ▼                     ▼                     ▼
//! ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
//! │AgentLifecycle│    │AgentMessaging│    │AgentPlugin   │
//! │  (可选扩展)   │    │  (可选扩展)   │    │  (可选扩展)   │
//! │              │    │              │    │   Support    │
//! │• pause()     │    │• handle_     │    │              │
//! │• resume()    │    │  message()   │    │• register_   │
//! │              │    │• handle_     │    │  plugin()    │
//! │              │    │  event()     │    │• unregister  │
//! └──────────────┘    └──────────────┘    │  _plugin()   │
//!                                         └──────────────┘
//! ```

pub use crate::agent::context::AgentEvent;
use crate::agent::{
    error::AgentResult,
    types::{AgentInput, AgentOutput, AgentState, InterruptResult},
    AgentCapabilities, AgentContext,
};
use async_trait::async_trait;

// ============================================================================
// MoFAAgent - 统一核心接口
// ============================================================================

/// MoFA Agent 统一接口
///
/// 这是 MoFA 框架中所有 Agent 必须实现的统一接口。
/// 整合了之前的 AgentCore 和 MoFAAgent 功能，提供一致的抽象。
///
/// # 设计原则
///
/// 1. **统一接口**：所有 Agent 实现同一个 trait
/// 2. **最小化核心**：只包含最基本的方法
/// 3. **可选扩展**：通过扩展 trait 提供额外功能
/// 4. **向后兼容**：保留类型别名以兼容旧代码
///
/// # 必须实现的方法
///
/// - `id()` - 唯一标识符
/// - `name()` - 人类可读名称
/// - `capabilities()` - 能力描述
/// - `initialize()` - 初始化
/// - `execute()` - 执行任务（核心方法）
/// - `shutdown()` - 关闭
/// - `state()` - 获取状态
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::core::MoFAAgent;
/// use mofa_kernel::agent::prelude::*;
///
/// struct MyAgent {
///     id: String,
///     name: String,
///     capabilities: AgentCapabilities,
///     state: AgentState,
/// }
///
/// #[async_trait]
/// impl MoFAAgent for MyAgent {
///     fn id(&self) -> &str { &self.id }
///     fn name(&self) -> &str { &self.name }
///     fn capabilities(&self) -> &AgentCapabilities { &self.capabilities }
///     fn state(&self) -> AgentState { self.state.clone() }
///
///     async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
///         self.state = AgentState::Ready;
///         Ok(())
///     }
///
///     async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
///         // 处理输入并返回输出
///         Ok(AgentOutput::text("Response"))
///     }
///
///     async fn shutdown(&mut self) -> AgentResult<()> {
///         self.state = AgentState::Shutdown;
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait MoFAAgent: Send + Sync + 'static {
    // ========================================================================
    // 标识和元数据
    // ========================================================================

    /// 获取唯一标识符
    ///
    /// ID 应该在 Agent 的整个生命周期内保持不变。
    /// 用于在注册中心、日志、监控系统中唯一标识 Agent。
    fn id(&self) -> &str;

    /// 获取人类可读名称
    ///
    /// 名称用于显示和日志记录，不需要唯一。
    fn name(&self) -> &str;

    /// 获取能力描述
    ///
    /// 返回 Agent 支持的能力，用于：
    /// - Agent 发现和路由
    /// - 能力匹配和验证
    /// - 多智能体协调
    fn capabilities(&self) -> &AgentCapabilities;

    // ========================================================================
    // 核心生命周期
    // ========================================================================

    /// 初始化 Agent
    ///
    /// 在执行任务前调用，用于：
    /// - 加载资源和配置
    /// - 建立连接（数据库、网络等）
    /// - 初始化内部状态
    ///
    /// # 参数
    ///
    /// - `ctx`: 执行上下文，提供配置、事件总线等
    ///
    /// # 状态转换
    ///
    /// Created -> Initializing -> Ready
    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()>;

    /// 执行任务 - 核心方法
    ///
    /// 这是 Agent 的主要执行方法，处理输入并返回输出。
    ///
    /// # 参数
    ///
    /// - `input`: 输入数据，支持多种格式（文本、JSON、二进制等）
    /// - `ctx`: 执行上下文，提供状态、事件、中断等
    ///
    /// # 返回
    ///
    /// 返回执行结果，包含输出内容、元数据、工具使用等。
    ///
    /// # 状态转换
    ///
    /// Ready -> Executing -> Ready
    async fn execute(
        &mut self,
        input: AgentInput,
        ctx: &AgentContext,
    ) -> AgentResult<AgentOutput>;

    /// 关闭 Agent
    ///
    /// 优雅关闭，释放资源：
    /// - 保存状态
    /// - 关闭连接
    /// - 清理资源
    ///
    /// # 状态转换
    ///
    /// * -> ShuttingDown -> Shutdown
    async fn shutdown(&mut self) -> AgentResult<()>;

    /// 中断 Agent
    ///
    /// 发送中断信号，Agent 可以选择如何响应：
    /// - 立即停止
    /// - 完成当前步骤后停止
    /// - 忽略中断
    ///
    /// # 返回
    ///
    /// 返回中断处理结果。
    ///
    /// # 默认实现
    ///
    /// 默认返回 `InterruptResult::Acknowledged`。
    ///
    /// # 状态转换
    ///
    /// Executing -> Interrupted
    async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
        Ok(InterruptResult::Acknowledged)
    }

    // ========================================================================
    // 状态查询
    // ========================================================================

    /// 获取当前状态
    ///
    /// 返回 Agent 的当前状态，用于：
    /// - 健康检查
    /// - 状态监控
    /// - 状态转换验证
    fn state(&self) -> AgentState;
}

// ============================================================================
// AgentLifecycle - 生命周期扩展
// ============================================================================

/// Agent 生命周期扩展
///
/// 提供额外的生命周期控制方法，用于需要更细粒度控制的场景。
/// 这个 trait 是可选的，只有需要这些功能的 Agent 才需要实现。
///
/// # 提供的方法
///
/// - `pause()` - 暂停执行
/// - `resume()` - 恢复执行
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::core::AgentLifecycle;
///
/// #[async_trait]
/// impl AgentLifecycle for MyAgent {
///     async fn pause(&mut self) -> AgentResult<()> {
///         self.state = AgentState::Paused;
///         Ok(())
///     }
///
///     async fn resume(&mut self) -> AgentResult<()> {
///         self.state = AgentState::Ready;
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait AgentLifecycle: MoFAAgent {
    /// 暂停 Agent
    ///
    /// 暂停当前执行，保持状态以便后续恢复。
    ///
    /// # 状态转换
    ///
    /// Executing -> Paused
    async fn pause(&mut self) -> AgentResult<()>;

    /// 恢复 Agent
    ///
    /// 从暂停状态恢复执行。
    ///
    /// # 状态转换
    ///
    /// Paused -> Ready
    async fn resume(&mut self) -> AgentResult<()>;
}

// ============================================================================
// AgentMessaging - 消息处理扩展
// ============================================================================

/// Agent 消息处理扩展
///
/// 提供消息和事件处理能力，用于需要与其他 Agent 或系统交互的场景。
/// 这个 trait 是可选的，只有需要消息处理的 Agent 才需要实现。
///
/// # 提供的方法
///
/// - `handle_message()` - 处理消息
/// - `handle_event()` - 处理事件
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::core::AgentMessaging;
///
/// #[async_trait]
/// impl AgentMessaging for MyAgent {
///     async fn handle_message(&mut self, msg: AgentMessage) -> AgentResult<AgentMessage> {
///         // 处理消息并返回响应
///         Ok(AgentMessage::new("response"))
///     }
///
///     async fn handle_event(&mut self, event: AgentEvent) -> AgentResult<()> {
///         // 处理事件
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait AgentMessaging: MoFAAgent {
    /// 处理消息
    ///
    /// 接收来自其他 Agent 或系统的消息，处理后返回响应。
    ///
    /// # 参数
    ///
    /// - `msg`: 接收到的消息
    ///
    /// # 返回
    ///
    /// 返回响应消息
    async fn handle_message(&mut self, msg: AgentMessage) -> AgentResult<AgentMessage>;

    /// 处理事件
    ///
    /// 处理来自事件总线的事件，用于响应系统级别的通知。
    ///
    /// # 参数
    ///
    /// - `event`: 事件对象
    async fn handle_event(&mut self, event: AgentEvent) -> AgentResult<()>;
}

// ============================================================================
// AgentPluginSupport - 插件支持扩展
// ============================================================================

/// Agent 插件支持扩展
///
/// 提供插件管理能力，允许在运行时动态扩展 Agent 功能。
/// 这个 trait 是可选的，只有需要插件系统的 Agent 才需要实现。
///
/// # 提供的方法
///
/// - `register_plugin()` - 注册插件
/// - `unregister_plugin()` - 注销插件
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::core::AgentPluginSupport;
///
/// #[async_trait]
/// impl AgentPluginSupport for MyAgent {
///     fn register_plugin(&mut self, plugin: Box<dyn AgentPlugin>) -> AgentResult<()> {
///         self.plugins.push(plugin);
///         Ok(())
///     }
///
///     fn unregister_plugin(&mut self, plugin_id: &str) -> AgentResult<()> {
///         self.plugins.retain(|p| p.id() != plugin_id);
///         Ok(())
///     }
/// }
/// ```
pub trait AgentPluginSupport: MoFAAgent {
    /// 注册插件
    ///
    /// 向 Agent 注册一个新插件，扩展其功能。
    ///
    /// # 参数
    ///
    /// - `plugin`: 插件对象，实现 AgentPlugin trait
    ///
    /// # 返回
    ///
    /// 返回错误如果插件已存在或注册失败
    fn register_plugin(&mut self, plugin: Box<dyn crate::plugin::AgentPlugin>) -> AgentResult<()>;

    /// 注销插件
    ///
    /// 从 Agent 中移除一个插件。
    ///
    /// # 参数
    ///
    /// - `plugin_id`: 要移除的插件 ID
    ///
    /// # 返回
    ///
    /// 返回错误如果插件不存在
    fn unregister_plugin(&mut self, plugin_id: &str) -> AgentResult<()>;
}

// ============================================================================
// 辅助类型
// ============================================================================

/// Agent 消息
///
/// 用于 Agent 之间的通信
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentMessage {
    /// 消息类型
    #[serde(rename = "type")]
    pub msg_type: String,

    /// 消息内容
    pub content: serde_json::Value,

    /// 发送者 ID
    pub sender_id: String,

    /// 接收者 ID
    pub recipient_id: String,

    /// 时间戳
    pub timestamp: i64,

    /// 消息 ID
    pub id: String,
}

impl AgentMessage {
    /// 创建新消息
    pub fn new(msg_type: impl Into<String>) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        Self {
            msg_type: msg_type.into(),
            content: serde_json::json!({}),
            sender_id: String::new(),
            recipient_id: String::new(),
            timestamp,
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// 设置内容
    pub fn with_content(mut self, content: serde_json::Value) -> Self {
        self.content = content;
        self
    }

    /// 设置发送者
    pub fn with_sender(mut self, sender_id: impl Into<String>) -> Self {
        self.sender_id = sender_id.into();
        self
    }

    /// 设置接收者
    pub fn with_recipient(mut self, recipient_id: impl Into<String>) -> Self {
        self.recipient_id = recipient_id.into();
        self
    }
}
