//! Agent Runner - 统一的 Agent 运行器
//!
//! 本模块提供通用的 Agent 运行器，可以运行任何实现了 MoFAAgent 的 Agent。
//!
//! # 设计原则
//!
//! - 通用性：支持任何实现 MoFAAgent 的 Agent
//! - 可组合性：通过 trait bounds 支持扩展功能
//! - 简洁性：提供简单易用的 API

use mofa_kernel::agent::{
    core::{MoFAAgent, AgentLifecycle, AgentMessaging},
    prelude::*,
};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// AgentRunner - 基础运行器
// ============================================================================

/// 通用 Agent 运行器
///
/// 可以运行任何实现了 MoFAAgent 的 Agent。
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_runtime::runner::AgentRunner;
/// use mofa_kernel::agent::prelude::*;
///
/// struct MyAgent { ... }
///
/// #[async_trait]
/// impl MoFAAgent for MyAgent { ... }
///
/// #[tokio::main]
/// async fn main() -> AgentResult<()> {
///     let agent = MyAgent::new();
///     let mut runner = AgentRunner::new(agent).await?;
///
///     let input = AgentInput::text("Hello");
///     let output = runner.run(input).await?;
///
///     runner.shutdown().await?;
///     Ok(())
/// }
/// ```
pub struct AgentRunner<T: MoFAAgent> {
    agent: T,
    context: AgentContext,
    state: Arc<RwLock<RunnerState>>,
}

/// 运行器状态
#[derive(Debug, Clone, PartialEq)]
enum RunnerState {
    Created,
    Initialized,
    Running,
    Stopped,
    Error(String),
}

impl<T: MoFAAgent> AgentRunner<T> {
    /// 创建新的运行器
    ///
    /// 初始化 Agent 并创建执行上下文。
    pub async fn new(mut agent: T) -> AgentResult<Self> {
        let execution_id = format!("{}-exec", agent.id());
        let context = AgentContext::new(execution_id);

        // 初始化 Agent
        agent.initialize(&context).await?;

        let state = Arc::new(RwLock::new(RunnerState::Initialized));

        Ok(Self {
            agent,
            context,
            state,
        })
    }

    /// 使用自定义上下文创建运行器
    pub async fn with_context(mut agent: T, context: AgentContext) -> AgentResult<Self> {
        // 初始化 Agent
        agent.initialize(&context).await?;

        let state = Arc::new(RwLock::new(RunnerState::Initialized));

        Ok(Self {
            agent,
            context,
            state,
        })
    }

    /// 获取 Agent 引用
    pub fn agent(&self) -> &T {
        &self.agent
    }

    /// 获取可变 Agent 引用
    pub fn agent_mut(&mut self) -> &mut T {
        &mut self.agent
    }

    /// 获取上下文引用
    pub fn context(&self) -> &AgentContext {
        &self.context
    }

    /// 获取运行器状态
    pub async fn state(&self) -> RunnerState {
        self.state.read().await.clone()
    }

    /// 运行 Agent - 执行单次任务
    ///
    /// # 参数
    ///
    /// - `input`: Agent 输入
    ///
    /// # 返回
    ///
    /// 返回 Agent 输出
    pub async fn run(&mut self, input: AgentInput) -> AgentResult<AgentOutput> {
        // 检查状态
        {
            let state = self.state.read().await;
            if *state != RunnerState::Initialized && *state != RunnerState::Running {
                return Err(AgentError::ExecutionFailed(format!(
                    "Invalid runner state: {:?}, expected Initialized or Running",
                    state
                )));
            }
        }

        // 更新状态为运行中
        *self.state.write().await = RunnerState::Running;

        // 执行 Agent
        let result = self.agent.execute(input, &self.context).await;

        // 恢复状态
        if result.is_ok() {
            *self.state.write().await = RunnerState::Initialized;
        } else {
            *self.state.write().await = RunnerState::Error("Execution failed".to_string());
        }

        result
    }

    /// 运行多次
    ///
    /// # 参数
    ///
    /// - `inputs`: 输入列表
    ///
    /// # 返回
    ///
    /// 返回输出列表
    pub async fn run_many(&mut self, inputs: Vec<AgentInput>) -> AgentResult<Vec<AgentOutput>> {
        let mut outputs = Vec::with_capacity(inputs.len());

        for input in inputs {
            let output = self.run(input).await?;
            outputs.push(output);
        }

        Ok(outputs)
    }

    /// 关闭运行器
    ///
    /// 优雅关闭 Agent 并释放资源。
    pub async fn shutdown(mut self) -> AgentResult<()> {
        *self.state.write().await = RunnerState::Stopped;
        self.agent.shutdown().await
    }

    /// 获取 Agent 状态
    pub fn agent_state(&self) -> AgentState {
        self.agent.state()
    }

    /// 检查 Agent 是否就绪
    pub fn is_ready(&self) -> bool {
        self.agent.state() == AgentState::Ready
    }

    /// 检查运行器是否活动
    pub async fn is_active(&self) -> bool {
        let state = self.state.read().await;
        matches!(*state, RunnerState::Initialized | RunnerState::Running)
    }
}

// ============================================================================
// AgentRunner - 扩展功能（生命周期）
// ============================================================================

