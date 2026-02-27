//! 默认秘书行为实现
//! Default secretary behavior implementation
//!
//! 提供一个开箱即用的秘书行为实现，包含完整的5阶段工作流程。
//! Provides an out-of-the-box secretary behavior implementation with a complete 5-stage workflow.

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use super::clarifier::{ClarificationStrategy, RequirementClarifier};
use super::coordinator::{DispatchStrategy, TaskCoordinator};
use super::monitor::TaskMonitor;
use super::reporter::{ReportConfig, Reporter};
use super::todo::TodoManager;
use super::types::*;

use crate::secretary::agent_router::{AgentInfo, AgentProvider, AgentRouter};
use crate::secretary::llm::{ChatMessage, ConversationHistory, LLMProvider};

// 使用 mofa-kernel 的核心抽象
// Using the core abstractions of mofa-kernel
use mofa_kernel::agent::secretary::{SecretaryBehavior, SecretaryContext};

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

// =============================================================================
// 默认秘书状态
// Default Secretary State
// =============================================================================

/// 默认秘书状态
/// Default secretary state
pub struct DefaultSecretaryState {
    /// 任务管理器
    /// Task manager
    pub todo_manager: TodoManager,
    /// 需求澄清器
    /// Requirement clarifier
    pub clarifier: RequirementClarifier,
    /// 任务协调器
    /// Task coordinator
    pub coordinator: TaskCoordinator,
    /// 任务监控器
    /// Task monitor
    pub monitor: TaskMonitor,
    /// 汇报器
    /// Reporter
    pub reporter: Reporter,
    /// 对话历史
    /// Conversation history
    pub conversation_history: Vec<ChatMessage>,
    /// 当前工作阶段
    /// Current work phase
    pub current_phase: WorkPhase,
}

impl DefaultSecretaryState {
    /// 创建新的默认状态
    /// Create new default state
    pub fn new(
        clarification_strategy: ClarificationStrategy,
        dispatch_strategy: DispatchStrategy,
        report_config: ReportConfig,
    ) -> Self {
        Self {
            todo_manager: TodoManager::new(),
            clarifier: RequirementClarifier::new(clarification_strategy),
            coordinator: TaskCoordinator::new(dispatch_strategy),
            monitor: TaskMonitor::new(),
            reporter: Reporter::new(report_config),
            conversation_history: Vec::new(),
            current_phase: WorkPhase::ReceivingIdea,
        }
    }
}

// =============================================================================
// 默认秘书配置
// Default Secretary Configuration
// =============================================================================

/// 默认秘书配置
/// Default secretary configuration
#[derive(Debug, Clone)]
pub struct DefaultSecretaryConfig {
    /// 秘书名称
    /// Secretary name
    pub name: String,
    /// 澄清策略
    /// Clarification strategy
    pub clarification_strategy: ClarificationStrategy,
    /// 分配策略
    /// Dispatch strategy
    pub dispatch_strategy: DispatchStrategy,
    /// 汇报配置
    /// Reporting configuration
    pub report_config: ReportConfig,
    /// 是否自动澄清
    /// Whether to auto-clarify
    pub auto_clarify: bool,
    /// 是否自动分配
    /// Whether to auto-dispatch
    pub auto_dispatch: bool,
    /// 是否使用LLM
    /// Whether to use LLM
    pub use_llm: bool,
    /// 系统提示词
    /// System prompt
    pub system_prompt: Option<String>,
}

impl Default for DefaultSecretaryConfig {
    fn default() -> Self {
        Self {
            name: "智能秘书".to_string(),
            clarification_strategy: ClarificationStrategy::Automatic,
            dispatch_strategy: DispatchStrategy::CapabilityFirst,
            report_config: ReportConfig::default(),
            auto_clarify: true,
            auto_dispatch: true,
            use_llm: true,
            system_prompt: None,
        }
    }
}

// =============================================================================
// 默认秘书行为
// Default Secretary Behavior
// =============================================================================

