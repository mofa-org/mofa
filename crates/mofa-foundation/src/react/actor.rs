//! ReAct Actor 实现
//!
//! 基于 ractor 的 ReAct Agent Actor 实现

use super::core::{ReActAgent, ReActConfig, ReActResult, ReActStep, ReActTool};
use crate::llm::{LLMAgent, LLMError, LLMResult};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

/// ReAct Actor 消息类型
pub enum ReActMessage {
    /// 执行任务
    RunTask {
        task: String,
        reply: oneshot::Sender<LLMResult<ReActResult>>,
    },
    /// 执行任务并流式返回步骤
    RunTaskStreaming {
        task: String,
        step_tx: mpsc::Sender<ReActStep>,
        reply: oneshot::Sender<LLMResult<ReActResult>>,
    },
    /// 注册工具
    RegisterTool { tool: Arc<dyn ReActTool> },
    /// 获取状态
    GetStatus {
        reply: oneshot::Sender<ReActActorStatus>,
    },
    /// 取消当前任务
    CancelTask,
    /// 停止 Actor
    Stop,
}

impl fmt::Debug for ReActMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunTask { task, .. } => f.debug_struct("RunTask").field("task", task).finish(),
            Self::RunTaskStreaming { task, .. } => f
                .debug_struct("RunTaskStreaming")
                .field("task", task)
                .finish(),
            Self::RegisterTool { tool } => f
                .debug_struct("RegisterTool")
                .field("tool_name", &tool.name())
                .finish(),
            Self::GetStatus { .. } => f.debug_struct("GetStatus").finish(),
            Self::CancelTask => f.debug_struct("CancelTask").finish(),
            Self::Stop => f.debug_struct("Stop").finish(),
        }
    }
}

/// ReAct Actor 状态
#[derive(Debug, Clone)]
pub struct ReActActorStatus {
    /// Actor ID
    pub id: String,
    /// 是否正在执行任务
    pub is_running: bool,
    /// 已完成的任务数
    pub completed_tasks: usize,
    /// 注册的工具数
    pub tool_count: usize,
    /// 当前任务 ID
    pub current_task_id: Option<String>,
}

/// ReAct Actor 内部状态
pub struct ReActActorState {
    /// ReAct Agent 实例
    agent: Option<ReActAgent>,
    /// LLM Agent (用于延迟初始化)
    llm: Option<Arc<LLMAgent>>,
    /// 配置
    config: ReActConfig,
    /// 待注册的工具
    pending_tools: Vec<Arc<dyn ReActTool>>,
    /// 是否正在运行任务
    is_running: bool,
    /// 已完成任务数
    completed_tasks: usize,
    /// 当前任务 ID
    current_task_id: Option<String>,
    /// 取消标志
    #[allow(dead_code)]
    cancelled: bool,
}

impl ReActActorState {
    pub fn new(llm: Arc<LLMAgent>, config: ReActConfig) -> Self {
        Self {
            agent: None,
            llm: Some(llm),
            config,
            pending_tools: Vec::new(),
            is_running: false,
            completed_tasks: 0,
            current_task_id: None,
            cancelled: false,
        }
    }

    /// 确保 Agent 已初始化
    async fn ensure_agent(&mut self) -> LLMResult<&ReActAgent> {
        if self.agent.is_none() {
            let llm = self
                .llm
                .take()
                .ok_or_else(|| LLMError::ConfigError("LLM already consumed".to_string()))?;

            let agent = ReActAgent::new(llm, self.config.clone());

            // 注册待注册的工具
            for tool in self.pending_tools.drain(..) {
                agent.register_tool(tool).await;
            }

            self.agent = Some(agent);
        }

        self.agent
            .as_ref()
            .ok_or_else(|| LLMError::Other("Agent not initialized".to_string()))
    }
}

/// ReAct Actor
pub struct ReActActor;

