//! Agent 工厂接口
//!
//! Kernel 仅保留抽象接口；注册中心实现位于运行时层 (mofa-runtime)。

use crate::agent::capabilities::AgentCapabilities;
use crate::agent::config::AgentConfig;
use crate::agent::core::MoFAAgent;
use crate::agent::error::AgentResult;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent 工厂 Trait
///
/// 负责创建特定类型的 Agent 实例
///
/// # 示例
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
    async fn create(&self, config: AgentConfig) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>>;

    /// 工厂类型标识
    fn type_id(&self) -> &str;

    /// 默认能力
    fn default_capabilities(&self) -> AgentCapabilities;

    /// 验证配置
    fn validate_config(&self, config: &AgentConfig) -> AgentResult<()> {
        let _ = config;
        Ok(())
    }

    /// 工厂描述
    fn description(&self) -> Option<&str> {
        None
    }
}
