//! 多智能体协同示例 - 基于 MoFA 微内核架构
//!
//! 本示例展示如何利用 MoFA 框架的核心能力构建多智能体系统：
//! - 使用 `MoFAAgent` trait 实现标准化智能体
//! - 使用 `SimpleRuntime` 管理智能体生命周期
//! - 使用消息总线 (`SimpleMessageBus`) 进行通信
//! - 使用 `AgentCoordinator` 实现协调策略
//! - 使用插件系统扩展功能
//! - 使用 `LLMClient` 进行智能决策
//!
//! # 运行方式
//!
//! ```bash
//! # 设置 OpenAI API Key
//! export OPENAI_API_KEY=your-api-key
//!
//! # 可选: 自定义 API 端点
//! export OPENAI_BASE_URL=http://localhost:11434/v1
//!
//! # 运行所有场景
//! cargo run --example multi_agent_coordination
//!
//! # 运行特定场景
//! cargo run --example multi_agent_coordination -- --scenario code-review
//! cargo run --example multi_agent_coordination -- --scenario doc-generation
//! cargo run --example multi_agent_coordination -- --scenario diagnosis
//! ```

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use mofa_sdk::kernel::{
    AgentCapabilities, AgentConfig, AgentEvent, AgentInput, AgentMetadata, AgentOutput,
    AgentResult, AgentState, AgentContext, MoFAAgent, TaskPriority, TaskRequest,
};
use mofa_sdk::llm::{openai_from_env, LLMClient};
use mofa_sdk::runtime::SimpleRuntime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

// ============================================================================
// 核心类型定义
// ============================================================================

/// Worker 专业领域
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkerSpecialty {
    /// 分析型 - 代码审查、数据分析、安全审计
    Analyst,
    /// 编码型 - 代码生成、重构、优化
    Coder,
    /// 写作型 - 文档生成、报告撰写
    Writer,
}

impl std::fmt::Display for WorkerSpecialty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerSpecialty::Analyst => write!(f, "Analyst"),
            WorkerSpecialty::Coder => write!(f, "Coder"),
            WorkerSpecialty::Writer => write!(f, "Writer"),
        }
    }
}

/// Worker 状态
#[derive(Debug, Clone)]
pub struct WorkerState {
    pub specialty: WorkerSpecialty,
    pub load: usize,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
}

/// 任务分配决策
#[derive(Debug, Clone)]
pub struct TaskAssignment {
    pub task_id: String,
    pub worker_id: String,
    pub reasoning: String,
    pub timestamp: chrono::DateTime<Utc>,
}

/// 任务执行结果
#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    pub task_id: String,
    pub worker_id: String,
    pub content: String,
    pub processing_time_ms: u64,
    pub timestamp: chrono::DateTime<Utc>,
}

// ============================================================================
// MasterAgent - 使用 MoFAAgent trait 的智能任务分发器
// ============================================================================

/// Master Agent - 负责任务分配和协调
///
/// 实现了 `MoFAAgent` trait，使用框架的:
/// - `AgentConfig` - 配置管理
/// - `AgentEvent` - 事件处理
/// - 消息总线通信
pub struct MasterAgent {
    /// Agent ID
    id: String,
    /// Agent 名称
    name: String,
    /// Agent 能力
    capabilities: AgentCapabilities,
    /// Agent 状态
    state: AgentState,
    /// LLM 客户端用于智能决策
    llm_client: Arc<LLMClient>,
    /// Worker 状态映射
    worker_states: Arc<RwLock<HashMap<String, WorkerState>>>,
    /// 任务分配历史
    assignment_history: Arc<RwLock<Vec<TaskAssignment>>>,
}