/// 默认秘书行为实现
/// Default secretary behavior implementation
///
/// 实现了完整的5阶段工作流程：
/// Implemented a complete 5-stage workflow:
/// 1. 接收想法 → 记录Todo
/// 1. Receive ideas → Record Todo
/// 2. 澄清需求 → 生成项目文档
/// 2. Clarify requirements → Generate project documents
/// 3. 调度分配 → 调用执行Agent
/// 3. Schedule and dispatch → Call execution Agents
/// 4. 监控反馈 → 推送关键决策
/// 4. Monitor feedback → Push key decisions
/// 5. 验收汇报 → 更新Todo
/// 5. Acceptance reporting → Update Todo
pub struct DefaultSecretaryBehavior {
    /// 配置
    /// Configuration
    config: DefaultSecretaryConfig,
    /// LLM提供者
    /// LLM provider
    llm: Option<Arc<dyn LLMProvider>>,
    /// Agent提供者
    /// Agent provider
    agent_provider: Option<Arc<dyn AgentProvider>>,
    /// Agent路由器
    /// Agent router
    agent_router: Option<Arc<dyn AgentRouter>>,
    /// 预注册的执行器
    /// Pre-registered executors
    executors: Vec<AgentInfo>,
}

impl DefaultSecretaryBehavior {
    /// 创建新的默认秘书行为
    /// Create new default secretary behavior
    pub fn new(config: DefaultSecretaryConfig) -> Self {
        Self {
            config,
            llm: None,
            agent_provider: None,
            agent_router: None,
            executors: Vec::new(),
        }
    }

    /// 设置LLM提供者
    /// Set LLM provider
    pub fn with_llm(mut self, llm: Arc<dyn LLMProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// 设置Agent提供者
    /// Set Agent provider
    pub fn with_agent_provider(mut self, provider: Arc<dyn AgentProvider>) -> Self {
        self.agent_provider = Some(provider);
        self
    }

    /// 设置Agent路由器
    /// Set Agent router
    pub fn with_agent_router(mut self, router: Arc<dyn AgentRouter>) -> Self {
        self.agent_router = Some(router);
        self
    }

    /// 添加执行器
    /// Add executor
    pub fn with_executor(mut self, executor: AgentInfo) -> Self {
        self.executors.push(executor);
        self
    }

    /// 获取默认系统提示词
    /// Get default system prompt
    fn default_system_prompt(&self) -> &'static str {
        r#"你是一个专业的项目秘书Agent，负责帮助用户管理任务和协调工作。

你的主要职责包括：
1. 接收用户的想法和需求，记录为TODO任务
2. 与用户交互澄清需求，生成结构化的项目需求文档
3. 根据需求分析，将任务分配给合适的执行Agent
4. 监控任务执行进度，在需要时请求用户决策
5. 汇总执行结果，生成汇报并更新TODO状态

你应该：
- 始终保持专业、礼貌的态度
- 主动询问澄清模糊的需求
- 合理评估任务优先级
- 及时汇报重要进展
- 在遇到需要人类决策的问题时，清晰地呈现选项"#
    }

    // =========================================================================
    // 阶段处理方法
    // Phase Handling Methods
    // =========================================================================

    /// 阶段1: 处理新想法
    /// Phase 1: Handling new ideas
    async fn handle_idea(
        &self,
        content: &str,
        priority: Option<TodoPriority>,
        metadata: Option<HashMap<String, String>>,
        ctx: &mut SecretaryContext<DefaultSecretaryState>,
    ) -> GlobalResult<Vec<DefaultOutput>> {
        let mut outputs = Vec::new();

        // 创建Todo
        // Create Todo
        let todo = ctx
            .state_mut()
            .todo_manager
            .receive_idea(content, priority, metadata)
            .await;

        outputs.push(DefaultOutput::Acknowledgment {
            message: format!(
                "已记录您的需求，任务ID: {}。优先级: {:?}",
                todo.id, todo.priority
            ),
        });

        tracing::info!("Received idea: {} -> {}", todo.id, content);

        // 自动澄清
        // Auto-clarify
        if self.config.auto_clarify {
            let clarify_outputs = self.clarify_requirement_internal(&todo.id, ctx).await?;
            outputs.extend(clarify_outputs);
        }

        Ok(outputs)
    }

