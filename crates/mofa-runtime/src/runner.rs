//! 统一 Agent 运行器
//! Unified Agent Runner
//!
//! 提供统一的 Agent 执行接口，可以运行任何实现了 `MoFAAgent` 的 Agent。
//! Provides a unified Agent execution interface to run any Agent implementing `MoFAAgent`.
//!
//! # 架构
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                   AgentRunner<T: MoFAAgent>                         │
//! │  ┌─────────────────────────────────────────────────────────────┐    │
//! │  │  状态管理                                                    │   │
//! │  │  Status Management                                          │   │
//! │  │  • RunnerState: Initializing, Running, Paused, Stopping     │   │
//! │  └─────────────────────────────────────────────────────────────┘   │
//! │  ┌─────────────────────────────────────────────────────────────┐   │
//! │  │  执行模式                                                    │   │
//! │  │  Execution Mode                                             │   │
//! │  │  • Single: 单次执行                                          │   │
//! │  │  • Single: Single execution                                 │   │
//! │  │  • EventLoop: 事件循环（支持 AgentMessaging）                │   │
//! │  │  • EventLoop: Event loop (supports AgentMessaging)          │   │
//! │  │  • Stream: 流式执行                                          │   │
//! │  │  • Stream: Stream execution                                 │   │
//! │  └─────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 示例
//! # Example
//!
//! ## 基本使用
//! ## Basic usage
//!
//! ```rust,ignore
//! use mofa_runtime::runner::AgentRunner;
//! use mofa_runtime::agent::MoFAAgent;
//!
//! #[tokio::main]
//! async fn main() -> AgentResult<()> {
//!     let agent = MyAgent::new();
//!     let mut runner = AgentRunner::new(agent).await?;
//!
//!     // 执行任务
//!     // Execute task
//!     let input = AgentInput::text("Hello, Agent!");
//!     let output = runner.execute(input).await?;
//!
//!     // 关闭
//!     // Shutdown
//!     runner.shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## 事件循环模式
//! ## Event loop mode
//!
//! ```rust,ignore
//! use mofa_runtime::runner::AgentRunner;
//! use mofa_runtime::agent::{MoFAAgent, AgentMessaging};
//!
//! struct MyEventAgent { }
//!
//! #[async_trait]
//! impl MoFAAgent for MyEventAgent { /* ... */ }
//!
//! #[async_trait]
//! impl AgentMessaging for MyEventAgent { /* ... */ }
//!
//! #[tokio::main]
//! async fn main() -> AgentResult<()> {
//!     let agent = MyEventAgent::new();
//!     let mut runner = AgentRunner::new(agent).await?;
//!
//!     // 运行事件循环
//!     // Run event loop
//!     runner.run_event_loop().await?;
//!
//!     Ok(())
//! }
//! ```

use crate::agent::capabilities::AgentCapabilities;
use crate::agent::context::{AgentContext, AgentEvent};
use crate::agent::core::{AgentLifecycle, AgentMessage, AgentMessaging, MoFAAgent};
use crate::agent::error::{AgentError, AgentResult};
use crate::agent::types::{AgentInput, AgentOutput, AgentState, InterruptResult};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 运行器状态
/// Runner state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerState {
    /// 已创建
    /// Created
    Created,
    /// 初始化中
    /// Initializing
    Initializing,
    /// 运行中
    /// Running
    Running,
    /// 暂停
    /// Paused
    Paused,
    /// 停止中
    /// Stopping
    Stopping,
    /// 已停止
    /// Stopped
    Stopped,
    /// 错误
    /// Error
    Error,
}

/// 运行器统计信息
/// Runner statistics
#[derive(Debug, Clone, Default)]
pub struct RunnerStats {
    /// 总执行次数
    /// Total execution count
    pub total_executions: u64,
    /// 成功次数
    /// Success count
    pub successful_executions: u64,
    /// 失败次数
    /// Failure count
    pub failed_executions: u64,
    /// 平均执行时间（毫秒）
    /// Average execution time (ms)
    pub avg_execution_time_ms: f64,
    /// 最后执行时间
    /// Last execution time
    pub last_execution_time_ms: Option<u64>,
}

