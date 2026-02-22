//! Agent 注册中心
//!
//! 提供 Agent 的注册、发现、工厂创建功能

use crate::agent::capabilities::{AgentCapabilities, AgentRequirements};
use crate::agent::context::AgentContext;
use crate::agent::core::MoFAAgent;
use crate::agent::error::{AgentError, AgentResult};
use crate::agent::traits::AgentMetadata;
use crate::agent::types::AgentState;
use mofa_kernel::agent::config::AgentConfig;
use mofa_kernel::agent::registry::AgentFactory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// Agent 注册条目
// ============================================================================

/// Agent 注册条目
struct AgentEntry {
    /// Agent 实例
    agent: Arc<RwLock<dyn MoFAAgent>>,
    /// 元数据
    metadata: AgentMetadata,
    /// 注册时间
    registered_at: u64,
}

// ============================================================================
// 能力索引
// ============================================================================

/// 能力索引
///
/// 用于快速查找具有特定能力的 Agent
struct CapabilityIndex {
    /// 标签索引: tag -> agent_ids
    by_tag: HashMap<String, Vec<String>>,
    /// 推理策略索引: strategy -> agent_ids
    by_strategy: HashMap<String, Vec<String>>,
}

impl CapabilityIndex {
    fn new() -> Self {
        Self {
            by_tag: HashMap::new(),
            by_strategy: HashMap::new(),
        }
    }

    /// 添加索引
    fn index(&mut self, agent_id: &str, capabilities: &AgentCapabilities) {
        // 索引标签
        for tag in &capabilities.tags {
            self.by_tag
                .entry(tag.clone())
                .or_default()
                .push(agent_id.to_string());
        }

        // 索引推理策略
        for strategy in &capabilities.reasoning_strategies {
            let strategy_name = format!("{:?}", strategy);
            self.by_strategy
                .entry(strategy_name)
                .or_default()
                .push(agent_id.to_string());
        }
    }

    /// 移除索引
    fn unindex(&mut self, agent_id: &str) {
        for ids in self.by_tag.values_mut() {
            ids.retain(|id| id != agent_id);
        }
        for ids in self.by_strategy.values_mut() {
            ids.retain(|id| id != agent_id);
        }
    }

    /// 按标签查找
    fn find_by_tag(&self, tag: &str) -> Vec<String> {
        self.by_tag.get(tag).cloned().unwrap_or_default()
    }

    /// 按多个标签查找 (交集)
    fn find_by_tags(&self, tags: &[String]) -> Vec<String> {
        if tags.is_empty() {
            return vec![];
        }

        let mut result: Option<Vec<String>> = None;
        for tag in tags {
            let ids = self.find_by_tag(tag);
            result = match result {
                None => Some(ids),
                Some(existing) => {
                    let intersection: Vec<String> =
                        existing.into_iter().filter(|id| ids.contains(id)).collect();
                    Some(intersection)
                }
            };
        }
        result.unwrap_or_default()
    }
}

// ============================================================================
// Agent 注册中心
// ============================================================================

/// Agent 注册中心
///
/// 提供 Agent 的注册、发现、工厂创建功能
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_runtime::agent::AgentRegistry;
/// use mofa_kernel::agent::config::AgentConfig;
///
/// let registry = AgentRegistry::new();
///
/// // 注册工厂
/// registry.register_factory(Arc::new(LLMAgentFactory)).await?;
///
/// // 通过工厂创建 Agent
/// let config = AgentConfig::new("agent-1", "My Agent", "llm");
/// let agent = registry.create("llm", config).await?;
///
/// // 注册 Agent
/// registry.register(agent).await?;
///
/// // 查找 Agent
/// let found = registry.get("agent-1").await;
/// ```
pub struct AgentRegistry {
    /// 已注册的 Agent
    agents: Arc<RwLock<HashMap<String, AgentEntry>>>,
    /// 能力索引
    capability_index: Arc<RwLock<CapabilityIndex>>,
    /// Agent 工厂
    factories: Arc<RwLock<HashMap<String, Arc<dyn AgentFactory>>>>,
}

impl AgentRegistry {
    /// 创建新的注册中心
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            capability_index: Arc::new(RwLock::new(CapabilityIndex::new())),
            factories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ========================================================================
    // Agent 管理
    // ========================================================================