impl ReActActor {
    /// 创建新的 ReAct Actor
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReActActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for ReActActor {
    type Msg = ReActMessage;
    type State = ReActActorState;
    type Arguments = (Arc<LLMAgent>, ReActConfig, Vec<Arc<dyn ReActTool>>);

    fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> impl Future<Output = Result<Self::State, ActorProcessingErr>> + Send {
        async move {
            let (llm, config, tools) = args;
            let mut state = ReActActorState::new(llm, config);
            state.pending_tools = tools;
            Ok(state)
        }
    }

    fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> impl Future<Output = Result<(), ActorProcessingErr>> + Send {
        // 我们需要在 future 之前处理 state 的可变借用
        // 为了避免生命周期问题，我们需要将处理逻辑分离出来
        handle_message(myself, message, state)
    }
}

/// 处理消息的异步函数
async fn handle_message(
    myself: ActorRef<ReActMessage>,
    message: ReActMessage,
    state: &mut ReActActorState,
) -> Result<(), ActorProcessingErr> {
    match message {
        ReActMessage::RunTask { task, reply } => {
            if state.is_running {
                let _ = reply.send(Err(LLMError::Other(
                    "Agent is already running a task".to_string(),
                )));
                return Ok(());
            }

            state.is_running = true;
            state.cancelled = false;
            state.current_task_id = Some(uuid::Uuid::now_v7().to_string());

            let result = match state.ensure_agent().await {
                Ok(agent) => agent.run(&task).await,
                Err(e) => Err(e),
            };

            state.is_running = false;
            state.current_task_id = None;

            if result.is_ok() {
                state.completed_tasks += 1;
            }

            let _ = reply.send(result);
        }

        ReActMessage::RunTaskStreaming {
            task,
            step_tx,
            reply,
        } => {
            if state.is_running {
                let _ = reply.send(Err(LLMError::Other(
                    "Agent is already running a task".to_string(),
                )));
                return Ok(());
            }

            state.is_running = true;
            state.cancelled = false;
            let task_id = uuid::Uuid::now_v7().to_string();
            state.current_task_id = Some(task_id.clone());

            // 执行带步骤回调的任务
            let result = match state.ensure_agent().await {
                Ok(agent) => {
                    // 运行任务
                    let result = agent.run(&task).await;

                    // 发送所有步骤
                    if let Ok(ref res) = result {
                        for step in &res.steps {
                            let _ = step_tx.send(step.clone()).await;
                        }
                    }

                    result
                }
                Err(e) => Err(e),
            };

            state.is_running = false;
            state.current_task_id = None;

            if result.is_ok() {
                state.completed_tasks += 1;
            }

            let _ = reply.send(result);
        }

        ReActMessage::RegisterTool { tool } => {
            if let Some(ref agent) = state.agent {
                agent.register_tool(tool).await;
            } else {
                state.pending_tools.push(tool);
            }
        }

        ReActMessage::GetStatus { reply } => {
            let tool_count = if let Some(ref agent) = state.agent {
                agent.get_tools().await.len()
            } else {
                state.pending_tools.len()
            };

            let status = ReActActorStatus {
                id: state.current_task_id.clone().unwrap_or_default(),
                is_running: state.is_running,
                completed_tasks: state.completed_tasks,
                tool_count,
                current_task_id: state.current_task_id.clone(),
            };

            let _ = reply.send(status);
        }

        ReActMessage::CancelTask => {
            state.cancelled = true;
        }

        ReActMessage::Stop => {
            myself.stop(Some("Stop requested".to_string()));
        }
    }

    Ok(())
}

/// ReAct Actor 引用包装
///
/// 提供便捷的方法与 ReAct Actor 交互
pub struct ReActActorRef {
    actor: ActorRef<ReActMessage>,
}

impl ReActActorRef {
    /// 从 ActorRef 创建
    pub fn new(actor: ActorRef<ReActMessage>) -> Self {
        Self { actor }
    }

    /// 执行任务
    pub async fn run_task(&self, task: impl Into<String>) -> LLMResult<ReActResult> {
        let (tx, rx) = oneshot::channel();
        self.actor
            .send_message(ReActMessage::RunTask {
                task: task.into(),
                reply: tx,
            })
            .map_err(|e| LLMError::Other(format!("Failed to send message: {}", e)))?;

        rx.await
            .map_err(|e| LLMError::Other(format!("Failed to receive response: {}", e)))?
    }

    /// 执行任务并流式返回步骤
    pub async fn run_task_streaming(
        &self,
        task: impl Into<String>,
    ) -> LLMResult<(
        mpsc::Receiver<ReActStep>,
        oneshot::Receiver<LLMResult<ReActResult>>,
    )> {
        let (step_tx, step_rx) = mpsc::channel(100);
        let (result_tx, result_rx) = oneshot::channel();

        self.actor
            .send_message(ReActMessage::RunTaskStreaming {
                task: task.into(),
                step_tx,
                reply: result_tx,
            })
            .map_err(|e| LLMError::Other(format!("Failed to send message: {}", e)))?;

        Ok((step_rx, result_rx))
    }

    /// 注册工具
    pub fn register_tool(&self, tool: Arc<dyn ReActTool>) -> LLMResult<()> {
        self.actor
            .send_message(ReActMessage::RegisterTool { tool })
            .map_err(|e| LLMError::Other(format!("Failed to register tool: {}", e)))
    }

    /// 获取状态
    pub async fn get_status(&self) -> LLMResult<ReActActorStatus> {
        let (tx, rx) = oneshot::channel();
        self.actor
            .send_message(ReActMessage::GetStatus { reply: tx })
            .map_err(|e| LLMError::Other(format!("Failed to send message: {}", e)))?;

        rx.await
            .map_err(|e| LLMError::Other(format!("Failed to receive status: {}", e)))
    }

    /// 取消当前任务
    pub fn cancel_task(&self) -> LLMResult<()> {
        self.actor
            .send_message(ReActMessage::CancelTask)
            .map_err(|e| LLMError::Other(format!("Failed to cancel task: {}", e)))
    }

    /// 停止 Actor
    pub fn stop(&self) -> LLMResult<()> {
        self.actor
            .send_message(ReActMessage::Stop)
            .map_err(|e| LLMError::Other(format!("Failed to stop actor: {}", e)))
    }