/// 统一 Agent 运行器
/// Unified Agent Runner
///
/// 可以运行任何实现了 `MoFAAgent` 的 Agent。
/// Can run any Agent that implements `MoFAAgent`.
pub struct AgentRunner<T: MoFAAgent> {
    /// Agent 实例
    /// Agent instance
    agent: T,
    /// 执行上下文
    /// Execution context
    context: AgentContext,
    /// 运行器状态
    /// Runner state
    state: Arc<RwLock<RunnerState>>,
    /// 统计信息
    /// Statistics
    stats: Arc<RwLock<RunnerStats>>,
}

impl<T: MoFAAgent> AgentRunner<T> {
    /// 创建新的运行器
    /// Create a new runner
    ///
    /// 此方法会初始化 Agent。
    /// This method will initialize the Agent.
    pub async fn new(mut agent: T) -> AgentResult<Self> {
        let context = AgentContext::new(agent.id().to_string());

        // 初始化 Agent
        // Initialize Agent
        agent
            .initialize(&context)
            .await
            .map_err(|e| AgentError::InitializationFailed(e.to_string()))?;

        Ok(Self {
            agent,
            context,
            state: Arc::new(RwLock::new(RunnerState::Created)),
            stats: Arc::new(RwLock::new(RunnerStats::default())),
        })
    }

    /// 使用自定义上下文创建运行器
    /// Create runner with custom context
    pub async fn with_context(mut agent: T, context: AgentContext) -> AgentResult<Self> {
        agent
            .initialize(&context)
            .await
            .map_err(|e| AgentError::InitializationFailed(e.to_string()))?;

        Ok(Self {
            agent,
            context,
            state: Arc::new(RwLock::new(RunnerState::Created)),
            stats: Arc::new(RwLock::new(RunnerStats::default())),
        })
    }

    /// 获取 Agent 引用
    /// Get Agent reference
    pub fn agent(&self) -> &T {
        &self.agent
    }

    /// 获取 Agent 可变引用
    /// Get mutable Agent reference
    pub fn agent_mut(&mut self) -> &mut T {
        &mut self.agent
    }

    /// 获取执行上下文
    /// Get execution context
    pub fn context(&self) -> &AgentContext {
        &self.context
    }

    /// 获取运行器状态
    /// Get runner state
    pub async fn state(&self) -> RunnerState {
        *self.state.read().await
    }

    /// 获取统计信息
    /// Get statistics
    pub async fn stats(&self) -> RunnerStats {
        self.stats.read().await.clone()
    }

    /// 检查是否正在运行
    /// Check if running
    pub async fn is_running(&self) -> bool {
        matches!(
            *self.state.read().await,
            RunnerState::Running | RunnerState::Paused
        )
    }

    /// 执行单个任务
    /// Execute a single task
    ///
    /// # 参数
    /// # Parameters
    ///
    /// - `input`: 输入数据
    /// - `input`: Input data
    ///
    /// # 返回
    /// # Returns
    ///
    /// 返回 Agent 的输出。
    /// Returns the Agent's output.
    pub async fn execute(&mut self, input: AgentInput) -> AgentResult<AgentOutput> {
        // 检查状态
        // Check state
        let current_state = self.state().await;
        if !matches!(
            current_state,
            RunnerState::Running | RunnerState::Created | RunnerState::Stopped
        ) {
            return Err(AgentError::ValidationFailed(format!(
                "Cannot execute in state: {:?}",
                current_state
            )));
        }

        // 更新状态为运行中
        // Update state to Running
        *self.state.write().await = RunnerState::Running;

        let start = std::time::Instant::now();

        // 执行 Agent
        // Execute Agent
        let result = self.agent.execute(input, &self.context).await;

        let duration = start.elapsed().as_millis() as u64;

        // 更新统计信息
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_executions += 1;
        stats.last_execution_time_ms = Some(duration);

        match &result {
            Ok(_) => {
                stats.successful_executions += 1;
            }
            Err(_) => {
                stats.failed_executions += 1;
            }
        }

        // 更新平均执行时间
        // Update average execution time
        let n = stats.total_executions as f64;
        stats.avg_execution_time_ms =
            (stats.avg_execution_time_ms * (n - 1.0) + duration as f64) / n;

        result
    }

