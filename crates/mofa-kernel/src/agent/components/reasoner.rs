//! 推理组件
//!
//! 定义 Agent 的推理/思考能力

use crate::agent::capabilities::ReasoningStrategy;
use crate::agent::context::CoreAgentContext;
use crate::agent::error::AgentResult;
use crate::agent::types::AgentInput;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 推理器 Trait
///
/// 负责 Agent 的推理/思考过程
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::components::reasoner::{Reasoner, ReasoningResult};
/// use mofa_foundation::agent::components::reasoner::DirectReasoner;
///
/// // 使用 foundation 层提供的具体实现
/// let reasoner = DirectReasoner;
/// // 或者实现自定义 Reasoner
/// struct MyReasoner;
///
/// #[async_trait]
/// impl Reasoner for MyReasoner {
///     async fn reason(&self, input: &AgentInput, ctx: &CoreAgentContext) -> AgentResult<ReasoningResult> {
///         Ok(ReasoningResult {
///             thoughts: vec![],
///             decision: Decision::Respond { content: input.to_text() },
///             confidence: 1.0,
///         })
///     }
///
///     fn strategy(&self) -> ReasoningStrategy {
///         ReasoningStrategy::Direct
///     }
/// }
/// ```
#[async_trait]
pub trait Reasoner: Send + Sync {
    /// 执行推理过程
    async fn reason(&self, input: &AgentInput, ctx: &CoreAgentContext) -> AgentResult<ReasoningResult>;

    /// 获取推理策略
    fn strategy(&self) -> ReasoningStrategy;

    /// 是否支持多步推理
    fn supports_multi_step(&self) -> bool {
        matches!(
            self.strategy(),
            ReasoningStrategy::ReAct { .. }
                | ReasoningStrategy::ChainOfThought
                | ReasoningStrategy::TreeOfThought { .. }
        )
    }

    /// 推理器名称
    fn name(&self) -> &str {
        "reasoner"
    }

    /// 推理器描述
    fn description(&self) -> Option<&str> {
        None
    }
}

/// 推理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningResult {
    /// 思考步骤
    pub thoughts: Vec<ThoughtStep>,
    /// 最终决策
    pub decision: Decision,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
}

impl ReasoningResult {
    /// 创建简单的响应结果
    pub fn respond(content: impl Into<String>) -> Self {
        Self {
            thoughts: vec![],
            decision: Decision::Respond {
                content: content.into(),
            },
            confidence: 1.0,
        }
    }

    /// 创建工具调用结果
    pub fn call_tool(
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            thoughts: vec![],
            decision: Decision::CallTool {
                tool_name: tool_name.into(),
                arguments,
            },
            confidence: 1.0,
        }
    }

    /// 添加思考步骤
    pub fn with_thought(mut self, step: ThoughtStep) -> Self {
        self.thoughts.push(step);
        self
    }

    /// 设置置信度
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// 思考步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtStep {
    /// 步骤类型
    pub step_type: ThoughtStepType,
    /// 步骤内容
    pub content: String,
    /// 步骤序号
    pub step_number: usize,
    /// 时间戳 (毫秒)
    pub timestamp_ms: u64,
}

impl ThoughtStep {
    /// 创建新的思考步骤
    pub fn new(step_type: ThoughtStepType, content: impl Into<String>, step_number: usize) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            step_type,
            content: content.into(),
            step_number,
            timestamp_ms: now,
        }
    }

    /// 创建思考步骤
    pub fn thought(content: impl Into<String>, step_number: usize) -> Self {
        Self::new(ThoughtStepType::Thought, content, step_number)
    }

    /// 创建分析步骤
    pub fn analysis(content: impl Into<String>, step_number: usize) -> Self {
        Self::new(ThoughtStepType::Analysis, content, step_number)
    }

    /// 创建规划步骤
    pub fn planning(content: impl Into<String>, step_number: usize) -> Self {
        Self::new(ThoughtStepType::Planning, content, step_number)
    }

    /// 创建反思步骤
    pub fn reflection(content: impl Into<String>, step_number: usize) -> Self {
        Self::new(ThoughtStepType::Reflection, content, step_number)
    }
}

/// 思考步骤类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThoughtStepType {
    /// 思考
    Thought,
    /// 分析
    Analysis,
    /// 规划
    Planning,
    /// 反思
    Reflection,
    /// 评估
    Evaluation,
    /// 自定义
    Custom(String),
}

/// 决策类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    /// 直接响应
    Respond {
        /// 响应内容
        content: String,
    },
    /// 调用工具
    CallTool {
        /// 工具名称
        tool_name: String,
        /// 工具参数
        arguments: serde_json::Value,
    },
    /// 调用多个工具 (并行)
    CallMultipleTools {
        /// 工具调用列表
        tool_calls: Vec<ToolCall>,
    },
    /// 委托给其他 Agent
    Delegate {
        /// 目标 Agent ID
        agent_id: String,
        /// 委托任务
        task: String,
    },
    /// 需要更多信息
    NeedMoreInfo {
        /// 需要的信息
        questions: Vec<String>,
    },
    /// 无法处理
    CannotHandle {
        /// 原因
        reason: String,
    },
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具名称
    pub tool_name: String,
    /// 工具参数
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// 创建新的工具调用
    pub fn new(tool_name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            tool_name: tool_name.into(),
            arguments,
        }
    }
}

// Note: Concrete Reasoner implementations (like DirectReasoner) are provided
// in the foundation layer (mofa-foundation::agent::components::reasoner)
