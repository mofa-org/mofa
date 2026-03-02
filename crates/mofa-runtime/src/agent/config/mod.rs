//! 配置系统
//! Configuration system
//!
//! 提供 Agent 的配置加载和管理
//! Provides loading and management of Agent configurations

pub mod loader;
pub mod schema;

pub use loader::{ConfigFormat, ConfigLoader};
pub use schema::{
    AgentConfig, AgentType, CapabilitiesConfig, ComponentsConfig, LlmAgentConfig, ReActAgentConfig,
    TeamAgentConfig, WorkflowAgentConfig,
};
