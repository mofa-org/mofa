//! 统一工具注册中心
//!
//! 整合内置工具、MCP 工具、自定义工具的注册中心

use mofa_kernel::agent::components::tool::{
    Tool, ToolDescriptor, ToolRegistry as ToolRegistryTrait,
};
use mofa_kernel::agent::error::AgentResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// 统一工具注册中心
///
/// 整合多种工具来源的注册中心
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::agent::tools::ToolRegistry;
/// use mofa_foundation::agent::components::tool::EchoTool;
///
/// let mut registry = ToolRegistry::new();
///
/// // 注册内置工具
/// registry.register(Arc::new(EchoTool)).unwrap();
///
/// // 注册 MCP 服务器的工具
/// registry.load_mcp_server("http://localhost:8080").await?;
///
/// // 列出所有工具
/// for tool in registry.list() {
///     info!("{}: {}", tool.name, tool.description);
/// }
/// ```
pub struct ToolRegistry {
    /// 工具存储
    tools: HashMap<String, Arc<dyn Tool>>,
    /// 工具来源
    sources: HashMap<String, ToolSource>,
    /// MCP 客户端 (TODO: 实际 MCP 客户端实现)
    mcp_endpoints: Vec<String>,
}

/// 工具来源
#[derive(Debug, Clone)]
pub enum ToolSource {
    /// 内置工具
    Builtin,
    /// MCP 服务器
    Mcp { endpoint: String },
    /// 自定义插件
    Plugin { path: String },
    /// 动态注册
    Dynamic,
}

impl ToolRegistry {
    /// 创建新的统一注册中心
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            sources: HashMap::new(),
            mcp_endpoints: Vec::new(),
        }
    }

    /// 注册工具并记录来源
    pub fn register_with_source(&mut self, tool: Arc<dyn Tool>, source: ToolSource) -> AgentResult<()> {
        let name = tool.name().to_string();
        self.sources.insert(name.clone(), source);
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 加载 MCP 服务器的工具 (TODO: 实际 MCP 实现)
    pub async fn load_mcp_server(&mut self, endpoint: &str) -> AgentResult<Vec<String>> {
        // TODO: 实际 MCP 客户端实现
        // 这里只是记录端点
        self.mcp_endpoints.push(endpoint.to_string());

        // 模拟加载的工具名称
        Ok(vec![])
    }

    /// 卸载 MCP 服务器的工具
    pub async fn unload_mcp_server(&mut self, endpoint: &str) -> AgentResult<Vec<String>> {
        self.mcp_endpoints.retain(|e| e != endpoint);

        // 移除该服务器的工具
        let to_remove: Vec<String> = self
            .sources
            .iter()
            .filter_map(|(name, source)| {
                if let ToolSource::Mcp { endpoint: ep } = source
                    && ep == endpoint {
                        return Some(name.clone());
                    }
                None
            })
            .collect();

        for name in &to_remove {
            self.tools.remove(name);
            self.sources.remove(name);
        }

        Ok(to_remove)
    }

    /// 热加载插件 (TODO: 实际插件系统实现)
    pub async fn hot_reload_plugin(&mut self, _path: &str) -> AgentResult<Vec<String>> {
        // TODO: 实际插件热加载实现
        Ok(vec![])
    }

    /// 获取工具来源
    pub fn get_source(&self, name: &str) -> Option<&ToolSource> {
        self.sources.get(name)
    }

    /// 按来源过滤工具
    pub fn filter_by_source(&self, source_type: &str) -> Vec<ToolDescriptor> {
        self.tools
            .iter()
            .filter(|(name, _)| {
                if let Some(source) = self.sources.get(*name) {
                    match source {
                        ToolSource::Builtin => source_type == "builtin",
                        ToolSource::Mcp { .. } => source_type == "mcp",
                        ToolSource::Plugin { .. } => source_type == "plugin",
                        ToolSource::Dynamic => source_type == "dynamic",
                    }
                } else {
                    false
                }
            })
            .map(|(_, tool)| ToolDescriptor::from_tool(tool.as_ref()))
            .collect()
    }

    /// 获取 MCP 端点列表
    pub fn mcp_endpoints(&self) -> &[String] {
        &self.mcp_endpoints
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistryTrait for ToolRegistry {
    fn register(&mut self, tool: Arc<dyn Tool>) -> AgentResult<()> {
        self.register_with_source(tool, ToolSource::Dynamic)
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    fn unregister(&mut self, name: &str) -> AgentResult<bool> {
        self.sources.remove(name);
        Ok(self.tools.remove(name).is_some())
    }

    fn list(&self) -> Vec<ToolDescriptor> {
        self.tools
            .values()
            .map(|t| ToolDescriptor::from_tool(t.as_ref()))
            .collect()
    }

    fn list_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    fn count(&self) -> usize {
        self.tools.len()
    }
}

// ============================================================================
// 工具搜索
// ============================================================================

/// 工具搜索器
pub struct ToolSearcher<'a> {
    registry: &'a ToolRegistry,
}

impl<'a> ToolSearcher<'a> {
    /// 创建搜索器
    pub fn new(registry: &'a ToolRegistry) -> Self {
        Self { registry }
    }

    /// 按名称模糊搜索
    pub fn search_by_name(&self, pattern: &str) -> Vec<ToolDescriptor> {
        let pattern_lower = pattern.to_lowercase();
        self.registry
            .tools
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&pattern_lower))
            .map(|(_, tool)| ToolDescriptor::from_tool(tool.as_ref()))
            .collect()
    }

    /// 按描述搜索
    pub fn search_by_description(&self, query: &str) -> Vec<ToolDescriptor> {
        let query_lower = query.to_lowercase();
        self.registry
            .tools
            .values()
            .filter(|tool| tool.description().to_lowercase().contains(&query_lower))
            .map(|tool| ToolDescriptor::from_tool(tool.as_ref()))
            .collect()
    }

    /// 按标签搜索
    pub fn search_by_tag(&self, tag: &str) -> Vec<ToolDescriptor> {
        self.registry
            .tools
            .values()
            .filter(|tool| {
                let metadata = tool.metadata();
                metadata.tags.iter().any(|t| t == tag)
            })
            .map(|tool| ToolDescriptor::from_tool(tool.as_ref()))
            .collect()
    }

    /// 搜索需要确认的工具
    pub fn search_dangerous(&self) -> Vec<ToolDescriptor> {
        self.registry
            .tools
            .values()
            .filter(|tool| tool.metadata().is_dangerous || tool.requires_confirmation())
            .map(|tool| ToolDescriptor::from_tool(tool.as_ref()))
            .collect()
    }
}