impl<T: MoFAAgent + AgentLifecycle> AgentRunner<T> {
    /// 暂停 Agent
    pub async fn pause(&mut self) -> AgentResult<()> {
        self.agent.pause().await?;
        *self.state.write().await = RunnerState::Initialized;
        Ok(())
    }

    /// 恢复 Agent
    pub async fn resume(&mut self) -> AgentResult<()> {
        self.agent.resume().await?;
        *self.state.write().await = RunnerState::Initialized;
        Ok(())
    }

    /// 中断 Agent
    pub async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
        self.agent.interrupt().await
    }
}

// ============================================================================
// AgentRunner - 扩展功能（消息处理）
// ============================================================================

impl<T: MoFAAgent + AgentMessaging> AgentRunner<T> {
    /// 运行事件循环
    ///
    /// 持续处理来自上下文的事件，直到收到 Shutdown 事件。
    pub async fn run_event_loop(&mut self) -> AgentResult<()> {
        *self.state.write().await = RunnerState::Running;

        loop {
            // 检查是否应该停止
            {
                let state = self.state.read().await;
                if *state == RunnerState::Stopped {
                    break;
                }
            }

            // 获取下一个事件（使用非阻塞方式）
            // 注意：这里需要 AgentContext 提供 next_event 方法
            // 如果没有，我们可以使用其他方式来处理事件

            // 简单实现：检查状态并休眠
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // 这里可以添加事件处理逻辑
            // if let Some(event) = self.context.next_event().await {
            //     self.agent.handle_event(event).await?;
            // }
        }

        *self.state.write().await = RunnerState::Initialized;
        Ok(())
    }

    /// 处理单个消息
    pub async fn handle_message(&mut self, msg: AgentMessage) -> AgentResult<AgentMessage> {
        self.agent.handle_message(msg).await
    }

    /// 处理单个事件
    pub async fn handle_event(&mut self, event: AgentEvent) -> AgentResult<()> {
        self.agent.handle_event(event).await
    }
}

// ============================================================================
// 便利函数
// ============================================================================

/// 创建并运行 Agent（一次性执行）
///
/// # 示例
///
/// ```rust,ignore
/// let agent = MyAgent::new();
/// let input = AgentInput::text("Hello");
/// let output = run_agent_once(agent, input).await?;
/// ```
pub async fn run_agent_once<T: MoFAAgent>(
    agent: T,
    input: AgentInput,
) -> AgentResult<AgentOutput> {
    let mut runner = AgentRunner::new(agent).await?;
    let output = runner.run(input).await?;
    runner.shutdown().await?;
    Ok(output)
}

/// 创建并运行 Agent（多次执行）
///
/// # 示例
///
/// ```rust,ignore
/// let agent = MyAgent::new();
/// let inputs = vec![
///     AgentInput::text("Hello"),
///     AgentInput::text("World"),
/// ];
/// let outputs = run_agent_many(agent, inputs).await?;
/// ```
pub async fn run_agent_many<T: MoFAAgent>(
    agent: T,
    inputs: Vec<AgentInput>,
) -> AgentResult<Vec<AgentOutput>> {
    let mut runner = AgentRunner::new(agent).await?;
    let outputs = runner.run_many(inputs).await?;
    runner.shutdown().await?;
    Ok(outputs)
}


#[cfg(test)]
mod tests {
    use super::*;

    // Mock Agent for testing
    struct MockAgent {
        id: String,
        name: String,
        capabilities: AgentCapabilities,
        state: AgentState,
    }

    impl MockAgent {
        fn new() -> Self {
            Self {
                id: "mock-agent".to_string(),
                name: "Mock Agent".to_string(),
                capabilities: AgentCapabilities::default(),
                state: AgentState::Created,
            }
        }
    }

    #[async_trait::async_trait]
    impl MoFAAgent for MockAgent {
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
            Ok(())
        }

        async fn execute(
            &mut self,
            input: AgentInput,
            _ctx: &AgentContext,
        ) -> AgentResult<AgentOutput> {
            match input {
                AgentInput::Text(text) => {
                    let response = format!("Echo: {}", text);
                    Ok(AgentOutput::text(response))
                }
                _ => Ok(AgentOutput::text("Received")),
            }
        }

        async fn shutdown(&mut self) -> AgentResult<()> {
            self.state = AgentState::Shutdown;
            Ok(())
        }

        fn state(&self) -> AgentState {
            self.state.clone()
        }
    }

    #[tokio::test]
    async fn test_agent_runner() {
        let agent = MockAgent::new();
        let mut runner = AgentRunner::new(agent).await.unwrap();

        assert!(runner.is_ready());
        assert!(runner.is_active().await);

        let input = AgentInput::text("Hello");
        let output = runner.run(input).await.unwrap();

        assert_eq!(output.content.to_text(), "Echo: Hello");

        runner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_run_agent_once() {
        let agent = MockAgent::new();
        let input = AgentInput::text("Test");
        let output = run_agent_once(agent, input).await.unwrap();

        assert_eq!(output.content.to_text(), "Echo: Test");
    }

    #[tokio::test]
    async fn test_run_agent_many() {
        let agent = MockAgent::new();
        let inputs = vec![
            AgentInput::text("Hello"),
            AgentInput::text("World"),
        ];
        let outputs = run_agent_many(agent, inputs).await.unwrap();

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].content.to_text(), "Echo: Hello");
        assert_eq!(outputs[1].content.to_text(), "Echo: World");
    }
}