    /// 阶段2: 澄清需求
    /// Phase 2: Clarifying requirements
    async fn clarify_requirement_internal(
        &self,
        todo_id: &str,
        ctx: &mut SecretaryContext<DefaultSecretaryState>,
    ) -> GlobalResult<Vec<DefaultOutput>> {
        let mut outputs = Vec::new();

        let todo = ctx
            .state()
            .todo_manager
            .get_todo(todo_id)
            .await
            .ok_or_else(|| GlobalError::Other(format!("Todo not found: {}", todo_id)))?;

        ctx.state_mut()
            .todo_manager
            .update_status(todo_id, TodoStatus::Clarifying)
            .await;

        // 生成需求文档 - 优先使用LLM
        // Generate requirement document - LLM preferred
        let requirement = if let Some(ref llm) = self.llm {
            tracing::info!("Using LLM for requirement clarification");

            // 构建对话历史
            // Build conversation history
            let mut conversation = ConversationHistory::new();

            // 系统提示词
            // System prompt
            conversation.add_system(
                "你是一个专业的需求分析师，请将用户的需求想法转换为结构化的项目需求文档。",
            );

            // 用户需求
            // User requirement
            conversation.add_user(format!("用户需求: {}", todo.raw_idea));

            // 发送请求给LLM
            // Send request to LLM
            let response = llm.chat(conversation.to_vec()).await?;
            tracing::info!("LLM response: {}", response);

            // 解析LLM的JSON响应
            // Parse JSON response from LLM
            serde_json::from_str::<ProjectRequirement>(response.as_str())?
        } else {
            // 回退到快速澄清
            // Fallback to quick clarification
            ctx.state()
                .clarifier
                .quick_clarify(todo_id, &todo.raw_idea)
                .await?
        };

        ctx.state_mut()
            .todo_manager
            .set_requirement(todo_id, requirement.clone())
            .await;

        outputs.push(DefaultOutput::Acknowledgment {
            message: format!(
                "需求已分析完成：\n标题: {}\n子任务数: {}\n验收标准: {} 条",
                requirement.title,
                requirement.subtasks.len(),
                requirement.acceptance_criteria.len()
            ),
        });

        // 自动分配
        // Auto-dispatch
        if self.config.auto_dispatch {
            let dispatch_outputs = self.dispatch_task_internal(todo_id, ctx).await?;
            outputs.extend(dispatch_outputs);
        }

        Ok(outputs)
    }

    /// 阶段3: 分配任务
    /// Phase 3: Dispatching tasks
    async fn dispatch_task_internal(
        &self,
        todo_id: &str,
        ctx: &mut SecretaryContext<DefaultSecretaryState>,
    ) -> GlobalResult<Vec<DefaultOutput>> {
        let mut outputs = Vec::new();

        let todo = ctx
            .state()
            .todo_manager
            .get_todo(todo_id)
            .await
            .ok_or_else(|| GlobalError::Other(format!("Todo not found: {}", todo_id)))?;

        let requirement = todo
            .clarified_requirement
            .ok_or_else(|| GlobalError::Other(format!("Requirement not clarified for: {}", todo_id)))?;

        // 检查可用执行器
        // Check available executors
        let executors = ctx.state().coordinator.list_available_executors().await;
        if executors.is_empty() {
            outputs.push(DefaultOutput::Message {
                content: "当前没有可用的执行Agent，任务将等待分配。".to_string(),
            });
            return Ok(outputs);
        }

        // 准备上下文
        // Prepare context
        let mut context = HashMap::new();
        context.insert("todo_id".to_string(), todo_id.to_string());
        context.insert("raw_idea".to_string(), todo.raw_idea.clone());

        // 分配任务
        // Dispatch task
        let results = ctx
            .state()
            .coordinator
            .dispatch_requirement(&requirement, context)
            .await?;

        // 记录分配
        // Record assignment
        let agent_ids: Vec<String> = results.iter().map(|r| r.agent_id.clone()).collect();
        ctx.state_mut()
            .todo_manager
            .assign_agents(todo_id, agent_ids.clone())
            .await;

        // 开始监控
        // Start monitoring
        for result in &results {
            ctx.state()
                .monitor
                .start_monitoring(&result.subtask_id, &result.agent_id)
                .await;
        }

        ctx.state_mut()
            .todo_manager
            .update_status(todo_id, TodoStatus::InProgress)
            .await;

        outputs.push(DefaultOutput::Acknowledgment {
            message: format!(
                "任务已分配给 {} 个执行Agent: {}",
                results.len(),
                agent_ids.join(", ")
            ),
        });

        Ok(outputs)
    }

