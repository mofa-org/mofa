// 插件系统
//!
//! 提供动态插件机制，允许用户在运行时扩展和控制上下文内容
//!
//! 插件可以在以下阶段介入：
//! 1. 请求处理前：预处理用户输入
//! 2. 上下文组装前：动态添加/修改上下文内容
//! 3. LLM响应后：后处理LLM返回结果
//!
//! 插件可以是HTTP请求、自定义函数等任何实现了Plugin trait的类型

use crate::agent::context::AgentContext;
use crate::agent::error::{AgentError, AgentResult};
use crate::agent::types::{AgentInput, AgentOutput};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

// 导入 simple_registry
mod simple_registry;
pub use simple_registry::SimplePluginRegistry;

// ============================================================================
// 插件接口
// ============================================================================

/// 插件执行阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStage {
    /// 请求处理前
    PreRequest,
    /// 上下文组装前
    PreContext,
    /// LLM响应后
    PostResponse,
    /// 整个流程完成后
    PostProcess,
}

/// 插件元数据
#[derive(Debug, Clone, Default)]
pub struct PluginMetadata {
    /// 插件名称
    pub name: String,
    /// 插件描述
    pub description: String,
    /// 插件版本
    pub version: String,
    /// 支持的执行阶段
    pub stages: Vec<PluginStage>,
    /// 自定义属性
    pub custom: HashMap<String, String>,
}

/// 插件接口
#[async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件名称
    fn name(&self) -> &str;

    /// 获取插件描述
    fn description(&self) -> &str;

    /// 获取插件元数据
    fn metadata(&self) -> PluginMetadata;

    /// 在请求处理前执行
    /// 可以修改输入内容
    async fn pre_request(&self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentInput> {
        Ok(input)
    }

    /// 在上下文组装前执行
    /// 可以动态修改上下文
    async fn pre_context(&self, ctx: &AgentContext) -> AgentResult<()> {
        Ok(())
    }

    /// 在LLM响应后执行
    /// 可以修改LLM返回的结果
    async fn post_response(&self, output: AgentOutput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
        Ok(output)
    }

    /// 在整个流程完成后执行
    /// 可以进行清理或后续处理
    async fn post_process(&self, ctx: &AgentContext) -> AgentResult<()> {
        Ok(())
    }
}

// ============================================================================
// 插件注册中心
// ============================================================================

/// 插件注册中心
pub trait PluginRegistry: Send + Sync {
    /// 注册插件
    fn register(&self, plugin: Arc<dyn Plugin>) -> AgentResult<()>;

    /// 批量注册插件
    fn register_all(&self, plugins: Vec<Arc<dyn Plugin>>) -> AgentResult<()> {
        for plugin in plugins {
            self.register(plugin)?;
        }
        Ok(())
    }

    /// 移除插件
    fn unregister(&self, name: &str) -> AgentResult<bool>;

    /// 获取插件
    fn get(&self, name: &str) -> Option<Arc<dyn Plugin>>;

    /// 列出所有插件
    fn list(&self) -> Vec<Arc<dyn Plugin>>;

    /// 列出指定阶段的插件
    fn list_by_stage(&self, stage: PluginStage) -> Vec<Arc<dyn Plugin>>;

    /// 检查插件是否存在
    fn contains(&self, name: &str) -> bool;

    /// 插件数量
    fn count(&self) -> usize;
}

// ============================================================================
// 插件执行器
// ============================================================================

/// 插件执行器
pub struct PluginExecutor {
    pub registry: Arc<dyn PluginRegistry>,
}

impl PluginExecutor {
    /// 创建插件执行器
    pub fn new(registry: Arc<dyn PluginRegistry>) -> Self {
        Self { registry }
    }

    /// 执行指定阶段的所有插件
    pub async fn execute_stage(
        &self,
        stage: PluginStage,
        ctx: &AgentContext,
    ) -> AgentResult<()> {
        let plugins = self.registry.list_by_stage(stage);
        for plugin in plugins {
            match stage {
                PluginStage::PreContext => {
                    plugin.pre_context(ctx).await?;
                }
                PluginStage::PostProcess => {
                    plugin.post_process(ctx).await?;
                }
                _ => {
                    // PreRequest 和 PostResponse 需要参数，单独处理
                    continue;
                }
            }
        }
        Ok(())
    }

    /// 执行PreRequest阶段的所有插件
    pub async fn execute_pre_request(
        &self,
        input: AgentInput,
        ctx: &AgentContext,
    ) -> AgentResult<AgentInput> {
        let mut result = input;
        let plugins = self.registry.list_by_stage(PluginStage::PreRequest);

        for plugin in plugins {
            result = plugin.pre_request(result.clone(), ctx).await?;
        }

        Ok(result)
    }

    /// 执行PostResponse阶段的所有插件
    pub async fn execute_post_response(
        &self,
        output: AgentOutput,
        ctx: &AgentContext,
    ) -> AgentResult<AgentOutput> {
        let mut result = output;
        let plugins = self.registry.list_by_stage(PluginStage::PostResponse);

        for plugin in plugins {
            result = plugin.post_response(result.clone(), ctx).await?;
        }

        Ok(result)
    }
}

// ============================================================================
// 内置插件示例
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
