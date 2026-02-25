//! 统一工具注册中心
//! Unified Tool Registry
//!
//! 整合内置工具、MCP 工具、自定义工具的注册中心
//! A registry that integrates builtin, MCP, and custom tools

use async_trait::async_trait;
use mofa_kernel::agent::components::tool::{
    Tool, ToolDescriptor, ToolRegistry as ToolRegistryTrait, DynTool, ToolExt,
};
use mofa_kernel::agent::error::{AgentError, AgentResult};
use std::collections::HashMap;
use std::sync::Arc;

/// 统一工具注册中心
/// Unified Tool Registry
///
/// 整合多种工具来源的注册中心
/// A registry integrating multiple tool sources
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::agent::tools::ToolRegistry;
/// use mofa_foundation::agent::components::tool::EchoTool;
///
/// let mut registry = ToolRegistry::new();
///
/// // 注册内置工具
/// // Register builtin tool
/// registry.register(Arc::new(EchoTool)).unwrap();
///
/// // 注册 MCP 服务器的工具
/// // Register tools from MCP server
/// registry.load_mcp_server("http://localhost:8080").await?;
///
/// // 列出所有工具
/// // List all tools
/// for tool in registry.list() {
///     info!("{}: {}", tool.name, tool.description);
/// }
/// ```
pub struct ToolRegistry {
    /// 工具存储
    /// Tool storage
    tools: HashMap<String, Arc<dyn DynTool>>,
    /// 工具来源
    /// Tool sources
    sources: HashMap<String, ToolSource>,
    /// MCP 端点列表
    /// MCP endpoint list
    mcp_endpoints: Vec<String>,
    /// MCP 客户端管理器 (仅在 mcp feature 启用时使用)
    /// MCP client manager (only used when mcp feature is enabled)
    #[cfg(feature = "mcp")]
    mcp_client: Option<std::sync::Arc<tokio::sync::RwLock<super::mcp::McpClientManager>>>,
}

/// 工具来源
/// Tool source
#[derive(Debug, Clone)]
pub enum ToolSource {
    /// 内置工具
    /// Builtin tool
    Builtin,
    /// MCP 服务器
    /// MCP server
    Mcp { endpoint: String },
    /// 自定义插件
    /// Custom plugin
    Plugin { path: String },
    /// 动态注册
    /// Dynamic registration
    Dynamic,
}

