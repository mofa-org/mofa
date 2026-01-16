//! 统一 Agent 运行器
//!
//! 提供统一的 Agent 执行接口，可以运行任何实现了 `MoFAAgent` 的 Agent。
//!
//! # 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                      AgentRunner<T: MoFAAgent>                      │
//! │  ┌─────────────────────────────────────────────────────────────┐   │
//! │  │  状态管理                                                     │   │
//! │  │  • RunnerState: Initializing, Running, Paused, Stopping      │   │
//! │  └─────────────────────────────────────────────────────────────┘   │
//! │  ┌─────────────────────────────────────────────────────────────┐   │
//! │  │  执行模式                                                     │   │
//! │  │  • Single: 单次执行                                           │   │
//! │  │  • EventLoop: 事件循环（支持 AgentMessaging）                 │   │
//! │  │  • Stream: 流式执行                                           │   │
//! │  └─────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 示例
//!
//! ## 基本使用
//!
//! ```rust,ignore
//! use mofa_kernel::agent::runner::AgentRunner;
//! use mofa_kernel::agent::MoFAAgent;
//!
//! #[tokio::main]
//! async fn main() -> AgentResult<()> {
//!     let agent = MyAgent::new();
//!     let mut runner = AgentRunner::new(agent).await?;
//!
//!     // 执行任务
//!     let input = AgentInput::text("Hello, Agent!");
//!     let output = runner.execute(input).await?;
//!
//!     // 关闭
//!     runner.shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## 事件循环模式
//!
//! ```rust,ignore
//! use mofa_kernel::agent::runner::AgentRunner;
//! use mofa_kernel::agent::{MoFAAgent, AgentMessaging};
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
//!     runner.run_event_loop().await?;
//!
//!     Ok(())
//! }
//! ```

use super::capabilities::AgentCapabilities;
use super::context::{AgentContext, AgentEvent};
use super::core::{AgentLifecycle, AgentMessage, AgentMessaging, MoFAAgent};
use super::error::{AgentError, AgentResult};
use super::types::{AgentInput, AgentOutput, AgentState};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 运行器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerState {
    /// 已创建
    Created,
    /// 初始化中
    Initializing,
    /// 运行中
    Running,
    /// 暂停
    Paused,
    /// 停止中
    Stopping,
    /// 已停止
    Stopped,
    /// 错误
    Error,
}

/// 运行器统计信息
#[derive(Debug, Clone, Default)]
pub struct RunnerStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 成功次数
    pub successful_executions: u64,
    /// 失败次数
    pub failed_executions: u64,
    /// 平均执行时间（毫秒）
    pub avg_execution_time_ms: f64,
    /// 最后执行时间
    pub last_execution_time_ms: Option<u64>,
}

/// 统一 Agent 运行器
///
/// 可以运行任何实现了 `MoFAAgent` 的 Agent。
pub struct AgentRunner<T: MoFAAgent> {
    /// Agent 实例
    agent: T,
    /// 执行上下文
    context: AgentContext,
    /// 运行器状态
    state: Arc<RwLock<RunnerState>>,
    /// 统计信息
    stats: Arc<RwLock<RunnerStats>>,
}

