// 插件系统
// Plugin system
//!
//! 提供动态插件机制，允许用户在运行时扩展和控制上下文内容
//! Provides a dynamic plugin mechanism, allowing users to extend and control context at runtime
//!
//! 插件可以在以下阶段介入：
//! Plugins can intervene in the following stages:
//! 1. 请求处理前：预处理用户输入
//! 1. Pre-request: Pre-process user input
//! 2. 上下文组装前：动态添加/修改上下文内容
//! 2. Pre-context: Dynamically add/modify context content
//! 3. LLM响应后：后处理LLM返回结果
//! 3. Post-response: Post-process LLM response results
//!
//! 插件可以是HTTP请求、自定义函数等任何实现了Plugin trait的类型
//! Plugins can be HTTP requests, custom functions, or any type implementing the Plugin trait
//! 运行时层提供执行器与默认注册中心实现。
//! The runtime layer provides executors and default registry implementations.

use crate::agent::context::AgentContext;
use crate::agent::error::AgentResult;
use crate::agent::types::{AgentInput, AgentOutput};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// 插件接口
// Plugin Interface
// ============================================================================

/// 插件执行阶段
/// Plugin execution stage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginStage {
    /// 请求处理前
    /// Pre-request processing
    PreRequest,
    /// 上下文组装前
    /// Pre-context assembly
    PreContext,
    /// LLM响应后
    /// Post-LLM response
    PostResponse,
    /// 整个流程完成后
    /// Post-process completion
    PostProcess,
}

/// 插件元数据
/// Plugin metadata
#[derive(Debug, Clone, Default)]
pub struct PluginMetadata {
    /// 插件名称
    /// Plugin name
    pub name: String,
    /// 插件描述
    /// Plugin description
    pub description: String,
    /// 插件版本
    /// Plugin version
    pub version: String,
    /// 支持的执行阶段
    /// Supported execution stages
    pub stages: Vec<PluginStage>,
    /// 自定义属性
    /// Custom attributes
    pub custom: HashMap<String, String>,
}

/// 插件接口
/// Plugin interface
#[async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件名称
    /// Get plugin name
    fn name(&self) -> &str;

    /// 获取插件描述
    /// Get plugin description
    fn description(&self) -> &str;

    /// 获取插件元数据
    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// 在请求处理前执行
    /// Executed before request processing
    /// 可以修改输入内容
    /// Can modify input content
    async fn pre_request(&self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentInput> {
        Ok(input)
    }

    /// 在上下文组装前执行
    /// Executed before context assembly
    /// 可以动态修改上下文
    /// Can dynamically modify context
    async fn pre_context(&self, _ctx: &AgentContext) -> AgentResult<()> {
        Ok(())
    }

    /// 在LLM响应后执行
    /// Executed after LLM response
    /// 可以修改LLM返回的结果
    /// Can modify results returned by LLM
    async fn post_response(
        &self,
        output: AgentOutput,
        _ctx: &AgentContext,
    ) -> AgentResult<AgentOutput> {
        Ok(output)
    }

    /// 在整个流程完成后执行
    /// Executed after the entire process
    /// 可以进行清理或后续处理
    /// Can perform cleanup or follow-up
    async fn post_process(&self, _ctx: &AgentContext) -> AgentResult<()> {
        Ok(())
    }
}

// ============================================================================
// 插件注册中心
// Plugin Registry
// ============================================================================

/// 插件注册中心
/// Plugin registry
pub trait PluginRegistry: Send + Sync {
    /// 注册插件
    /// Register plugin
    fn register(&self, plugin: Arc<dyn Plugin>) -> AgentResult<()>;

    /// 批量注册插件
    /// Batch register plugins
    fn register_all(&self, plugins: Vec<Arc<dyn Plugin>>) -> AgentResult<()> {
        for plugin in plugins {
            self.register(plugin)?;
        }
        Ok(())
    }

    /// 移除插件
    /// Remove plugin
    fn unregister(&self, name: &str) -> AgentResult<bool>;

    /// 获取插件
    /// Get plugin
    fn get(&self, name: &str) -> Option<Arc<dyn Plugin>>;

    /// 列出所有插件
    /// List all plugins
    fn list(&self) -> Vec<Arc<dyn Plugin>>;

    /// 列出指定阶段的插件
    /// List plugins for a specific stage
    fn list_by_stage(&self, stage: PluginStage) -> Vec<Arc<dyn Plugin>>;

    /// 检查插件是否存在
    /// Check if plugin exists
    fn contains(&self, name: &str) -> bool;

    /// 插件数量
    /// Plugin count
    fn count(&self) -> usize;
}