    /// 处理决策响应
    /// Handling decision response
    async fn handle_decision(
        &self,
        decision_id: &str,
        selected_option: usize,
        comment: Option<String>,
        ctx: &mut SecretaryContext<DefaultSecretaryState>,
    ) -> GlobalResult<Vec<DefaultOutput>> {
        ctx.state()
            .monitor
            .submit_human_response(decision_id, selected_option, comment)
            .await?;

        Ok(vec![DefaultOutput::Acknowledgment {
            message: format!(
                "决策 {} 已记录，选择了选项 {}",
                decision_id, selected_option
            ),
        }])
    }

    /// 处理查询
    /// Handling queries
    async fn handle_query(
        &self,
        query: QueryType,
        ctx: &mut SecretaryContext<DefaultSecretaryState>,
    ) -> GlobalResult<Vec<DefaultOutput>> {
        match query {
            QueryType::ListTodos { filter } => {
                let todos = if let Some(status) = filter {
                    ctx.state().todo_manager.list_by_status(status).await
                } else {
                    ctx.state().todo_manager.list_todos().await
                };

                let summary = if todos.is_empty() {
                    "当前没有任务。".to_string()
                } else {
                    let mut s = format!("共 {} 个任务：\n", todos.len());
                    for todo in &todos {
                        s.push_str(&format!(
                            "- {} [{:?}] {:?}: {}\n",
                            todo.id,
                            todo.priority,
                            todo.status,
                            todo.raw_idea.chars().take(30).collect::<String>()
                        ));
                    }
                    s
                };

                Ok(vec![DefaultOutput::Message { content: summary }])
            }
            QueryType::GetTodo { todo_id } => {
                if let Some(todo) = ctx.state().todo_manager.get_todo(&todo_id).await {
                    let detail = format!(
                        "任务详情:\nID: {}\n需求: {}\n状态: {:?}\n优先级: {:?}\n分配Agent: {:?}",
                        todo.id, todo.raw_idea, todo.status, todo.priority, todo.assigned_agents
                    );
                    Ok(vec![DefaultOutput::Message { content: detail }])
                } else {
                    Ok(vec![DefaultOutput::Error {
                        message: format!("未找到任务: {}", todo_id),
                    }])
                }
            }
            QueryType::Statistics => {
                let stats = ctx.state().todo_manager.get_statistics().await;
                let summary = format!(
                    "统计信息:\n{}",
                    stats
                        .iter()
                        .map(|(k, v)| format!("- {}: {}", k, v))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                Ok(vec![DefaultOutput::Message { content: summary }])
            }
            QueryType::PendingDecisions => {
                let decisions = ctx.state().monitor.get_pending_decisions().await;
                if decisions.is_empty() {
                    Ok(vec![DefaultOutput::Message {
                        content: "当前没有待处理的决策。".to_string(),
                    }])
                } else {
                    Ok(decisions
                        .into_iter()
                        .map(|d| DefaultOutput::DecisionRequired { decision: d })
                        .collect())
                }
            }
            QueryType::Reports { report_type } => {
                let reports = if let Some(rt) = report_type {
                    ctx.state().reporter.get_by_type(rt).await
                } else {
                    ctx.state().reporter.get_history().await
                };

                if reports.is_empty() {
                    Ok(vec![DefaultOutput::Message {
                        content: "暂无汇报历史。".to_string(),
                    }])
                } else {
                    Ok(reports
                        .into_iter()
                        .map(|r| DefaultOutput::Report { report: r })
                        .collect())
                }
            }
        }
    }

    /// 处理命令
    /// Handling commands
    async fn handle_command(
        &self,
        cmd: SecretaryCommand,
        ctx: &mut SecretaryContext<DefaultSecretaryState>,
    ) -> GlobalResult<Vec<DefaultOutput>> {
        match cmd {
            SecretaryCommand::Clarify { todo_id } => {
                self.clarify_requirement_internal(&todo_id, ctx).await
            }
            SecretaryCommand::Dispatch { todo_id } => {
                self.dispatch_task_internal(&todo_id, ctx).await
            }
            SecretaryCommand::Cancel { todo_id, reason } => {
                ctx.state_mut()
                    .todo_manager
                    .update_status(&todo_id, TodoStatus::Cancelled)
                    .await;
                Ok(vec![DefaultOutput::Acknowledgment {
                    message: format!("任务 {} 已取消，原因: {}", todo_id, reason),
                }])
            }
            SecretaryCommand::GenerateReport { report_type } => {
                let todos = ctx.state().todo_manager.list_todos().await;
                let report = match report_type {
                    ReportType::Progress => {
                        let stats = ctx.state().todo_manager.get_statistics().await;
                        let stats_json: HashMap<String, serde_json::Value> = stats
                            .into_iter()
                            .map(|(k, v)| (k, serde_json::json!(v)))
                            .collect();
                        ctx.state()
                            .reporter
                            .generate_progress_report(&todos, stats_json)
                            .await
                    }
                    ReportType::DailySummary => {
                        ctx.state().reporter.generate_daily_summary(&todos).await
                    }
                    _ => {
                        let stats = ctx.state().todo_manager.get_statistics().await;
                        let stats_json: HashMap<String, serde_json::Value> = stats
                            .into_iter()
                            .map(|(k, v)| (k, serde_json::json!(v)))
                            .collect();
                        ctx.state()
                            .reporter
                            .generate_progress_report(&todos, stats_json)
                            .await
                    }
                };
                Ok(vec![DefaultOutput::Report { report }])
            }
            SecretaryCommand::Pause | SecretaryCommand::Resume | SecretaryCommand::Shutdown => {
                // 这些命令由核心引擎处理
                // These commands are handled by the core engine
                Ok(vec![DefaultOutput::Acknowledgment {
                    message: "命令已收到".to_string(),
                }])
            }
        }
    }
}

#[async_trait]
impl SecretaryBehavior for DefaultSecretaryBehavior {
    type Input = DefaultInput;
    type Output = DefaultOutput;
    type State = DefaultSecretaryState;

