//! 配置系统
//! Configuration system
//!
//! 提供 Agent 的配置加载和管理
//! Provides configuration loading and management for Agents

#[cfg(feature = "config")]
pub mod loader;
pub mod schema;

#[cfg(feature = "config")]
pub use loader::{ConfigFormat, ConfigLoader};
pub use schema::{
    AgentConfig, AgentType, CapabilitiesConfig, ComponentsConfig, LlmAgentConfig, ReActAgentConfig,
    TeamAgentConfig, WorkflowAgentConfig,
};