impl<T: MoFAAgent> AgentRunner<T> {
    /// 创建新的运行器
    ///
    /// 此方法会初始化 Agent。
    pub async fn new(mut agent: T) -> AgentResult<Self> {
        let context = AgentContext::new(agent.id().to_string());

        // 初始化 Agent
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
    pub fn agent(&self) -> &T {
        &self.agent
    }

    /// 获取 Agent 可变引用
    pub fn agent_mut(&mut self) -> &mut T {
        &mut self.agent
    }

    /// 获取执行上下文
    pub fn context(&self) -> &AgentContext {
        &self.context
    }

    /// 获取运行器状态
    pub async fn state(&self) -> RunnerState {
        *self.state.read().await
    }

    /// 获取统计信息
    pub async fn stats(&self) -> RunnerStats {
        self.stats.read().await.clone()
    }

    /// 检查是否正在运行
    pub async fn is_running(&self) -> bool {
        matches!(
            *self.state.read().await,
            RunnerState::Running | RunnerState::Paused
        )
    }

    /// 执行单个任务
    ///
    /// # 参数
    ///
    /// - `input`: 输入数据
    ///
    /// # 返回
    ///
    /// 返回 Agent 的输出。
    pub async fn execute(&mut self, input: AgentInput) -> AgentResult<AgentOutput> {
        // 检查状态
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
        *self.state.write().await = RunnerState::Running;

        let start = std::time::Instant::now();

        // 执行 Agent
        let result = self.agent.execute(input, &self.context).await;

        let duration = start.elapsed().as_millis() as u64;

        // 更新统计信息
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
        let n = stats.total_executions as f64;
        stats.avg_execution_time_ms =
            (stats.avg_execution_time_ms * (n - 1.0) + duration as f64) / n;

        result
    }

    /// 批量执行多个任务
    ///
    /// # 参数
    ///
    /// - `inputs`: 输入数据列表
    ///
    /// # 返回
    ///
    /// 返回输出列表，如果某个任务失败，返回对应错误。
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
    ///
    /// 仅支持实现了 `AgentLifecycle` 的 Agent。
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
    ///
    /// 仅支持实现了 `AgentLifecycle` 的 Agent。
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
    ///
    /// 优雅关闭，释放资源。
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
    pub async fn interrupt(&mut self) -> AgentResult<super::types::InterruptResult> {
        self.agent
            .interrupt()
            .await
            .map_err(|e| AgentError::Other(format!("Interrupt failed: {}", e)))
    }

    /// 消耗运行器，返回内部 Agent
    pub fn into_inner(self) -> T {
        self.agent
    }

    /// 获取 Agent ID
    pub fn id(&self) -> &str {
        self.agent.id()
    }

    /// 获取 Agent 名称
    pub fn name(&self) -> &str {
        self.agent.name()
    }

    /// 获取 Agent 能力
    pub fn capabilities(&self) -> &AgentCapabilities {
        self.agent.capabilities()
    }

    /// 获取 Agent 状态
    pub fn agent_state(&self) -> AgentState {
        self.agent.state()
    }
}

/// 为支持消息处理的 Agent 提供的扩展方法
impl<T: MoFAAgent + AgentMessaging> AgentRunner<T> {
    /// 处理单个事件
    pub async fn handle_event(&mut self, event: AgentEvent) -> AgentResult<()> {
        self.agent.handle_event(event).await
    }

    /// 发送消息给 Agent
    pub async fn send_message(
        &mut self,
        msg: AgentMessage,
    ) -> AgentResult<AgentMessage> {
        self.agent.handle_message(msg).await
    }
}

// ============================================================================
// 构建器模式
// ============================================================================

/// AgentRunner 构建器
pub struct AgentRunnerBuilder<T: MoFAAgent> {
    agent: Option<T>,
    context: Option<AgentContext>,
}

impl<T: MoFAAgent> AgentRunnerBuilder<T> {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            agent: None,
            context: None,
        }
    }

    /// 设置 Agent
    pub fn with_agent(mut self, agent: T) -> Self {
        self.agent = Some(agent);
        self
    }

    /// 设置上下文
    pub fn with_context(mut self, context: AgentContext) -> Self {
        self.context = Some(context);
        self
    }

    /// 构建运行器
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
// ============================================================================

/// 快速创建并启动运行器
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::runner::run_agent;
/// use mofa_kernel::agent::MoFAAgent;
///
/// #[tokio::main]
/// async fn main() -> AgentResult<()> {
///     let agent = MyAgent::new();
///     let result = run_agent(agent, AgentInput::text("Hello")).await?;
///     Ok(())
/// }
/// ```
pub async fn run_agent<T: MoFAAgent>(
    agent: T,
    input: AgentInput,
) -> AgentResult<AgentOutput> {
    let mut runner = AgentRunner::new(agent).await?;
    let output = runner.execute(input).await?;
    runner.shutdown().await?;
    Ok(output)
}

// ============================================================================
// 测试
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
    async fn test_run_agent_function() {
        let agent = TestAgent::new("test-003", "Test Agent");
        let input = AgentInput::text("Test");
        let output = run_agent(agent, input).await.unwrap();

        assert_eq!(output.to_text(), "Echo: Test");
    }
}