    /// 获取内部 ActorRef
    pub fn inner(&self) -> &ActorRef<ReActMessage> {
        &self.actor
    }
}

/// 启动 ReAct Actor
///
/// # 示例
///
/// ```rust,ignore
/// let (actor_ref, handle) = spawn_react_actor(
///     "my-react-agent",
///     llm_agent,
///     ReActConfig::default(),
///     vec![Arc::new(SearchTool)],
/// ).await?;
///
/// let result = actor_ref.run_task("What is Rust?").await?;
/// ```
pub async fn spawn_react_actor(
    name: impl Into<String>,
    llm: Arc<LLMAgent>,
    config: ReActConfig,
    tools: Vec<Arc<dyn ReActTool>>,
) -> LLMResult<(ReActActorRef, tokio::task::JoinHandle<()>)> {
    let (actor_ref, handle) =
        Actor::spawn(Some(name.into()), ReActActor::new(), (llm, config, tools))
            .await
            .map_err(|e| LLMError::Other(format!("Failed to spawn actor: {}", e)))?;

    Ok((ReActActorRef::new(actor_ref), handle))
}

/// AutoAgent - 自动选择最佳策略的智能 Agent
///
/// 根据任务类型自动选择：
/// - 简单问答：直接 LLM 回答
/// - 需要搜索：使用搜索工具
/// - 需要计算：使用计算工具
/// - 复杂任务：使用完整 ReAct 循环
pub struct AutoAgent {
    /// ReAct Agent
    react_agent: Arc<ReActAgent>,
    /// 直接 LLM Agent (用于简单问答)
    llm: Arc<LLMAgent>,
    /// 是否启用自动模式选择
    auto_mode: bool,
}

impl AutoAgent {
    /// 创建 AutoAgent
    pub fn new(llm: Arc<LLMAgent>, react_agent: Arc<ReActAgent>) -> Self {
        Self {
            react_agent,
            llm,
            auto_mode: true,
        }
    }

    /// 设置是否自动选择模式
    pub fn with_auto_mode(mut self, enabled: bool) -> Self {
        self.auto_mode = enabled;
        self
    }

    /// 执行任务
    pub async fn run(&self, task: impl Into<String>) -> LLMResult<AutoAgentResult> {
        let task = task.into();
        let start = std::time::Instant::now();

        if !self.auto_mode {
            // 强制使用 ReAct
            let result = self.react_agent.run(&task).await?;
            let answer = result.answer.clone();
            return Ok(AutoAgentResult {
                mode: ExecutionMode::ReAct,
                answer,
                react_result: Some(result),
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // 分析任务复杂度
        let complexity = self.analyze_complexity(&task).await;

        match complexity {
            TaskComplexity::Simple => {
                // 直接 LLM 回答
                let answer = self.llm.ask(&task).await?;
                Ok(AutoAgentResult {
                    mode: ExecutionMode::Direct,
                    answer,
                    react_result: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
            TaskComplexity::RequiresTool | TaskComplexity::Complex => {
                // 使用 ReAct
                let result = self.react_agent.run(&task).await?;
                let answer = result.answer.clone();
                Ok(AutoAgentResult {
                    mode: ExecutionMode::ReAct,
                    answer,
                    react_result: Some(result),
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
        }
    }

    /// 分析任务复杂度
    async fn analyze_complexity(&self, task: &str) -> TaskComplexity {
        // 简单的关键词分析
        let task_lower = task.to_lowercase();

        // 需要工具的关键词
        let tool_keywords = [
            "search",
            "find",
            "lookup",
            "calculate",
            "compute",
            "weather",
            "current",
            "latest",
            "today",
            "now",
        ];

        // 复杂任务关键词
        let complex_keywords = [
            "analyze",
            "compare",
            "research",
            "investigate",
            "step by step",
            "explain in detail",
        ];

        for keyword in complex_keywords {
            if task_lower.contains(keyword) {
                return TaskComplexity::Complex;
            }
        }

        for keyword in tool_keywords {
            if task_lower.contains(keyword) {
                return TaskComplexity::RequiresTool;
            }
        }

        // 问号数量
        let question_marks = task.matches('?').count();
        if question_marks > 1 {
            return TaskComplexity::Complex;
        }

        TaskComplexity::Simple
    }
}

/// 任务复杂度
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskComplexity {
    /// 简单任务 - 直接 LLM 回答
    Simple,
    /// 需要工具 - 使用单个工具
    RequiresTool,
    /// 复杂任务 - 使用完整 ReAct 循环
    Complex,
}

/// 执行模式
#[derive(Debug, Clone)]
pub enum ExecutionMode {
    /// 直接 LLM 回答
    Direct,
    /// ReAct 模式
    ReAct,
}

/// AutoAgent 执行结果
#[derive(Debug, Clone)]
pub struct AutoAgentResult {
    /// 执行模式
    pub mode: ExecutionMode,
    /// 答案
    pub answer: String,
    /// ReAct 结果 (如果使用 ReAct 模式)
    pub react_result: Option<ReActResult>,
    /// 耗时 (毫秒)
    pub duration_ms: u64,
}
