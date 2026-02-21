//! 统一 Agent 框架
//!
//! 提供模块化、可组合、可扩展的 Agent 架构
//!
//! # 架构概述
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         统一 Agent 框架                              │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────────────────────────────────────────────────┐    │
//! │  │                    MoFAAgent Trait                            │    │
//! │  │  (统一 Agent 接口：id, capabilities, execute, interrupt)      │    │
//! │  └───────────────────────────┬─────────────────────────────────┘    │
//! │                              │                                       │
//! │  ┌───────────────────────────┼───────────────────────────────────┐  │
//! │  │           Modular Components (组件化设计)                      │  │
//! │  │  ┌──────────┐  ┌──────────┐  ┌────────┐  ┌──────────────┐    │  │
//! │  │  │ Reasoner │  │  Tool    │  │ Memory │  │  Coordinator │    │  │
//! │  │  │   推理器  │  │  工具    │  │ 记忆    │  │   协调器      │    │  │
//! │  │  └──────────┘  └──────────┘  └────────┘  └──────────────┘    │  │
//! │  └───────────────────────────────────────────────────────────────┘  │
//! │                                                                      │
//! │  ┌─────────────────────────────────────────────────────────────┐    │
//! │  │        AgentRegistry (runtime 注册中心实现)                    │    │
//! │  └─────────────────────────────────────────────────────────────┘    │
//! │                                                                      │
//! │  ┌─────────────────────────────────────────────────────────────┐    │
//! │  │               CoreAgentContext (统一上下文)                       │    │
//! │  └─────────────────────────────────────────────────────────────┘    │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 核心概念
//!
//! ## MoFAAgent Trait
//!
//! 所有 Agent 实现的统一接口：
//!
//! ```rust,ignore
//! use mofa_kernel::agent::prelude::*;
//!
//! #[async_trait]
//! impl MoFAAgent for MyAgent {
//!     fn id(&self) -> &str { "my-agent" }
//!     fn name(&self) -> &str { "My Agent" }
//!     fn capabilities(&self) -> &AgentCapabilities { &self.caps }
//!
//!     async fn initialize(&mut self, ctx: &CoreAgentContext) -> AgentResult<()> {
//!         Ok(())
//!     }
//!
//!     async fn execute(&mut self, input: AgentInput, ctx: &CoreAgentContext) -> AgentResult<AgentOutput> {
//!         Ok(AgentOutput::text("Hello!"))
//!     }
//!
//!     async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
//!         Ok(InterruptResult::Acknowledged)
//!     }
//!
//!     async fn shutdown(&mut self) -> AgentResult<()> {
//!         Ok(())
//!     }
//!
//!     fn state(&self) -> AgentState {
//!         AgentState::Ready
//!     }
//! }
//! ```
//!
//! ## AgentCapabilities
//!
//! 描述 Agent 的能力，用于发现和路由：
//!
//! ```rust,ignore
//! let caps = AgentCapabilities::builder()
//!     .tag("llm")
//!     .tag("coding")
//!     .input_type(InputType::Text)
//!     .output_type(OutputType::Text)
//!     .supports_streaming(true)
//!     .supports_tools(true)
//!     .build();
//! ```
//!
//! ## CoreAgentContext
//!
//! 执行上下文，在 Agent 执行过程中传递状态：
//!
//! ```rust,ignore
//! let ctx = CoreAgentContext::new("execution-123");
//! ctx.set("user_id", "user-456").await;
//! ctx.emit_event(AgentEvent::new("task_started", json!({}))).await;
//! ```
//!
//! # 模块结构
//!
//! - `core` - AgentCore 微内核接口（最小化核心）
//! - `traits` - MoFAAgent trait 定义
//! - `types` - AgentInput, AgentOutput, AgentState 等类型
//! - `capabilities` - AgentCapabilities 能力描述
//! - `context` - CoreAgentContext 执行上下文
//! - `error` - 错误类型定义
//! - `components` - 组件 trait (Reasoner, Tool, Memory, Coordinator)
//! - `config` - 配置系统
//! - `registry` - Agent 注册中心
//! - `tools` - 统一工具系统

