//! 默认类型定义
//!
//! 包含默认秘书实现使用的所有类型。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Todo 任务类型
// =============================================================================

/// Todo 任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    /// 待处理
    Pending,
    /// 需求澄清中
    Clarifying,
    /// 执行中
    InProgress,
    /// 等待反馈
    WaitingFeedback,
    /// 已完成
    Completed,
    /// 已取消
    Cancelled,
    /// 阻塞中（需要人类决策）
    Blocked(String),
}

/// Todo 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TodoPriority {
    Low = 0,
    Medium = 1,
    High = 2,
    Urgent = 3,
}

/// Todo 任务项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// 任务ID
    pub id: String,
    /// 原始想法/需求描述
    pub raw_idea: String,
    /// 澄清后的需求文档
    pub clarified_requirement: Option<ProjectRequirement>,
    /// 任务状态
    pub status: TodoStatus,
    /// 优先级
    pub priority: TodoPriority,
    /// 创建时间（Unix时间戳）
    pub created_at: u64,
    /// 更新时间
    pub updated_at: u64,
    /// 分配的执行Agent ID列表
    pub assigned_agents: Vec<String>,
    /// 执行结果
    pub execution_result: Option<ExecutionResult>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl TodoItem {
    pub fn new(id: &str, raw_idea: &str, priority: TodoPriority) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id: id.to_string(),
            raw_idea: raw_idea.to_string(),
            clarified_requirement: None,
            status: TodoStatus::Pending,
            priority,
            created_at: now,
            updated_at: now,
            assigned_agents: Vec::new(),
            execution_result: None,
            metadata: HashMap::new(),
        }
    }

    /// 更新状态
    pub fn update_status(&mut self, status: TodoStatus) {
        self.status = status;
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

// =============================================================================
// 项目需求类型
// =============================================================================

/// 项目需求文档（澄清后的需求）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRequirement {
    /// 需求标题
    pub title: String,
    /// 详细描述
    pub description: String,
    /// 验收标准
    pub acceptance_criteria: Vec<String>,
    /// 子任务列表
    pub subtasks: Vec<Subtask>,
    /// 依赖关系
    pub dependencies: Vec<String>,
    /// 预估工作量（可选）
    pub estimated_effort: Option<String>,
    /// 相关资源
    pub resources: Vec<Resource>,
}

/// 子任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    /// 子任务ID
    pub id: String,
    /// 子任务描述
    pub description: String,
    /// 所需能力（用于匹配执行Agent）
    pub required_capabilities: Vec<String>,
    /// 执行顺序（可并行的任务可以有相同的顺序号）
    pub order: u32,
    /// 依赖的其他子任务ID
    pub depends_on: Vec<String>,
}

/// 相关资源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// 资源名称
    pub name: String,
    /// 资源类型（file/url/api等）
    pub resource_type: String,
    /// 资源路径或URL
    pub path: String,
}

// =============================================================================
// 执行结果类型
// =============================================================================

/// 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 结果摘要
    pub summary: String,
    /// 详细输出
    pub details: HashMap<String, String>,
    /// 产出物列表
    pub artifacts: Vec<Artifact>,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// 产出物
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// 产出物名称
    pub name: String,
    /// 产出物类型
    pub artifact_type: String,
    /// 产出物路径或内容
    pub content: String,
}

// =============================================================================
// 决策类型
// =============================================================================

/// 关键决策（需要推送给人类）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalDecision {
    /// 决策ID
    pub id: String,
    /// 关联的Todo ID
    pub todo_id: String,
    /// 决策类型
    pub decision_type: DecisionType,
    /// 决策描述
    pub description: String,
    /// 可选方案
    pub options: Vec<DecisionOption>,
    /// 推荐方案（如果有）
    pub recommended_option: Option<usize>,
    /// 截止时间（Unix时间戳，可选）
    pub deadline: Option<u64>,
    /// 创建时间
    pub created_at: u64,
    /// 人类响应
    pub human_response: Option<HumanResponse>,
}

/// 决策类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionType {
    /// 需求澄清
    RequirementClarification,
    /// 技术选型
    TechnicalChoice,
    /// 优先级调整
    PriorityAdjustment,
    /// 异常处理
    ExceptionHandling,
    /// 资源申请
    ResourceRequest,
    /// 其他
    Other(String),
}

/// 决策选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOption {
    /// 选项标签
    pub label: String,
    /// 选项描述
    pub description: String,
    /// 预期影响
    pub impact: String,
}

