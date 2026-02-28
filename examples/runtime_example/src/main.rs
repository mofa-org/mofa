use async_trait::async_trait;
use mofa_sdk::kernel::{
    AgentCapabilities, AgentContext, AgentEvent, AgentInput, AgentOutput,
    AgentResult, AgentState, MoFAAgent,
};
use mofa_sdk::runtime::{AgentRunner, run_agents, AgentBuilder, SimpleRuntime};
use tracing::info;
// ============================================================================
// SimpleRuntimeAgent - 新版 MoFAAgent 实现
// SimpleRuntimeAgent - New MoFAAgent Implementation
// ============================================================================

/// 简单运行时智能体实现
/// Simple runtime agent implementation
///
/// 展示如何使用新版 MoFAAgent 接口实现智能体
/// Demonstrates how to implement an agent using the new MoFAAgent interface
struct SimpleRuntimeAgent {
    id: String,
    name: String,
    capabilities: AgentCapabilities,
    state: AgentState,
    event_count: usize,
}

impl SimpleRuntimeAgent {
    pub fn new(agent_id: &str, name: &str) -> Self {
        Self {
            id: agent_id.to_string(),
            name: name.to_string(),
            capabilities: AgentCapabilities::builder()
                .tags(vec!["echo".to_string(), "event_handler".to_string()])
                .build(),
            state: AgentState::Created,
            event_count: 0,
        }
    }
}

#[async_trait]
impl MoFAAgent for SimpleRuntimeAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &AgentCapabilities {
        &self.capabilities
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        info!("Agent {} initialized", self.id);
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        self.state = AgentState::Executing;
        self.event_count += 1;

        let text = input.to_text();
        info!(
            "Agent {} processing input '{}' (total events: {})",
            self.id, text, self.event_count
        );

        self.state = AgentState::Ready;
        Ok(AgentOutput::text(format!("Processed: {}", text)))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        info!("Agent {} shutdown", self.id);
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }
}

// ============================================================================
// Main Function
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("=== MoFA Runtime Example (New API) ===\n");

    // ========================================================================
    // Example 1: 使用 run_agents 批量执行
    // Example 1: Using run_agents for batch execution
    // ========================================================================
    info!("Example 1: Using run_agents() for batch execution");
    let batch_agent = SimpleRuntimeAgent::new("agent_batch", "BatchAgent");
    let batch_inputs = vec![
        AgentInput::text("task-1"),
        AgentInput::text("task-2"),
        AgentInput::text("task-3"),
    ];
    let batch_outputs = run_agents(batch_agent, batch_inputs).await?;
    for output in batch_outputs {
        info!("Batch output: {}", output.to_text());
    }
    info!("\n");

    // ========================================================================
    // Example 2: 直接使用 AgentBuilder 构建和使用 SimpleAgentRuntime
    // Example 2: Building and using SimpleAgentRuntime via AgentBuilder directly
    // ========================================================================
    info!("Example 2: Using AgentBuilder directly");

    let agent1 = SimpleRuntimeAgent::new("agent1", "AgentOne");

    let mut runtime = AgentBuilder::new("agent1", "AgentOne")
        .with_capability("echo")
        .with_capability("event_handler")
        .with_agent(agent1)
        .await?;

    // 启动智能体
    // Start the agent
    runtime.start().await?;

    // 处理一些自定义事件
    // Handle some custom events
    info!("Sending events to agent1...");
    runtime
        .handle_event(AgentEvent::Custom("test_event".to_string(), vec![]))
        .await?;
    runtime
        .handle_event(AgentEvent::Custom(
            "greeting".to_string(),
            b"Hello from Runtime".to_vec(),
        ))
        .await?;

    // 停止智能体
    // Stop the agent
    runtime.stop().await?;

    info!("\n");

    // ========================================================================
    // Example 3: 使用 SimpleRuntime 管理多个智能体
    // Example 3: Using SimpleRuntime to manage multiple agents
    // ========================================================================
    info!("Example 3: Using SimpleRuntime to manage multiple agents");

    let runtime = SimpleRuntime::new();

    // 使用 AgentBuilder 构建 AgentMetadata
    // Use AgentBuilder to construct AgentMetadata
    let metadata1 = AgentBuilder::new("agent_master", "MasterAgent")
        .with_capability("master")
        .build_metadata();

    let metadata2 = AgentBuilder::new("agent_worker", "WorkerAgent")
        .with_capability("worker")
        .build_metadata();

    // 构建配置
    // Construct configurations
    let config1 = AgentBuilder::new("agent_master", "MasterAgent").build_config();
    let config2 = AgentBuilder::new("agent_worker", "WorkerAgent").build_config();

    let mut rx1 = runtime
        .register_agent(metadata1, config1, "master")
        .await?;
    let mut rx2 = runtime
        .register_agent(metadata2, config2, "worker")
        .await?;

    // 创建并运行智能体实例
    // Create and run agent instances
    let agent_master = SimpleRuntimeAgent::new("agent_master", "MasterAgent");
    let agent_worker = SimpleRuntimeAgent::new("agent_worker", "WorkerAgent");

    // Master agent 事件循环
    // Master agent event loop
    tokio::spawn(async move {
        let mut runner = AgentRunner::new(agent_master).await.unwrap();

        info!("Starting master agent event loop...");

        while let Some(event) = rx1.recv().await {
            if matches!(event, AgentEvent::Shutdown) {
                info!("Master agent received shutdown event");
                break;
            }

            // 将事件转换为输入
            // Convert event into input
            let input = match event {
                AgentEvent::TaskReceived(task) => AgentInput::text(task.content),
                AgentEvent::Custom(data, _) => AgentInput::text(data),
                _ => AgentInput::text(format!("{:?}", event)),
            };

            let _ = runner.execute(input).await;
        }

        let _ = runner.shutdown().await;
    });

    // Worker agent 事件循环
    // Worker agent event loop
    tokio::spawn(async move {
        let mut runner = AgentRunner::new(agent_worker).await.unwrap();

        info!("Starting worker agent event loop...");

        while let Some(event) = rx2.recv().await {
            if matches!(event, AgentEvent::Shutdown) {
                info!("Worker agent received shutdown event");
                break;
            }

            let input = match event {
                AgentEvent::TaskReceived(task) => AgentInput::text(task.content),
                AgentEvent::Custom(data, _) => AgentInput::text(data),
                _ => AgentInput::text(format!("{:?}", event)),
            };

            let _ = runner.execute(input).await;
        }

        let _ = runner.shutdown().await;
    });

    // 创建消息总线并发送消息
    // Create message bus and send messages
    let message_bus = runtime.message_bus();

    // 订阅主题
    // Subscribe to topics
    runtime
        .subscribe_topic("agent_master", "commands")
        .await?;
    runtime
        .subscribe_topic("agent_worker", "commands")
        .await?;

    // 发布消息到主题
    // Publish message to topic
    info!("Publishing command to 'commands' topic...");
    message_bus
        .publish(
            "commands",
            AgentEvent::Custom("start_work".to_string(), vec![]),
        )
        .await?;

    // 发送点对点消息
    // Send peer-to-peer message
    info!("Sending direct message from master to worker...");
    message_bus
        .send_to(
            "agent_worker",
            AgentEvent::Custom(
                "task_assignment".to_string(),
                b"Process data".to_vec(),
            ),
        )
        .await?;

    // 让程序运行一段时间
    // Let the program run for a while
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 停止所有智能体
    // Stop all agents
    runtime.stop_all().await?;

    info!("\nAll examples completed!");

    Ok(())
}