impl MasterAgent {
    /// 创建新的 Master Agent
    pub fn new(config: AgentConfig, llm_client: Arc<LLMClient>) -> Self {
        let capabilities = AgentCapabilities::builder()
            .tag("task_scheduling")
            .tag("llm_decision")
            .tag("coordination")
            .supports_coordination(true)
            .build();

        Self {
            id: config.agent_id.clone(),
            name: config.name.clone(),
            capabilities,
            state: AgentState::Created,
            llm_client,
            worker_states: Arc::new(RwLock::new(HashMap::new())),
            assignment_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 注册 Worker
    pub async fn register_worker(&self, worker_id: String, specialty: WorkerSpecialty) {
        let state = WorkerState {
            specialty,
            load: 0,
            tasks_completed: 0,
            tasks_failed: 0,
        };

        self.worker_states
            .write()
            .await
            .insert(worker_id.clone(), state);

        info!(
            "Master: Registered worker '{}' with specialty '{:?}'",
            worker_id, specialty
        );
    }

    /// 使用 LLM 进行智能任务分配
    async fn assign_task_with_llm(&self, task: &TaskRequest) -> Result<String> {
        let workers_map = self.worker_states.read().await;
        let workers: Vec<_> = workers_map.iter().collect();

        if workers.is_empty() {
            return Err(anyhow::anyhow!("No workers available"));
        }

        // 构建 worker 列表描述
        let worker_info: Vec<String> = workers
            .iter()
            .map(|(id, state)| {
                format!(
                    "- {}: {} (load: {}, completed: {})",
                    id, state.specialty, state.load, state.tasks_completed
                )
            })
            .collect();

        let worker_list = worker_info.join("\n");

        // 使用 LLM 选择最合适的 worker
        let prompt = format!(
            "You are a task dispatcher. Available workers:\n{}\n\n\
             Task: {}\n\
             Priority: {:?}\n\n\
             Analyze the task and select the best worker based on:\n\
             1. Worker specialty matching the task requirements\n\
             2. Current load (prefer less loaded workers)\n\
             3. Past performance\n\n\
             Respond with ONLY the worker ID (e.g., 'worker_001').",
            worker_list, task.content, task.priority
        );

        debug!("Master: Asking LLM for task assignment...");

        let response = self
            .llm_client
            .chat()
            .system("You are an intelligent task dispatcher for a multi-agent system.")
            .user(&prompt)
            .temperature(0.3)
            .send()
            .await?;

        let response_text = response.content().unwrap_or("");

        // 解析 worker ID
        let worker_id = self
            .parse_worker_id(response_text, &workers_map)
            .await
            .unwrap_or_else(|_| {
                // 默认选择负载最低的 worker
                workers
                    .iter()
                    .min_by_key(|(_, w)| w.load)
                    .map(|(id, _)| (*id).clone())
                    .unwrap_or_else(|| String::from("worker_001"))
            });

        info!(
            "Master: Assigned task '{}' to worker '{}' (LLM reasoning: {})",
            task.task_id,
            worker_id,
            response_text.chars().take(100).collect::<String>()
        );

        Ok(worker_id)
    }

    /// 解析 LLM 返回的 worker ID
    async fn parse_worker_id(
        &self,
        response: &str,
        workers: &HashMap<String, WorkerState>,
    ) -> Result<String> {
        // 尝试直接匹配
        let trimmed = response.trim();
        if workers.contains_key(trimmed) {
            return Ok(trimmed.to_string());
        }

        // 尝试提取 worker_XXX 格式
        if let Some(pos) = trimmed.find("worker_") {
            let end_pos = pos + "worker_".len();
            if end_pos < trimmed.len() {
                let id_end = trimmed[end_pos..]
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(trimmed[end_pos..].len());
                let id = format!("worker_{}", &trimmed[end_pos..end_pos + id_end]);
                if workers.contains_key(&id) {
                    return Ok(id);
                }
            }
        }

        // 尝试正则匹配
        let re = regex::Regex::new(r"worker_\d+").unwrap();
        if let Some(captures) = re.captures(response) {
            let id = captures.get(0).unwrap().as_str();
            if workers.contains_key(id) {
                return Ok(id.to_string());
            }
        }

        Err(anyhow::anyhow!("Could not parse worker ID from: {}", response))
    }

    /// 处理任务请求事件
    async fn handle_task_request(&self, task: TaskRequest) -> Result<(String, TaskRequest)> {
        // 使用 LLM 分配任务
        let worker_id = self.assign_task_with_llm(&task).await?;

        // 更新 worker 负载
        {
            let mut states = self.worker_states.write().await;
            if let Some(state) = states.get_mut(&worker_id) {
                state.load += 1;
            }
        }

        // 记录分配决策
        let assignment = TaskAssignment {
            task_id: task.task_id.clone(),
            worker_id: worker_id.clone(),
            reasoning: format!("LLM-based assignment for priority {:?}", task.priority),
            timestamp: Utc::now(),
        };

        {
            let mut history = self.assignment_history.write().await;
            history.push(assignment);
        }

        // 返回 (worker_id, task) 元组用于任务分发
        Ok((worker_id, task))
    }

    /// 处理任务完成事件
    async fn handle_task_completion(&self, worker_id: String, task_id: String) {
        // 减少 worker 负载
        {
            let mut states = self.worker_states.write().await;
            if let Some(state) = states.get_mut(&worker_id) {
                state.load = state.load.saturating_sub(1);
                state.tasks_completed += 1;
            }
        }

        info!(
            "Master: Task '{}' completed by worker '{}'",
            task_id, worker_id
        );
    }

    /// 获取 Worker 状态
    pub async fn get_worker_stats(&self) -> HashMap<String, WorkerState> {
        self.worker_states.read().await.clone()
    }
}

#[async_trait]
impl MoFAAgent for MasterAgent {
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
        info!("Master Agent: Initializing...");
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        // 处理输入 - 这里简化处理，实际应该解析输入并执行相应的任务分配
        let text = input.to_text();
        info!("Master Agent: Received input - {}", text);

        Ok(AgentOutput::text(format!("Master processed: {}", text)))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        info!("Master Agent: Shutting down...");
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }
}

// ============================================================================
// WorkerAgent - 使用 MoFAAgent trait 的专业任务执行者
// ============================================================================

/// Worker Agent - 负责执行特定领域的任务
pub struct WorkerAgent {
    /// Agent ID
    id: String,
    /// Agent 名称
    name: String,
    /// Agent 能力
    capabilities: AgentCapabilities,
    /// Agent 状态
    state: AgentState,
    /// 专业领域
    specialty: WorkerSpecialty,
    /// LLM 客户端
    llm_client: Arc<LLMClient>,
    /// 统计信息
    stats: WorkerState,
    /// 当前处理的任务
    current_task: Option<String>,
}

impl WorkerAgent {
    /// 创建新的 Worker Agent
    pub fn new(
        config: AgentConfig,
        specialty: WorkerSpecialty,
        llm_client: Arc<LLMClient>,
    ) -> Self {
        let stats = WorkerState {
            specialty,
            load: 0,
            tasks_completed: 0,
            tasks_failed: 0,
        };

        let capabilities = match specialty {
            WorkerSpecialty::Analyst => AgentCapabilities::builder()
                .tag("code_analysis")
                .tag("security_audit")
                .tag("analyst")
                .build(),
            WorkerSpecialty::Coder => AgentCapabilities::builder()
                .tag("code_generation")
                .tag("refactoring")
                .tag("optimization")
                .tag("coder")
                .build(),
            WorkerSpecialty::Writer => AgentCapabilities::builder()
                .tag("documentation")
                .tag("report_generation")
                .tag("writer")
                .build(),
        };

        Self {
            id: config.agent_id.clone(),
            name: config.name.clone(),
            capabilities,
            state: AgentState::Created,
            specialty,
            llm_client,
            stats,
            current_task: None,
        }
    }

