// 插件系统
//!
//! 提供动态插件机制，允许用户在运行时扩展和控制上下文内容
//! 该模块基于 mofa-kernel 的插件抽象，并提供运行时示例实现

pub use mofa_kernel::agent::plugins::{
    Plugin, PluginExecutor, PluginMetadata, PluginRegistry, PluginStage, SimplePluginRegistry,
};

use crate::agent::context::AgentContext;
use crate::agent::error::AgentResult;
use crate::agent::types::AgentInput;
use async_trait::async_trait;
use std::sync::Arc;

// ============================================================================
// 内置插件示例 (运行时层)
// ============================================================================

/// 示例HTTP请求插件
pub struct HttpPlugin {
    name: String,
    description: String,
    url: String,
}

impl HttpPlugin {
    /// 创建HTTP插件
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            name: "http-plugin".to_string(),
            description: "HTTP请求插件".to_string(),
            url: url.into(),
        }
    }
}

#[async_trait]
impl Plugin for HttpPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn metadata(&self) -> PluginMetadata {
        let mut metadata = PluginMetadata::default();
        metadata.name = self.name.clone();
        metadata.description = self.description.clone();
        metadata.version = "1.0.0".to_string();
        metadata.stages = vec![PluginStage::PreContext];
        metadata
    }

    async fn pre_context(&self, ctx: &AgentContext) -> AgentResult<()> {
        // 这里可以实现HTTP请求逻辑，并将结果存入上下文
        // 示例：将固定内容存入上下文
        ctx.set("http_response", "示例HTTP响应内容").await;
        Ok(())
    }
}

/// 示例自定义函数插件
pub struct CustomFunctionPlugin {
    name: String,
    description: String,
    func: Arc<dyn Fn(AgentInput, &AgentContext) -> AgentResult<AgentInput> + Send + Sync + 'static>,
}

impl CustomFunctionPlugin {
    /// 创建自定义函数插件
    pub fn new<F>(name: impl Into<String>, desc: impl Into<String>, func: F) -> Self
    where
        F: Fn(AgentInput, &AgentContext) -> AgentResult<AgentInput> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: desc.into(),
            func: Arc::new(func),
        }
    }
}

#[async_trait]
impl Plugin for CustomFunctionPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn metadata(&self) -> PluginMetadata {
        let mut metadata = PluginMetadata::default();
        metadata.name = self.name.clone();
        metadata.description = self.description.clone();
        metadata.version = "1.0.0".to_string();
        metadata.stages = vec![PluginStage::PreRequest];
        metadata
    }

    async fn pre_request(&self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentInput> {
        (self.func)(input, ctx)
    }
}