    /// 注册 Agent
    pub async fn register(&self, agent: Arc<RwLock<dyn MoFAAgent>>) -> AgentResult<()> {
        let agent_guard = agent.read().await;
        let id = agent_guard.id().to_string();
        let name = agent_guard.name().to_string();
        let capabilities = agent_guard.capabilities().clone();
        let state = agent_guard.state();
        drop(agent_guard);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let metadata = AgentMetadata {
            id: id.clone(),
            name,
            description: None,
            version: None,
            capabilities: capabilities.clone(),
            state,
            registered_at: Some(now),
        };

        let entry = AgentEntry {
            agent,
            metadata,
            registered_at: now,
        };

        // 更新能力索引
        {
            let mut index = self.capability_index.write().await;
            index.index(&id, &capabilities);
        }

        // 注册 Agent
        {
            let mut agents = self.agents.write().await;
            agents.insert(id, entry);
        }

        Ok(())
    }

    /// 获取 Agent
    pub async fn get(&self, id: &str) -> Option<Arc<RwLock<dyn MoFAAgent>>> {
        let agents = self.agents.read().await;
        agents.get(id).map(|e| e.agent.clone())
    }

    /// 移除 Agent
    pub async fn unregister(&self, id: &str) -> AgentResult<bool> {
        // 更新能力索引
        {
            let mut index = self.capability_index.write().await;
            index.unindex(id);
        }

        // 移除 Agent
        let mut agents = self.agents.write().await;
        Ok(agents.remove(id).is_some())
    }

    /// 获取 Agent 元数据
    pub async fn get_metadata(&self, id: &str) -> Option<AgentMetadata> {
        let agents = self.agents.read().await;
        agents.get(id).map(|e| e.metadata.clone())
    }

    /// 列出所有 Agent
    pub async fn list(&self) -> Vec<AgentMetadata> {
        let agents = self.agents.read().await;
        agents.values().map(|e| e.metadata.clone()).collect()
    }

    /// 获取 Agent 数量
    pub async fn count(&self) -> usize {
        let agents = self.agents.read().await;
        agents.len()
    }

    /// 检查 Agent 是否存在
    pub async fn contains(&self, id: &str) -> bool {
        let agents = self.agents.read().await;
        agents.contains_key(id)
    }

    // ========================================================================
    // 能力查询
    // ========================================================================

    /// 按能力要求查找 Agent
    pub async fn find_by_capabilities(
        &self,
        requirements: &AgentRequirements,
    ) -> Vec<AgentMetadata> {
        let agents = self.agents.read().await;

        agents
            .values()
            .filter(|entry| requirements.matches(&entry.metadata.capabilities))
            .map(|entry| entry.metadata.clone())
            .collect()
    }

    /// 按标签查找 Agent
    pub async fn find_by_tag(&self, tag: &str) -> Vec<AgentMetadata> {
        let index = self.capability_index.read().await;
        let ids = index.find_by_tag(tag);
        drop(index);

        let agents = self.agents.read().await;
        ids.iter()
            .filter_map(|id| agents.get(id).map(|e| e.metadata.clone()))
            .collect()
    }

    /// 按多个标签查找 Agent (交集)
    pub async fn find_by_tags(&self, tags: &[String]) -> Vec<AgentMetadata> {
        let index = self.capability_index.read().await;
        let ids = index.find_by_tags(tags);
        drop(index);

        let agents = self.agents.read().await;
        ids.iter()
            .filter_map(|id| agents.get(id).map(|e| e.metadata.clone()))
            .collect()
    }

    /// 按状态查找 Agent
    pub async fn find_by_state(&self, state: AgentState) -> Vec<AgentMetadata> {
        let agents = self.agents.read().await;

        agents
            .values()
            .filter(|entry| entry.metadata.state == state)
            .map(|entry| entry.metadata.clone())
            .collect()
    }

    // ========================================================================
    // 工厂管理
    // ========================================================================

    /// 注册 Agent 工厂
    pub async fn register_factory(&self, factory: Arc<dyn AgentFactory>) -> AgentResult<()> {
        let type_id = factory.type_id().to_string();
        let mut factories = self.factories.write().await;
        factories.insert(type_id, factory);
        Ok(())
    }

    /// 获取 Agent 工厂
    pub async fn get_factory(&self, type_id: &str) -> Option<Arc<dyn AgentFactory>> {
        let factories = self.factories.read().await;
        factories.get(type_id).cloned()
    }