/// 人类响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanResponse {
    /// 选择的选项索引
    pub selected_option: usize,
    /// 附加说明
    pub comment: Option<String>,
    /// 响应时间
    pub responded_at: u64,
}

// =============================================================================
// 汇报类型
// =============================================================================

/// 汇报消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// 汇报ID
    pub id: String,
    /// 汇报类型
    pub report_type: ReportType,
    /// 关联的Todo ID列表
    pub todo_ids: Vec<String>,
    /// 汇报内容
    pub content: String,
    /// 附带的统计数据
    pub statistics: HashMap<String, serde_json::Value>,
    /// 创建时间
    pub created_at: u64,
}

/// 汇报类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReportType {
    /// 任务完成汇报
    TaskCompletion,
    /// 进度汇报
    Progress,
    /// 异常汇报
    Exception,
    /// 每日总结
    DailySummary,
}

// =============================================================================
// A2A 消息类型
// =============================================================================

/// 秘书 Agent 与执行 Agent 之间的 A2A 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretaryMessage {
    /// 分配任务
    AssignTask {
        task_id: String,
        subtask: Subtask,
        context: HashMap<String, String>,
    },
    /// 任务状态查询
    QueryTaskStatus { task_id: String },
    /// 任务取消
    CancelTask { task_id: String, reason: String },
    /// 任务状态报告（执行Agent发送）
    TaskStatusReport {
        task_id: String,
        status: TaskExecutionStatus,
        progress: u32,
        message: Option<String>,
    },
    /// 任务完成报告（执行Agent发送）
    TaskCompleteReport {
        task_id: String,
        result: ExecutionResult,
    },
    /// 请求决策（执行Agent发送）
    RequestDecision {
        task_id: String,
        decision: CriticalDecision,
    },
}

/// 任务执行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskExecutionStatus {
    /// 已接收
    Received,
    /// 准备中
    Preparing,
    /// 执行中
    Executing,
    /// 等待外部响应
    WaitingExternal,
    /// 已完成
    Completed,
    /// 失败
    Failed(String),
}

// =============================================================================
// 默认输入输出类型
// =============================================================================

/// 默认用户输入
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DefaultInput {
    /// 新想法/需求
    Idea {
        content: String,
        priority: Option<TodoPriority>,
        metadata: Option<HashMap<String, String>>,
    },
    /// 决策响应
    Decision {
        decision_id: String,
        selected_option: usize,
        comment: Option<String>,
    },
    /// 查询请求
    Query(QueryType),
    /// 命令
    Command(SecretaryCommand),
}

/// 查询类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryType {
    /// 获取 Todo 列表
    ListTodos { filter: Option<TodoStatus> },
    /// 获取 Todo 详情
    GetTodo { todo_id: String },
    /// 获取统计信息
    Statistics,
    /// 获取待决策列表
    PendingDecisions,
    /// 获取汇报历史
    Reports { report_type: Option<ReportType> },
}

/// 秘书命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretaryCommand {
    /// 开始澄清某个 Todo
    Clarify { todo_id: String },
    /// 分配任务
    Dispatch { todo_id: String },
    /// 取消任务
    Cancel { todo_id: String, reason: String },
    /// 生成汇报
    GenerateReport { report_type: ReportType },
    /// 暂停秘书 Agent
    Pause,
    /// 恢复秘书 Agent
    Resume,
    /// 关闭秘书 Agent
    Shutdown,
}

/// 默认秘书输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DefaultOutput {
    /// 确认消息
    Acknowledgment { message: String },
    /// 需要决策
    DecisionRequired { decision: CriticalDecision },
    /// 汇报
    Report { report: Report },
    /// 状态更新
    StatusUpdate { todo_id: String, status: TodoStatus },
    /// 任务完成通知
    TaskCompleted { todo_id: String, result: ExecutionResult },
    /// 错误消息
    Error { message: String },
    /// 通用消息（LLM生成）
    Message { content: String },
}

/// 秘书 Agent 工作阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkPhase {
    /// 阶段1: 接收想法
    ReceivingIdea,
    /// 阶段2: 澄清需求
    ClarifyingRequirement,
    /// 阶段3: 调度分配
    DispatchingTask,
    /// 阶段4: 监控反馈
    MonitoringExecution,
    /// 阶段5: 验收汇报
    ReportingCompletion,
}