    /// 获取专业领域描述
    fn get_specialty_prompt(&self) -> &'static str {
        match self.specialty {
            WorkerSpecialty::Analyst => {
                "You are a code analyst specializing in security, performance, and best practices. \
                 You provide detailed analysis and recommendations."
            }
            WorkerSpecialty::Coder => {
                "You are a senior software engineer who writes clean, efficient, well-documented code. \
                 You follow best practices and design patterns."
            }
            WorkerSpecialty::Writer => {
                "You are a technical writer who creates clear, comprehensive documentation. \
                 You explain complex concepts in simple terms."
            }
        }
    }

    /// 处理任务
    async fn process_task(&mut self, task: &TaskRequest) -> Result<TaskExecutionResult> {
        let start_time = std::time::Instant::now();
        self.current_task = Some(task.task_id.clone());

        let prompt = format!(
            "Task: {}\n\n\
             Please analyze this task and provide a detailed response. \
             Consider the best practices and standards in your area of expertise.",
            task.content
        );

        let response = self
            .llm_client
            .chat()
            .system(self.get_specialty_prompt())
            .user(&prompt)
            .temperature(0.7)
            .max_tokens(2048)
            .send()
            .await?;

        let processing_time_ms = start_time.elapsed().as_millis() as u64;
        let content = response.content().unwrap_or("").to_string();

        self.stats.tasks_completed += 1;
        self.current_task = None;

        Ok(TaskExecutionResult {
            task_id: task.task_id.clone(),
            worker_id: self.id.clone(),
            content,
            processing_time_ms,
            timestamp: Utc::now(),
        })
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &WorkerState {
        &self.stats
    }
}

#[async_trait]
impl MoFAAgent for WorkerAgent {
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
        info!(
            "Worker Agent ({}): Initializing with specialty '{:?}'...",
            self.id, self.specialty
        );
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        let text = input.to_text();