    /// 移除 Agent 工厂
    pub async fn unregister_factory(&self, type_id: &str) -> AgentResult<bool> {
        let mut factories = self.factories.write().await;
        Ok(factories.remove(type_id).is_some())
    }

    /// 列出所有工厂类型
    pub async fn list_factory_types(&self) -> Vec<String> {
        let factories = self.factories.read().await;
        factories.keys().cloned().collect()
    }

    /// 通过工厂创建 Agent
    pub async fn create(
        &self,
        type_id: &str,
        config: AgentConfig,
    ) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>> {
        let factory = self
            .get_factory(type_id)
            .await
            .ok_or_else(|| AgentError::NotFound(format!("Factory not found: {}", type_id)))?;

        factory.validate_config(&config)?;
        factory.create(config).await
    }

    /// 创建并注册 Agent
    pub async fn create_and_register(
        &self,
        type_id: &str,
        config: AgentConfig,
    ) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>> {
        let agent = self.create(type_id, config).await?;
        self.register(agent.clone()).await?;
        Ok(agent)
    }

    // ========================================================================
    // 批量操作
    // ========================================================================

    /// 初始化所有 Agent
    pub async fn initialize_all(&self, ctx: &AgentContext) -> AgentResult<Vec<String>> {
        let agents = self.agents.read().await;
        let mut initialized = Vec::new();

        for (id, entry) in agents.iter() {
            let mut agent = entry.agent.write().await;
            if agent.state() == AgentState::Created {
                agent.initialize(ctx).await?;
                initialized.push(id.clone());
            }
        }

        Ok(initialized)
    }

    /// 关闭所有 Agent
    pub async fn shutdown_all(&self) -> AgentResult<Vec<String>> {
        let agents = self.agents.read().await;
        let mut shutdown = Vec::new();

        for (id, entry) in agents.iter() {
            let mut agent = entry.agent.write().await;
            let state = agent.state();
            if state != AgentState::Shutdown && state != AgentState::Failed {
                agent.shutdown().await?;
                shutdown.push(id.clone());
            }
        }

        Ok(shutdown)
    }