    fn initial_state(&self) -> Self::State {
        let mut state = DefaultSecretaryState::new(
            self.config.clarification_strategy.clone(),
            self.config.dispatch_strategy.clone(),
            self.config.report_config.clone(),
        );

        // 设置Agent提供者和路由器
        // Set Agent provider and router
        if let Some(ref provider) = self.agent_provider {
            state.coordinator.set_agent_provider(provider.clone());
        }
        if let Some(ref router) = self.agent_router {
            state.coordinator.set_agent_router(router.clone());
        }

        // 初始化系统提示词
        // Initialize system prompt
        let system_prompt = self
            .config
            .system_prompt
            .as_deref()
            .unwrap_or_else(|| self.default_system_prompt());
        state
            .conversation_history
            .push(ChatMessage::system(system_prompt));

        state
    }

    fn welcome_message(&self) -> Option<Self::Output> {
        Some(DefaultOutput::Message {
            content: format!(
                "您好！我是{}，您的智能秘书。我可以帮您管理任务、协调工作。请告诉我您的需求。",
                self.config.name
            ),
        })
    }

    async fn handle_input(
        &self,
        input: Self::Input,
        ctx: &mut SecretaryContext<Self::State>,
    ) -> Result<Vec<Self::Output>, mofa_kernel::agent::secretary::SecretaryError> {
        match input {
            DefaultInput::Idea {
                content,
                priority,
                metadata,
            } => self.handle_idea(&content, priority, metadata, ctx).await
                .map_err(|e| mofa_kernel::agent::secretary::SecretaryError::InputHandlingFailed(e.to_string())),
            DefaultInput::Decision {
                decision_id,
                selected_option,
                comment,
            } => {
                self.handle_decision(&decision_id, selected_option, comment, ctx)
                    .await
                    .map_err(|e| mofa_kernel::agent::secretary::SecretaryError::InputHandlingFailed(e.to_string()))
            }
            DefaultInput::Query(query) => self.handle_query(query, ctx).await
                .map_err(|e| mofa_kernel::agent::secretary::SecretaryError::InputHandlingFailed(e.to_string())),
            DefaultInput::Command(cmd) => self.handle_command(cmd, ctx).await
                .map_err(|e| mofa_kernel::agent::secretary::SecretaryError::InputHandlingFailed(e.to_string())),
        }
    }