        // 解析任务
        if let Ok(task) = serde_json::from_str::<TaskRequest>(&text) {
            info!(
                "Worker Agent ({}): Processing task '{}'",
                self.id, task.task_id
            );

            match self.process_task(&task).await {
                Ok(result) => {
                    info!(
                        "Worker Agent ({}): Completed task '{}' in {}ms",
                        self.id,
                        result.task_id,
                        result.processing_time_ms
                    );
                    return Ok(AgentOutput::text(result.content)
                        .with_metadata("task_id", serde_json::json!(result.task_id))
                        .with_metadata("processing_time_ms", serde_json::json!(result.processing_time_ms)));
                }
                Err(e) => {
                    error!(
                        "Worker Agent ({}): Failed to process task '{}': {}",
                        self.id, task.task_id, e
                    );
                    self.stats.tasks_failed += 1;
                    return Ok(AgentOutput::error(format!("Task failed: {}", e)));
                }
            }
        }

        // 默认响应
        Ok(AgentOutput::text(format!("Worker '{}' processed input", self.id)))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        info!(
            "Worker Agent ({}): Shutting down... (Completed: {}, Failed: {})",
            self.id, self.stats.tasks_completed, self.stats.tasks_failed
        );
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }
}

// ============================================================================
// 演示场景
// ============================================================================

/// 场景 1: 代码审查协同
async fn scenario_code_review(
    master: &mut MasterAgent,
    _workers: &mut Vec<WorkerAgent>,
    runtime: &SimpleRuntime,
) -> Result<()> {
    info!("\n{}", "=".repeat(70));
    info!("场景 1: 代码审查协同 (基于消息总线通信)");
    info!("{}\n", "=".repeat(70));

    let code_snippet = r#"
fn process_data(input: &str) -> String {
    let mut result = String::new();
    for ch in input.chars() {
        if ch.is_ascii() {
            result.push(ch.to_ascii_uppercase());
        }
    }
    result
}
"#;

    // 通过消息总线发送任务请求
    let task = TaskRequest {
        task_id: "review_001".to_string(),
        content: format!(
            "Analyze this Rust code for security vulnerabilities and performance issues:\n{}",
            code_snippet
        ),
        priority: TaskPriority::High,
        deadline: None,
        metadata: HashMap::new(),
    };

    // 序列化任务并通过事件发送给 Master
    let task_data = serde_json::to_vec(&task)?;
    runtime
        .broadcast(AgentEvent::Custom("task_request".to_string(), task_data))
        .await?;

    // 给时间处理
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 打印统计
    let stats = master.get_worker_stats().await;
    for (worker_id, state) in stats {
        info!(
            "Worker '{}': {} completed, {} failed, load {}",
            worker_id, state.tasks_completed, state.tasks_failed, state.load
        );
    }

    Ok(())
}