    /// 清空所有 Agent
    pub async fn clear(&self) -> AgentResult<usize> {
        // 先关闭所有 Agent
        self.shutdown_all().await?;

        // 清空索引
        {
            let mut index = self.capability_index.write().await;
            *index = CapabilityIndex::new();
        }

        // 清空 Agent
        let mut agents = self.agents.write().await;
        let count = agents.len();
        agents.clear();

        Ok(count)
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 注册中心统计
// ============================================================================

/// 注册中心统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryStats {
    /// 总 Agent 数
    pub total_agents: usize,
    /// 各状态 Agent 数
    pub by_state: HashMap<String, usize>,
    /// 各标签 Agent 数
    pub by_tag: HashMap<String, usize>,
    /// 工厂类型数
    pub factory_count: usize,
    /// 最早注册时间戳 (Unix milliseconds)
    pub earliest_registration: Option<u64>,
    /// 最新注册时间戳 (Unix milliseconds)
    pub latest_registration: Option<u64>,
}

impl AgentRegistry {
    /// 获取统计信息
    pub async fn stats(&self) -> RegistryStats {
        let agents = self.agents.read().await;
        let factories = self.factories.read().await;

        let mut by_state: HashMap<String, usize> = HashMap::new();
        let mut by_tag: HashMap<String, usize> = HashMap::new();
        let mut earliest_registration: Option<u64> = None;
        let mut latest_registration: Option<u64> = None;

        for entry in agents.values() {
            // 统计状态
            let state_name = format!("{:?}", entry.metadata.state);
            *by_state.entry(state_name).or_insert(0) += 1;

            // 统计标签
            for tag in &entry.metadata.capabilities.tags {
                *by_tag.entry(tag.clone()).or_insert(0) += 1;
            }

            // 统计注册时间
            if let Some(ts) = entry.registered_at.checked_sub(0) {
                earliest_registration = Some(earliest_registration.map_or(ts, |e| e.min(ts)));
                latest_registration = Some(latest_registration.map_or(ts, |l| l.max(ts)));
            }
        }

        RegistryStats {
            total_agents: agents.len(),
            by_state,
            by_tag,
            factory_count: factories.len(),
            earliest_registration,
            latest_registration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::capabilities::AgentCapabilities;
    use crate::agent::context::AgentContext;
    use crate::agent::core::MoFAAgent;
    use crate::agent::error::AgentResult;
    use crate::agent::types::{AgentInput, AgentOutput, AgentState};
    use async_trait::async_trait;

    // 测试用的简单 Agent (内联实现，不依赖 BaseAgent)
    struct TestAgent {
        id: String,
        name: String,
        capabilities: AgentCapabilities,
        state: AgentState,
    }

    impl TestAgent {
        fn new(id: &str, name: &str) -> Self {
            Self {
                id: id.to_string(),
                name: name.to_string(),
                capabilities: AgentCapabilities::default(),
                state: AgentState::Created,
            }
        }
    }

    #[async_trait]
    impl MoFAAgent for TestAgent {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn capabilities(&self) -> &AgentCapabilities {
            &self.capabilities
        }

        fn state(&self) -> AgentState {
            self.state.clone()
        }

        async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
            self.state = AgentState::Ready;
            Ok(())
        }

        async fn execute(
            &mut self,
            _input: AgentInput,
            _ctx: &AgentContext,
        ) -> AgentResult<AgentOutput> {
            Ok(AgentOutput::text("test output"))
        }

        async fn shutdown(&mut self) -> AgentResult<()> {
            self.state = AgentState::Shutdown;
            Ok(())
        }
    }

    // 测试用的工厂
    struct TestAgentFactory;

    #[async_trait]
    impl AgentFactory for TestAgentFactory {
        async fn create(&self, config: AgentConfig) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>> {
            let agent = TestAgent::new(&config.id, &config.name);
            Ok(Arc::new(RwLock::new(agent)))
        }

        fn type_id(&self) -> &str {
            "test"
        }

        fn default_capabilities(&self) -> AgentCapabilities {
            AgentCapabilities::builder().with_tag("test").build()
        }
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(RwLock::new(TestAgent::new("agent-1", "Test Agent")));

        registry.register(agent).await.unwrap();

        let found = registry.get("agent-1").await;
        assert!(found.is_some());

        let not_found = registry.get("nonexistent").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_factory_create() {
        let registry = AgentRegistry::new();
        registry
            .register_factory(Arc::new(TestAgentFactory))
            .await
            .unwrap();

        let config = AgentConfig::new("agent-2", "Created Agent");
        let agent = registry.create("test", config).await.unwrap();

        let agent_guard = agent.read().await;
        assert_eq!(agent_guard.id(), "agent-2");
        assert_eq!(agent_guard.name(), "Created Agent");
    }

    #[tokio::test]
    async fn test_find_by_tag() {
        let registry = AgentRegistry::new();

        // 创建带有标签的 Agent
        let mut agent1 = TestAgent::new("agent-1", "Agent 1");
        agent1.capabilities = AgentCapabilities::builder()
            .with_tag("llm")
            .with_tag("chat")
            .build();

        let mut agent2 = TestAgent::new("agent-2", "Agent 2");
        agent2.capabilities = AgentCapabilities::builder()
            .with_tag("react")
            .with_tag("chat")
            .build();

        registry
            .register(Arc::new(RwLock::new(agent1)))
            .await
            .unwrap();
        registry
            .register(Arc::new(RwLock::new(agent2)))
            .await
            .unwrap();

        // 按标签查找
        let chat_agents = registry.find_by_tag("chat").await;
        assert_eq!(chat_agents.len(), 2);

        let llm_agents = registry.find_by_tag("llm").await;
        assert_eq!(llm_agents.len(), 1);
    }

    #[tokio::test]
    async fn test_unregister() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(RwLock::new(TestAgent::new("agent-1", "Test Agent")));

        registry.register(agent).await.unwrap();
        assert!(registry.contains("agent-1").await);

        registry.unregister("agent-1").await.unwrap();
        assert!(!registry.contains("agent-1").await);
    }

    #[tokio::test]
    async fn test_stats() {
        let registry = AgentRegistry::new();
        registry
            .register_factory(Arc::new(TestAgentFactory))
            .await
            .unwrap();

        let agent = Arc::new(RwLock::new(TestAgent::new("agent-1", "Test")));
        registry.register(agent).await.unwrap();

        let stats = registry.stats().await;
        assert_eq!(stats.total_agents, 1);
        assert_eq!(stats.factory_count, 1);
    }
}
