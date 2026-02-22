//! Integration tests for agent registry behavior used by CLI workflows.

#![cfg(test)]

use async_trait::async_trait;
use mofa_kernel::agent::config::AgentConfig;
use mofa_runtime::agent::capabilities::{AgentCapabilities, AgentRequirements};
use mofa_runtime::agent::context::AgentContext;
use mofa_runtime::agent::core::MoFAAgent;
use mofa_runtime::agent::error::AgentResult;
use mofa_runtime::agent::types::{AgentInput, AgentOutput, AgentState};
use mofa_runtime::agent::{AgentFactory, AgentRegistry};
use std::sync::Arc;
use tokio::sync::RwLock;

struct TestAgent {
    id: String,
    name: String,
    state: AgentState,
    capabilities: AgentCapabilities,
}

impl TestAgent {
    fn new(id: &str, name: &str, capabilities: AgentCapabilities) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            state: AgentState::Created,
            capabilities,
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
        Ok(AgentOutput::text("ok"))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}

struct TestAgentFactory;

#[async_trait]
impl AgentFactory for TestAgentFactory {
    async fn create(&self, config: AgentConfig) -> AgentResult<Arc<RwLock<dyn MoFAAgent>>> {
        let capabilities = AgentCapabilities::builder().with_tag("factory").build();
        let agent = TestAgent::new(&config.id, &config.name, capabilities);
        Ok(Arc::new(RwLock::new(agent)))
    }

    fn type_id(&self) -> &str {
        "test-factory"
    }

    fn default_capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::builder().with_tag("factory").build()
    }
}

#[tokio::test]
async fn test_registry_register_and_query() {
    let registry = AgentRegistry::new();
    let caps = AgentCapabilities::builder()
        .with_tag("cli")
        .with_tag("integration")
        .build();
    let agent = Arc::new(RwLock::new(TestAgent::new("agent-1", "Agent One", caps)));

    registry.register(agent).await.unwrap();

    assert!(registry.contains("agent-1").await);
    assert_eq!(registry.count().await, 1);
    assert_eq!(registry.list().await.len(), 1);

    let tagged = registry.find_by_tag("cli").await;
    assert_eq!(tagged.len(), 1);
    assert_eq!(tagged[0].id, "agent-1");
}

#[tokio::test]
async fn test_registry_find_by_capabilities() {
    let registry = AgentRegistry::new();

    let caps_a = AgentCapabilities::builder().with_tag("planner").build();
    let caps_b = AgentCapabilities::builder().with_tag("writer").build();
    let agent_a = Arc::new(RwLock::new(TestAgent::new("agent-a", "Planner", caps_a)));
    let agent_b = Arc::new(RwLock::new(TestAgent::new("agent-b", "Writer", caps_b)));

    registry.register(agent_a).await.unwrap();
    registry.register(agent_b).await.unwrap();

    let requirements = AgentRequirements::builder().require_tag("writer").build();
    let matched = registry.find_by_capabilities(&requirements).await;
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].id, "agent-b");
}

#[tokio::test]
async fn test_registry_factory_create() {
    let registry = AgentRegistry::new();
    registry
        .register_factory(Arc::new(TestAgentFactory))
        .await
        .unwrap();

    let created = registry
        .create("test-factory", AgentConfig::new("factory-agent", "Factory Agent"))
        .await
        .unwrap();
    let guard = created.read().await;

    assert_eq!(guard.id(), "factory-agent");
    assert_eq!(guard.name(), "Factory Agent");
}

#[tokio::test]
async fn test_registry_concurrent_register() {
    let registry = Arc::new(AgentRegistry::new());
    let mut handles = Vec::new();

    for i in 0..5 {
        let reg = Arc::clone(&registry);
        handles.push(tokio::spawn(async move {
            let caps = AgentCapabilities::builder().with_tag("batch").build();
            let agent = Arc::new(RwLock::new(TestAgent::new(
                &format!("batch-{i}"),
                &format!("Batch Agent {i}"),
                caps,
            )));
            reg.register(agent).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(registry.count().await, 5);
}