    async fn periodic_check(
        &self,
        ctx: &mut SecretaryContext<Self::State>,
    ) -> Result<Vec<Self::Output>, mofa_kernel::agent::secretary::SecretaryError> {
        // 检查待处理的决策
        // Check for pending decisions
        let pending_decisions = ctx.state().monitor.get_pending_decisions().await;
        Ok(pending_decisions
            .into_iter()
            .map(|d| DefaultOutput::DecisionRequired { decision: d })
            .collect())
    }

    fn handle_error(&self, error: &mofa_kernel::agent::secretary::SecretaryError) -> Option<Self::Output> {
        Some(DefaultOutput::Error {
            message: format!("处理请求时出错: {}", error),
        })
    }
}

// =============================================================================
// 构建器
// Builder
// =============================================================================

/// 默认秘书构建器
/// Default secretary builder
pub struct DefaultSecretaryBuilder {
    config: DefaultSecretaryConfig,
    llm: Option<Arc<dyn LLMProvider>>,
    agent_provider: Option<Arc<dyn AgentProvider>>,
    agent_router: Option<Arc<dyn AgentRouter>>,
    executors: Vec<AgentInfo>,
}

impl DefaultSecretaryBuilder {
    /// 创建新的构建器
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: DefaultSecretaryConfig::default(),
            llm: None,
            agent_provider: None,
            agent_router: None,
            executors: Vec::new(),
        }
    }

    /// 设置名称
    /// Set name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.config.name = name.into();
        self
    }

    /// 设置LLM
    /// Set LLM
    pub fn with_llm(mut self, llm: Arc<dyn LLMProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// 设置澄清策略
    /// Set clarification strategy
    pub fn with_clarification_strategy(mut self, strategy: ClarificationStrategy) -> Self {
        self.config.clarification_strategy = strategy;
        self
    }

    /// 设置分配策略
    /// Set dispatch strategy
    pub fn with_dispatch_strategy(mut self, strategy: DispatchStrategy) -> Self {
        self.config.dispatch_strategy = strategy;
        self
    }

    /// 设置是否自动澄清
    /// Set whether to auto-clarify
    pub fn with_auto_clarify(mut self, auto: bool) -> Self {
        self.config.auto_clarify = auto;
        self
    }

    /// 设置是否自动分配
    /// Set whether to auto-dispatch
    pub fn with_auto_dispatch(mut self, auto: bool) -> Self {
        self.config.auto_dispatch = auto;
        self
    }

    /// 设置系统提示词
    /// Set system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.system_prompt = Some(prompt.into());
        self
    }

    /// 设置Agent提供者
    /// Set Agent provider
    pub fn with_agent_provider(mut self, provider: Arc<dyn AgentProvider>) -> Self {
        self.agent_provider = Some(provider);
        self
    }

    /// 设置Agent路由器
    /// Set Agent router
    pub fn with_agent_router(mut self, router: Arc<dyn AgentRouter>) -> Self {
        self.agent_router = Some(router);
        self
    }

    /// 添加执行器
    /// Add executor
    pub fn with_executor(mut self, executor: AgentInfo) -> Self {
        self.executors.push(executor);
        self
    }

    /// 构建秘书行为
    /// Build secretary behavior
    #[must_use]
    pub fn build(self) -> DefaultSecretaryBehavior {
        let mut behavior = DefaultSecretaryBehavior::new(self.config);

        if let Some(llm) = self.llm {
            behavior = behavior.with_llm(llm);
        }
        if let Some(provider) = self.agent_provider {
            behavior = behavior.with_agent_provider(provider);
        }
        if let Some(router) = self.agent_router {
            behavior = behavior.with_agent_router(router);
        }
        for executor in self.executors {
            behavior = behavior.with_executor(executor);
        }

        behavior
    }
}

impl Default for DefaultSecretaryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let behavior = DefaultSecretaryBuilder::new()
            .with_name("测试秘书")
            .with_auto_clarify(false)
            .build();

        assert!(!behavior.config.auto_clarify);
        assert_eq!(behavior.config.name, "测试秘书");
    }

    #[test]
    fn test_welcome_message() {
        let behavior = DefaultSecretaryBuilder::new().with_name("小助手").build();

        let welcome = behavior.welcome_message().unwrap();
        match welcome {
            DefaultOutput::Message { content } => {
                assert!(content.contains("小助手"));
            }
            _ => panic!("Expected Message output"),
        }
    }
}