impl ToolRegistry {
    /// 创建新的统一注册中心
    /// Create a new unified registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            sources: HashMap::new(),
            mcp_endpoints: Vec::new(),
            #[cfg(feature = "mcp")]
            mcp_client: None,
        }
    }

    /// 注册工具并记录来源
    /// Register tool and record source
    pub fn register_with_source(
        &mut self,
        tool: Arc<dyn DynTool>,
        source: ToolSource,
    ) -> AgentResult<()> {
        let name = tool.name().to_string();
        self.sources.insert(name.clone(), source);
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 加载 MCP 服务器的工具
    /// Load MCP server tools
    ///
    /// 连接到 MCP 服务器，发现可用工具，并注册到工具注册中心。
    /// Connect to MCP server, discover available tools, and register them.
    ///
    /// # 参数
    /// # Parameters
    ///
    /// - `config`: MCP 服务器配置
    /// - `config`: MCP server configuration
    ///
    /// # 返回
    /// # Returns
    ///
    /// 成功注册的工具名称列表
    /// List of successfully registered tool names
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_kernel::agent::components::mcp::McpServerConfig;
    ///
    /// let config = McpServerConfig::stdio(
    ///     "github",
    ///     "npx",
    ///     vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
    /// );
    /// let tool_names = registry.load_mcp_server(config).await?;
    /// println!("Loaded {} MCP tools", tool_names.len());
    /// ```
    #[cfg(feature = "mcp")]
    pub async fn load_mcp_server(
        &mut self,
        config: mofa_kernel::agent::components::mcp::McpServerConfig,
    ) -> AgentResult<Vec<String>> {
        use mofa_kernel::agent::components::mcp::McpClient;

        let endpoint = config.name.clone();
        self.mcp_endpoints.push(endpoint.clone());

        // Create or get the shared MCP client manager
        let client = self
            .mcp_client
            .get_or_insert_with(|| {
                std::sync::Arc::new(tokio::sync::RwLock::new(super::mcp::McpClientManager::new()))
            })
            .clone();

        // Connect to the MCP server
        {
            let mut client_guard = client.write().await;
            client_guard.connect(config).await?;
        }

        // List available tools
        let tools = {
            let client_guard = client.read().await;
            client_guard.list_tools(&endpoint).await?
        };

        // Register each MCP tool as a kernel Tool
        let mut registered_names = Vec::new();
        for tool_info in tools {
            let name = tool_info.name.clone();
            let adapter =
                super::mcp::McpToolAdapter::new(endpoint.clone(), tool_info, client.clone());
            self.register_with_source(
                adapter.into_dynamic(),
                ToolSource::Mcp {
                    endpoint: endpoint.clone(),
                },
            )?;
            registered_names.push(name);
        }

        tracing::info!(
            "Loaded {} tools from MCP server '{}'",
            registered_names.len(),
            endpoint,
        );

        Ok(registered_names)
    }

    /// 加载 MCP 服务器的工具 (存根 - 需要启用 `mcp` feature)
    /// Load MCP tools (Stub - requires `mcp` feature)
    #[cfg(not(feature = "mcp"))]
    pub async fn load_mcp_server(&mut self, endpoint: &str) -> AgentResult<Vec<String>> {
        self.mcp_endpoints.push(endpoint.to_string());
        // MCP feature not enabled - return empty
        Ok(vec![])
    }

    /// 卸载 MCP 服务器的工具
    /// Unload MCP server tools
    pub async fn unload_mcp_server(&mut self, endpoint: &str) -> AgentResult<Vec<String>> {
        self.mcp_endpoints.retain(|e| e != endpoint);

        // 移除该服务器的工具
        // Remove tools of this server
        let to_remove: Vec<String> = self
            .sources
            .iter()
            .filter_map(|(name, source)| {
                if let ToolSource::Mcp { endpoint: ep } = source
                    && ep == endpoint
                {
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
    /// Hot reload plugin (TODO: actual implementation)
    pub async fn hot_reload_plugin(&mut self, _path: &str) -> AgentResult<Vec<String>> {
        // TODO: 实际插件热加载实现
        // TODO: actual hot reload implementation
        Err(mofa_kernel::agent::error::AgentError::Other(
            "Plugin hot reloading is not yet implemented".to_string(),
        ))
    }

    /// 获取工具来源
    /// Get tool source
    pub fn get_source(&self, name: &str) -> Option<&ToolSource> {
        self.sources.get(name)
    }

    /// 按来源过滤工具
    /// Filter tools by source
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
            .map(|(_, tool)| ToolDescriptor::from_dyn_tool(tool.as_ref()))
            .collect()
    }

    /// 获取 MCP 端点列表
    /// Get list of MCP endpoints
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
    fn register(&mut self, tool: Arc<dyn DynTool>) -> AgentResult<()> {
        self.register_with_source(tool, ToolSource::Dynamic)
    }

    fn get(&self, name: &str) -> Option<Arc<dyn DynTool>> {
        self.tools.get(name).cloned()
    }

    fn unregister(&mut self, name: &str) -> AgentResult<bool> {
        self.sources.remove(name);
        Ok(self.tools.remove(name).is_some())
    }

    fn list(&self) -> Vec<ToolDescriptor> {
        self.tools
            .values()
            .map(|t| ToolDescriptor::from_dyn_tool(t.as_ref()))
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
// Tool Search
// ============================================================================

/// 工具搜索器
/// Tool searcher
pub struct ToolSearcher<'a> {
    registry: &'a ToolRegistry,
}

impl<'a> ToolSearcher<'a> {
    /// 创建搜索器
    /// Create searcher
    pub fn new(registry: &'a ToolRegistry) -> Self {
        Self { registry }
    }

    /// 按名称模糊搜索
    /// Fuzzy search by name
    pub fn search_by_name(&self, pattern: &str) -> Vec<ToolDescriptor> {
        let pattern_lower = pattern.to_lowercase();
        self.registry
            .tools
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&pattern_lower))
            .map(|(_, tool)| ToolDescriptor::from_dyn_tool(tool.as_ref()))
            .collect()
    }

    /// 按描述搜索
    /// Search by description
    pub fn search_by_description(&self, query: &str) -> Vec<ToolDescriptor> {
        let query_lower = query.to_lowercase();
        self.registry
            .tools
            .values()
            .filter(|tool| tool.description().to_lowercase().contains(&query_lower))
            .map(|tool| ToolDescriptor::from_dyn_tool(tool.as_ref()))
            .collect()
    }

    /// 按标签搜索
    /// Search by tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<ToolDescriptor> {
        self.registry
            .tools
            .values()
            .filter(|tool| {
                let metadata = tool.metadata();
                metadata.tags.iter().any(|t| t == tag)
            })
            .map(|tool| ToolDescriptor::from_dyn_tool(tool.as_ref()))
            .collect()
    }

    /// 搜索需要确认的工具
    /// Search for tools requiring confirmation
    pub fn search_dangerous(&self) -> Vec<ToolDescriptor> {
        self.registry
            .tools
            .values()
            .filter(|tool| tool.metadata().is_dangerous || tool.requires_confirmation())
            .map(|tool| ToolDescriptor::from_dyn_tool(tool.as_ref()))
            .collect()
    }
}