    /// 批量执行多个任务
    /// Batch execute multiple tasks
    ///
    /// # 参数
    /// # Parameters
    ///
    /// - `inputs`: 输入数据列表
    /// - `inputs`: List of input data
    ///
    /// # 返回
    /// # Returns
    ///
    /// 返回输出列表，如果某个任务失败，返回对应错误。
    /// Returns a list of outputs; if a task fails, returns the corresponding error.
    pub async fn execute_batch(
        &mut self,
        inputs: Vec<AgentInput>,
    ) -> Vec<AgentResult<AgentOutput>> {
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.execute(input).await);
        }
        results
    }

    /// 暂停运行器
    /// Pause the runner
    ///
    /// 仅支持实现了 `AgentLifecycle` 的 Agent。
    /// Only supports Agents implementing `AgentLifecycle`.
    pub async fn pause(&mut self) -> AgentResult<()>
    where
        T: AgentLifecycle,
    {
        *self.state.write().await = RunnerState::Stopping;

        self.agent
            .pause()
            .await
            .map_err(|e| AgentError::Other(format!("Pause failed: {}", e)))?;

        *self.state.write().await = RunnerState::Paused;
        Ok(())
    }

    /// 恢复运行器
    /// Resume the runner
    ///
    /// 仅支持实现了 `AgentLifecycle` 的 Agent。
    /// Only supports Agents implementing `AgentLifecycle`.
    pub async fn resume(&mut self) -> AgentResult<()>
    where
        T: AgentLifecycle,
    {
        *self.state.write().await = RunnerState::Running;

        self.agent
            .resume()
            .await
            .map_err(|e| AgentError::Other(format!("Resume failed: {}", e)))?;

        Ok(())
    }

    /// 关闭运行器
    /// Shutdown the runner
    ///
    /// 优雅关闭，释放资源。
    /// Graceful shutdown, releases resources.
    pub async fn shutdown(mut self) -> AgentResult<()> {
        *self.state.write().await = RunnerState::Stopping;

        self.agent
            .shutdown()
            .await
            .map_err(|e| AgentError::ShutdownFailed(e.to_string()))?;

        *self.state.write().await = RunnerState::Stopped;
        Ok(())
    }

    /// 中断当前执行
    /// Interrupt current execution
    pub async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
        self.agent
            .interrupt()
            .await
            .map_err(|e| AgentError::Other(format!("Interrupt failed: {}", e)))
    }

    /// 消耗运行器，返回内部 Agent
    /// Consume runner, return internal Agent
    pub fn into_inner(self) -> T {
        self.agent
    }

    /// 获取 Agent ID
    /// Get Agent ID
    pub fn id(&self) -> &str {
        self.agent.id()
    }

    /// 获取 Agent 名称
    /// Get Agent name
    pub fn name(&self) -> &str {
        self.agent.name()
    }

    /// 获取 Agent 能力
    /// Get Agent capabilities
    pub fn capabilities(&self) -> &AgentCapabilities {
        self.agent.capabilities()
    }

    /// 获取 Agent 状态
    /// Get Agent state
    pub fn agent_state(&self) -> AgentState {
        self.agent.state()
    }
}

/// 为支持消息处理的 Agent 提供的扩展方法
/// Extension methods for Agents supporting message processing
impl<T: MoFAAgent + AgentMessaging> AgentRunner<T> {
    /// 处理单个事件
    /// Handle a single event
    pub async fn handle_event(&mut self, event: AgentEvent) -> AgentResult<()> {
        self.agent.handle_event(event).await
    }