// 核心模块
pub mod capabilities;
pub mod context;
pub mod core;
pub mod error;
pub mod traits;
pub mod types;

// 组件模块
pub mod components;

// 配置模块
pub mod config;

// 注册中心
pub mod registry;

// 工具系统

// 执行引擎与运行器已迁移到 mofa-runtime

// 秘书Agent抽象
pub mod plugins;
pub mod secretary;

// AgentPlugin 统一到 plugin 模块
pub use crate::plugin::AgentPlugin;
// 重新导出核心类型
pub use capabilities::{
    AgentCapabilities, AgentCapabilitiesBuilder, AgentRequirements, AgentRequirementsBuilder,
    ReasoningStrategy,
};
pub use context::{AgentContext, AgentEvent, ContextConfig, EventBus};
pub use core::{
    // MoFAAgent - 统一的 Agent 接口
    AgentLifecycle,
    AgentMessage,
    AgentMessaging,
    AgentPluginSupport,
    MoFAAgent,
};
pub use error::{AgentError, AgentResult};
pub use traits::{AgentMetadata, AgentStats, DynAgent, HealthStatus};
pub use types::event::execution as execution_events;
// Event type constants are available via types::event::lifecycle, types::event::execution, etc.
// Note: Aliased to avoid conflict with existing modules (plugins, etc.)
pub use types::event::lifecycle;
pub use types::event::message as message_events;
pub use types::event::plugin as plugin_events;
pub use types::event::state as state_events;
pub use types::{
    AgentInput,
    AgentOutput,
    AgentState,
    // LLM types
    ChatCompletionRequest,
    ChatCompletionResponse,
    ChatMessage,
    ErrorCategory,
    ErrorContext,
    EventBuilder,
    GlobalError,
    GlobalEvent,
    GlobalMessage,
    GlobalResult,
    InputType,
    InterruptResult,
    LLMProvider,
    MessageContent,
    MessageMetadata,
    OutputContent,
    // Global types
    OutputType,
    ReasoningStep,
    ReasoningStepType,
    TokenUsage,
    ToolCall,
    ToolDefinition,
    ToolUsage,
};

// 重新导出组件
pub use components::{
    coordinator::{CoordinationPattern, Coordinator},
    mcp::{McpClient, McpServerConfig, McpServerInfo, McpToolInfo, McpTransportConfig},
    memory::{Memory, MemoryItem, MemoryStats, MemoryValue, Message, MessageRole},
    reasoner::{Reasoner, ReasoningResult},
    tool::{Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolResult},
};

// 重新导出工厂接口
pub use registry::AgentFactory;

// 重新导出配置
pub use config::{AgentConfig, AgentType};
#[cfg(feature = "config")]
pub use config::{ConfigFormat, ConfigLoader};

/// Prelude 模块 - 常用类型导入
pub mod prelude {
    pub use super::capabilities::{
        AgentCapabilities, AgentCapabilitiesBuilder, AgentRequirements, ReasoningStrategy,
    };
    pub use super::context::{AgentContext, AgentEvent, ContextConfig};
    pub use super::core::{
        // MoFAAgent - 统一的 Agent 接口
        AgentLifecycle,
        AgentMessage,
        AgentMessaging,
        AgentPluginSupport,
        MoFAAgent,
    };
    pub use super::error::{AgentError, AgentResult};
    pub use super::traits::{AgentMetadata, DynAgent, HealthStatus};
    pub use super::types::{
        AgentInput,
        AgentOutput,
        AgentState,
        // LLM types
        ChatCompletionRequest,
        ChatMessage,
        InputType,
        InterruptResult,
        LLMProvider,
        OutputType,
        TokenUsage,
        ToolUsage,
    };
    // AgentPlugin 统一到 plugin 模块
    pub use crate::plugin::AgentPlugin;
    pub use async_trait::async_trait;
}
