// 插件系统
//!
//! 提供动态插件机制，允许用户在运行时扩展和控制上下文内容
//! 该模块基于 mofa-kernel 的插件抽象，并提供运行时示例实现

pub use mofa_kernel::agent::plugins::{Plugin, PluginMetadata, PluginRegistry, PluginStage};

use crate::agent::context::AgentContext;
use crate::agent::error::{AgentError, AgentResult};
use crate::agent::types::{AgentInput, AgentOutput};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Type alias for custom function handler in CustomFunctionPlugin
pub type CustomFunctionHandler =
    Arc<dyn Fn(AgentInput, &AgentContext) -> AgentResult<AgentInput> + Send + Sync + 'static>;

// ============================================================================
// 运行时插件注册中心
// ============================================================================

/// 简单插件注册中心实现
pub struct SimplePluginRegistry {
    plugins: RwLock<HashMap<String, Arc<dyn Plugin>>>,
}

impl SimplePluginRegistry {
    /// 创建新的插件注册中心
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for SimplePluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry for SimplePluginRegistry {
    fn register(&self, plugin: Arc<dyn Plugin>) -> AgentResult<()> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|_| AgentError::ExecutionFailed("Failed to acquire write lock".to_string()))?;
        plugins.insert(plugin.name().to_string(), plugin);
        Ok(())
    }

    fn unregister(&self, name: &str) -> AgentResult<bool> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|_| AgentError::ExecutionFailed("Failed to acquire write lock".to_string()))?;
        Ok(plugins.remove(name).is_some())
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().ok()?;
        plugins.get(name).cloned()
    }

    fn list(&self) -> Vec<Arc<dyn Plugin>> {
        self.plugins
            .read()
            .ok()
            .map(|plugins| plugins.values().cloned().collect())
            .unwrap_or_default()
    }

    fn list_by_stage(&self, stage: PluginStage) -> Vec<Arc<dyn Plugin>> {
        self.plugins
            .read()
            .ok()
            .map(|plugins| {
                plugins
                    .values()
                    .filter(|plugin| plugin.metadata().stages.contains(&stage))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn contains(&self, name: &str) -> bool {
        self.plugins
            .read()
            .ok()
            .map(|plugins| plugins.contains_key(name))
            .unwrap_or(false)
    }

    fn count(&self) -> usize {
        self.plugins
            .read()
            .ok()
            .map(|plugins| plugins.len())
            .unwrap_or(0)
    }
}

// ============================================================================
// 插件执行器（运行时层）
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
    pub async fn execute_stage(&self, stage: PluginStage, ctx: &AgentContext) -> AgentResult<()> {
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
        PluginMetadata {
            name: self.name.clone(),
            description: self.description.clone(),
            version: "1.0.0".to_string(),
            stages: vec![PluginStage::PreContext],
            ..Default::default()
        }
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
    func: CustomFunctionHandler,
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
        PluginMetadata {
            name: self.name.clone(),
            description: self.description.clone(),
            version: "1.0.0".to_string(),
            stages: vec![PluginStage::PreRequest],
            ..Default::default()
        }
    }

    async fn pre_request(&self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentInput> {
        (self.func)(input, ctx)
    }
}