/// 场景 2: 文档生成
async fn scenario_doc_generation(
    _master: &mut MasterAgent,
    _workers: &mut Vec<WorkerAgent>,
    runtime: &SimpleRuntime,
) -> Result<()> {
    info!("\n{}", "=".repeat(70));
    info!("场景 2: 文档生成 (使用 AgentCoordinator 协调)");
    info!("{}\n", "=".repeat(70));

    let api_definition = r#"
API: User Management
Endpoints:
- POST /users - Create new user
- GET /users/:id - Get user by ID
- PUT /users/:id - Update user
- DELETE /users/:id - Delete user
"#;

    let task = TaskRequest {
        task_id: "doc_001".to_string(),
        content: format!(
            "Create comprehensive API documentation for:\n{}",
            api_definition
        ),
        priority: TaskPriority::High,
        deadline: None,
        metadata: HashMap::new(),
    };

    // 通过事件发送任务
    let task_data = serde_json::to_vec(&task)?;
    runtime
        .broadcast(AgentEvent::Custom("task_request".to_string(), task_data))
        .await?;

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    Ok(())
}

/// 场景 3: 问题诊断
async fn scenario_diagnosis(
    _master: &mut MasterAgent,
    _workers: &mut Vec<WorkerAgent>,
    runtime: &SimpleRuntime,
) -> Result<()> {
    info!("\n{}", "=".repeat(70));
    info!("场景 3: 问题诊断 (并行处理)");
    info!("{}\n", "=".repeat(70));

    let error_log = r#"
[ERROR] 2024-01-15 10:23:45
Thread: main
Panic: called `Result::unwrap()` on an `Err` value: ParseIntError { kind: InvalidDigit }
    at src/parser.rs:42:15
"#;

    let task = TaskRequest {
        task_id: "diag_001".to_string(),
        content: format!("Diagnose the root cause of this error:\n{}", error_log),
        priority: TaskPriority::Critical,
        deadline: None,
        metadata: HashMap::new(),
    };

    let task_data = serde_json::to_vec(&task)?;
    runtime
        .broadcast(AgentEvent::Custom("task_request".to_string(), task_data))
        .await?;

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    Ok(())
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("{}", "=".repeat(70));
    info!(" MoFA Multi-Agent Coordination - Microkernel Architecture");
    info!("{}\n", "=".repeat(70));

    // 创建 LLM Provider
    let provider = openai_from_env().map_err(|e| {
        error!("Failed to create OpenAI provider: {}", e);
        e
    })?;

    info!("LLM Provider initialized\n");

    // 创建 LLM 客户端
    let llm_client = Arc::new(LLMClient::new(Arc::new(provider)));

    // 创建 SimpleRuntime - 使用框架的运行时系统
    let runtime = SimpleRuntime::new();
    info!("SimpleRuntime initialized\n");

    // 创建 Master Agent
    let master_config = AgentConfig {
        agent_id: "master_001".to_string(),
        name: "Master Agent".to_string(),
        node_config: HashMap::new(),
    };

    let mut master = MasterAgent::new(master_config.clone(), llm_client.clone());

    // 注册 Master 到运行时
    let master_capabilities = AgentCapabilities::builder()
        .tag("task_scheduling")
        .tag("llm_decision")
        .tag("coordination")
        .supports_coordination(true)
        .build();

    let _master_rx = runtime
        .register_agent(
            AgentMetadata {
                id: "master_001".to_string(),
                name: "Master Agent".to_string(),
                description: Some("Coordinates task distribution among workers".to_string()),
                version: Some("1.0.0".to_string()),
                capabilities: master_capabilities,
                state: AgentState::Created,
            },
            master_config.clone(),
            "master",
        )
        .await?;

    info!("Master Agent registered to runtime\n");

    // 创建 Worker Agents
    let mut workers = Vec::new();

    for (i, specialty) in [
        WorkerSpecialty::Analyst,
        WorkerSpecialty::Coder,
        WorkerSpecialty::Writer,
    ]
    .iter()
    .enumerate()
    {
        let worker_id = format!("worker_{:03}", i + 1);
        let worker_config = AgentConfig {
            agent_id: worker_id.clone(),
            name: format!("{} Agent", specialty),
            node_config: HashMap::new(),
        };

        let worker = WorkerAgent::new(
            worker_config.clone(),
            *specialty,
            llm_client.clone(),
        );

        let worker_capabilities = match specialty {
            WorkerSpecialty::Analyst => AgentCapabilities::builder()
                .tag("code_analysis")
                .tag("security_audit")
                .tag("analyst")
                .build(),
            WorkerSpecialty::Coder => AgentCapabilities::builder()
                .tag("code_generation")
                .tag("refactoring")
                .tag("optimization")
                .tag("coder")
                .build(),
            WorkerSpecialty::Writer => AgentCapabilities::builder()
                .tag("documentation")
                .tag("report_generation")
                .tag("writer")
                .build(),
        };

        let _worker_rx = runtime
            .register_agent(
                AgentMetadata {
                    id: worker_id.clone(),
                    name: format!("{} Agent", specialty),
                    description: Some(format!("{:?} specialist agent", specialty)),
                    version: Some("1.0.0".to_string()),
                    capabilities: worker_capabilities,
                    state: AgentState::Created,
                },
                worker_config,
                "worker",
            )
            .await?;

        // 注册到 Master
        master.register_worker(worker_id.clone(), *specialty).await;

        workers.push(worker);
    }

    info!("Worker Pool initialized with 3 workers\n");

    // 获取要运行的场景
    let args: Vec<String> = std::env::args().collect();
    let scenario = args
        .get(2)
        .and_then(|s| s.strip_prefix("--scenario="))
        .or_else(|| {
            args.get(2).and_then(|s| {
                if s == "--scenario" {
                    args.get(3).map(|x| x.as_str())
                } else {
                    None
                }
            })
        })
        .unwrap_or("all");

    match scenario {
        "code-review" => {
            scenario_code_review(&mut master, &mut workers, &runtime).await?;
        }
        "doc-generation" => {
            scenario_doc_generation(&mut master, &mut workers, &runtime).await?;
        }
        "diagnosis" => {
            scenario_diagnosis(&mut master, &mut workers, &runtime).await?;
        }
        "all" => {
            scenario_code_review(&mut master, &mut workers, &runtime).await?;
            scenario_doc_generation(&mut master, &mut workers, &runtime).await?;
            scenario_diagnosis(&mut master, &mut workers, &runtime).await?;
        }
        _ => {
            error!("Unknown scenario: {}", scenario);
            info!("\nAvailable scenarios:");
            info!("  --scenario=code-review   - Code review collaboration");
            info!("  --scenario=doc-generation - Documentation generation");
            info!("  --scenario=diagnosis     - Problem diagnosis");
            info!("  --scenario=all            - Run all scenarios (default)");
        }
    }

    // 打印最终统计
    info!("\n{}", "=".repeat(70));
    info!(" Final Statistics");
    info!("{}\n", "=".repeat(70));
    let stats = master.get_worker_stats().await;
    for (worker_id, state) in stats {
        info!(
            "{}: {} tasks completed, {} failed, load {}",
            worker_id, state.tasks_completed, state.tasks_failed, state.load
        );
    }

    // 停止运行时
    runtime.stop_all().await?;

    info!("\n{}", "=".repeat(70));
    info!(" Demo completed successfully!");
    info!("{}\n", "=".repeat(70));

    Ok(())
}