    /// 发送消息给 Agent
    /// Send message to Agent
    pub async fn send_message(&mut self, msg: AgentMessage) -> AgentResult<AgentMessage> {
        self.agent.handle_message(msg).await
    }
}

// ============================================================================
// 构建器模式
// Builder Pattern
// ============================================================================

/// AgentRunner 构建器
/// AgentRunner Builder
pub struct AgentRunnerBuilder<T: MoFAAgent> {
    agent: Option<T>,
    context: Option<AgentContext>,
}

impl<T: MoFAAgent> AgentRunnerBuilder<T> {
    /// 创建新的构建器
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            agent: None,
            context: None,
        }
    }

    /// 设置 Agent
    /// Set Agent
    pub fn with_agent(mut self, agent: T) -> Self {
        self.agent = Some(agent);
        self
    }

    /// 设置上下文
    /// Set context
    pub fn with_context(mut self, context: AgentContext) -> Self {
        self.context = Some(context);
        self
    }

    /// 构建运行器
    /// Build runner
    pub async fn build(self) -> AgentResult<AgentRunner<T>> {
        let agent = self
            .agent
            .ok_or_else(|| AgentError::ValidationFailed("Agent not set".to_string()))?;

        if let Some(context) = self.context {
            AgentRunner::with_context(agent, context).await
        } else {
            AgentRunner::new(agent).await
        }
    }
}

impl<T: MoFAAgent> Default for AgentRunnerBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 便捷函数
// Convenience Functions
// ============================================================================

/// 创建并运行 Agent（多次执行）
/// Create and run Agent (multiple executions)
pub async fn run_agents<T: MoFAAgent>(
    agent: T,
    inputs: Vec<AgentInput>,
) -> AgentResult<Vec<AgentOutput>> {
    let mut runner = AgentRunner::new(agent).await?;
    let results = runner.execute_batch(inputs).await;
    runner.shutdown().await?;
    results.into_iter().collect()
}

// ============================================================================
// 测试
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::capabilities::AgentCapabilitiesBuilder;

    struct TestAgent {
        id: String,
        name: String,
        state: AgentState,
    }

    impl TestAgent {
        fn new(id: &str, name: &str) -> Self {
            Self {
                id: id.to_string(),
                name: name.to_string(),
                state: AgentState::Created,
            }
        }
    }

    #[async_trait::async_trait]
    impl MoFAAgent for TestAgent {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn capabilities(&self) -> &AgentCapabilities {
            static CAPS: std::sync::OnceLock<AgentCapabilities> = std::sync::OnceLock::new();
            CAPS.get_or_init(|| AgentCapabilitiesBuilder::new().build())
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
            self.state = AgentState::Executing;
            let text = input.to_text();
            Ok(AgentOutput::text(format!("Echo: {}", text)))
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
    async fn test_agent_runner_new() {
        let agent = TestAgent::new("test-001", "Test Agent");
        let runner = AgentRunner::new(agent).await.unwrap();

        assert_eq!(runner.id(), "test-001");
        assert_eq!(runner.name(), "Test Agent");
        // 初始化后状态是 Created（因为 initialize 已经完成）
        // State is Created after initialization (since initialize is complete)
        assert_eq!(runner.state().await, RunnerState::Created);
    }

    #[tokio::test]
    async fn test_agent_runner_execute() {
        let agent = TestAgent::new("test-002", "Test Agent");
        let mut runner = AgentRunner::new(agent).await.unwrap();

        let input = AgentInput::text("Hello");
        let output = runner.execute(input).await.unwrap();

        assert_eq!(output.to_text(), "Echo: Hello");

        let stats = runner.stats().await;
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.successful_executions, 1);
    }

    #[tokio::test]
    async fn test_run_agents_function() {
        let agent = TestAgent::new("test-003", "Test Agent");
        let inputs = vec![AgentInput::text("Test")];
        let outputs = run_agents(agent, inputs).await.unwrap();

        assert_eq!(outputs[0].to_text(), "Echo: Test");
    }
}
