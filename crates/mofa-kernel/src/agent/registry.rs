//! Agent 工厂接口
//! Agent Factory Interface
//!
//! Kernel 仅保留抽象接口；注册中心实现位于运行时层 (mofa-runtime)。
//! Kernel only keeps the abstract interface; the registry implementation is in the runtime layer (mofa-runtime).

use crate::agent::capabilities::AgentCapabilities;
use crate::agent::config::AgentConfig;
use crate::agent::core::MoFAAgent;
use crate::agent::error::AgentResult;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent 工厂 Trait
/// Agent Factory Trait
///
/// 负责创建特定类型的 Agent 实例
/// Responsible for creating specific types of Agent instances
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::agent::registry::AgentFactory;
/// use mofa_kernel::agent::config::AgentConfig;
///
/// struct LLMAgentFactory;
///
/// #[async_trait]
/// impl AgentFactory for LLMAgentFactory {
///     async fn create(&self, config: AgentConfig) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>> {
///         let _ = config;
///         unimplemented!()
///     }
///
///     fn type_id(&self) -> &str {
///         "llm"
///     }
///
///     fn default_capabilities(&self) -> AgentCapabilities {
///         AgentCapabilities::builder()
///             .with_tag("llm")
///             .with_tag("chat")
///             .build()
///     }
/// }
/// ```
#[async_trait]
pub trait AgentFactory: Send + Sync {
    /// 创建 Agent 实例
    /// Create an Agent instance
    async fn create(&self, config: AgentConfig) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>>;

    /// 工厂类型标识
    /// Factory type identifier
    fn type_id(&self) -> &str;

    /// 默认能力
    /// Default capabilities
    fn default_capabilities(&self) -> AgentCapabilities;

    /// 验证配置
    /// Validate configuration
    fn validate_config(&self, config: &AgentConfig) -> AgentResult<()> {
        let _ = config;
        Ok(())
    }

    /// 工厂描述
    /// Factory description
    fn description(&self) -> Option<&str> {
        None
    }
}
