//! 配置 Schema 定义
//! Configuration Schema Definitions
//!
//! 定义 Agent 的配置结构
//! Defines the configuration structure for Agents

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 主配置结构
// Main Configuration Structures
// ============================================================================

/// Agent 配置
/// Agent Configuration
///
/// 统一的 Agent 配置结构，支持多种 Agent 类型
/// Unified Agent configuration structure supporting multiple Agent types
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_runtime::agent::config::{AgentConfig, AgentType, LlmAgentConfig};
///
/// let config = AgentConfig {
///     id: "my-agent".to_string(),
///     name: "My LLM Agent".to_string(),
///     description: Some("A helpful assistant".to_string()),
///     agent_type: AgentType::Llm(LlmAgentConfig {
///         model: "gpt-4".to_string(),
///         ..Default::default()
///     }),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent ID (唯一标识符)
    /// Agent ID (Unique Identifier)
    pub id: String,

    /// Agent 名称 (显示名)
    /// Agent Name (Display Name)
    pub name: String,

    /// Agent 描述
    /// Agent Description
    #[serde(default)]
    pub description: Option<String>,

    /// Agent 类型配置
    /// Agent Type Configuration
    #[serde(flatten)]
    pub agent_type: AgentType,

    /// 组件配置
    /// Components Configuration
    #[serde(default)]
    pub components: ComponentsConfig,

    /// 能力配置
    /// Capabilities Configuration
    #[serde(default)]
    pub capabilities: CapabilitiesConfig,

    /// 自定义配置
    /// Custom Configuration
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,

    /// 环境变量映射
    /// Environment Variable Mappings
    #[serde(default)]
    pub env_mappings: HashMap<String, String>,

    /// 是否启用
    /// Whether Enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// 版本号
    /// Version Number
    #[serde(default)]
    pub version: Option<String>,
}

fn default_enabled() -> bool {
    true
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: None,
            agent_type: AgentType::default(),
            components: ComponentsConfig::default(),
            capabilities: CapabilitiesConfig::default(),
            custom: HashMap::new(),
            env_mappings: HashMap::new(),
            enabled: true,
            version: None,
        }
    }
}

impl AgentConfig {
    /// 创建新配置
    /// Create New Configuration
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Default::default()
        }
    }

    /// 设置描述
    /// Set Description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置 Agent 类型
    /// Set Agent Type
    pub fn with_type(mut self, agent_type: AgentType) -> Self {
        self.agent_type = agent_type;
        self
    }

    /// 添加自定义配置
    /// Add Custom Configuration
    pub fn with_custom(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom.insert(key.into(), value);
        self
    }

    /// 获取自定义配置
    /// Get Custom Configuration
    pub fn get_custom<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.custom
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// 验证配置
    /// Validate Configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.id.is_empty() {
            errors.push("Agent ID cannot be empty".to_string());
        }

        if self.name.is_empty() {
            errors.push("Agent name cannot be empty".to_string());
        }

        // 验证类型特定配置
        // Validate type-specific configurations
        if let Err(type_errors) = self.agent_type.validate() {
            errors.extend(type_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ============================================================================
// Agent 类型
// Agent Types
// ============================================================================

/// Agent 类型
/// Agent Type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentType {
    /// LLM Agent
    /// LLM Agent
    Llm(LlmAgentConfig),

    /// ReAct Agent
    /// ReAct Agent
    #[serde(rename = "react")]
    ReAct(ReActAgentConfig),

    /// 工作流 Agent
    /// Workflow Agent
    Workflow(WorkflowAgentConfig),

    /// 团队 Agent
    /// Team Agent
    Team(TeamAgentConfig),

    /// 自定义 Agent
    /// Custom Agent
    Custom {
        /// 类路径或插件标识
        /// Class path or plugin identifier
        class_path: String,
        /// 自定义配置
        /// Custom configuration
        #[serde(default)]
        config: HashMap<String, serde_json::Value>,
    },
}

impl Default for AgentType {
    fn default() -> Self {
        Self::Llm(LlmAgentConfig::default())
    }
}

impl AgentType {
    /// 获取类型名称
    /// Get Type Name
    pub fn type_name(&self) -> &str {
        match self {
            Self::Llm(_) => "llm",
            Self::ReAct(_) => "react",
            Self::Workflow(_) => "workflow",
            Self::Team(_) => "team",
            Self::Custom { .. } => "custom",
        }
    }

    /// 验证类型配置
    /// Validate Type Configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        match self {
            Self::Llm(config) => config.validate(),
            Self::ReAct(config) => config.validate(),
            Self::Workflow(config) => config.validate(),
            Self::Team(config) => config.validate(),
            Self::Custom { class_path, .. } => {
                if class_path.is_empty() {
                    Err(vec!["Custom agent class_path cannot be empty".to_string()])
                } else {
                    Ok(())
                }
            }
        }
    }
}

// ============================================================================
// LLM Agent 配置
// LLM Agent Configuration
// ============================================================================

/// LLM Agent 配置
/// LLM Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAgentConfig {
    /// 模型名称
    /// Model Name
    pub model: String,

    /// 系统提示词
    /// System Prompt
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// 温度参数
    /// Temperature Parameter
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// 最大 token 数
    /// Maximum Token Count
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Top P 参数
    /// Top P Parameter
    #[serde(default)]
    pub top_p: Option<f32>,

    /// 停止序列
    /// Stop Sequences
    #[serde(default)]
    pub stop_sequences: Vec<String>,

    /// 是否启用流式输出
    /// Whether Streaming is Enabled
    #[serde(default)]
    pub streaming: bool,

    /// API Key 环境变量名
    /// API Key Env Var Name
    #[serde(default)]
    pub api_key_env: Option<String>,

    /// API Base URL
    /// API Base URL
    #[serde(default)]
    pub base_url: Option<String>,

    /// 额外参数
    /// Extra Parameters
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_temperature() -> f32 {
    0.7
}

impl Default for LlmAgentConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4".to_string(),
            system_prompt: None,
            temperature: 0.7,
            max_tokens: None,
            top_p: None,
            stop_sequences: Vec::new(),
            streaming: false,
            api_key_env: None,
            base_url: None,
            extra: HashMap::new(),
        }
    }
}

impl LlmAgentConfig {
    /// 验证配置
    /// Validate Configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.model.is_empty() {
            errors.push("LLM model cannot be empty".to_string());
        }

        if self.temperature < 0.0 || self.temperature > 2.0 {
            errors.push("Temperature must be between 0.0 and 2.0".to_string());
        }

        if let Some(top_p) = self.top_p
            && (!(0.0..=1.0).contains(&top_p))
        {
            errors.push("Top P must be between 0.0 and 1.0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ============================================================================
// ReAct Agent 配置
// ReAct Agent Configuration
// ============================================================================

/// ReAct Agent 配置
/// ReAct Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReActAgentConfig {
    /// LLM 配置
    /// LLM Configuration
    pub llm: LlmAgentConfig,

    /// 最大推理步数
    /// Maximum Inference Steps
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,

    /// 工具配置
    /// Tools Configuration
    #[serde(default)]
    pub tools: Vec<ToolConfig>,

    /// 是否启用并行工具调用
    /// Whether Parallel Tool Calling is Enabled
    #[serde(default)]
    pub parallel_tool_calls: bool,

    /// 思考格式
    /// Thought Format
    #[serde(default)]
    pub thought_format: Option<String>,
}

fn default_max_steps() -> usize {
    10
}

impl Default for ReActAgentConfig {
    fn default() -> Self {
        Self {
            llm: LlmAgentConfig::default(),
            max_steps: 10,
            tools: Vec::new(),
            parallel_tool_calls: false,
            thought_format: None,
        }
    }
}

impl ReActAgentConfig {
    /// 验证配置
    /// Validate Configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Err(llm_errors) = self.llm.validate() {
            errors.extend(llm_errors);
        }

        if self.max_steps == 0 {
            errors.push("ReAct max_steps must be greater than 0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// 工具配置
/// Tool Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// 工具名称
    /// Tool Name
    pub name: String,

    /// 工具类型
    /// Tool Type
    #[serde(default)]
    pub tool_type: ToolType,

    /// 工具配置
    /// Tool Configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,

    /// 是否启用
    /// Whether Enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// 工具类型
/// Tool Type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    /// 内置工具
    /// Builtin Tool
    #[default]
    Builtin,
    /// MCP 工具
    /// MCP Tool
    Mcp,
    /// 自定义工具
    /// Custom Tool
    Custom,
    /// 插件工具
    /// Plugin Tool
    Plugin,
}

// ============================================================================
// Workflow Agent 配置
// Workflow Agent Configuration
// ============================================================================

/// Workflow Agent 配置
/// Workflow Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowAgentConfig {
    /// 工作流步骤
    /// Workflow Steps
    pub steps: Vec<WorkflowStep>,

    /// 是否启用并行执行
    /// Whether Parallel Execution is Enabled
    #[serde(default)]
    pub parallel: bool,

    /// 错误处理策略
    /// Error Handling Strategy
    #[serde(default)]
    pub error_strategy: ErrorStrategy,
}

impl WorkflowAgentConfig {
    /// 验证配置
    /// Validate Configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.steps.is_empty() {
            errors.push("Workflow steps cannot be empty".to_string());
        }

        for (i, step) in self.steps.iter().enumerate() {
            if step.agent_id.is_empty() {
                errors.push(format!("Workflow step {} agent_id cannot be empty", i));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// 工作流步骤
/// Workflow Step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// 步骤 ID
    /// Step ID
    pub id: String,

    /// Agent ID
    /// Agent ID
    pub agent_id: String,

    /// 输入映射
    /// Input Mapping
    #[serde(default)]
    pub input_mapping: HashMap<String, String>,

    /// 输出映射
    /// Output Mapping
    #[serde(default)]
    pub output_mapping: HashMap<String, String>,

    /// 条件表达式
    /// Condition Expression
    #[serde(default)]
    pub condition: Option<String>,

    /// 超时 (毫秒)
    /// Timeout (Milliseconds)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// 错误处理策略
/// Error Handling Strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStrategy {
    /// 快速失败
    /// Fail Fast
    #[default]
    FailFast,
    /// 继续执行
    /// Continue Execution
    Continue,
    /// 重试
    /// Retry
    Retry { max_retries: usize, delay_ms: u64 },
    /// 回退
    /// Fallback
    Fallback { fallback_agent_id: String },
}

// ============================================================================
// Team Agent 配置
// Team Agent Configuration
// ============================================================================

/// Team Agent 配置
/// Team Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamAgentConfig {
    /// 团队成员
    /// Team Members
    pub members: Vec<TeamMember>,

    /// 协调模式
    /// Coordination Mode
    #[serde(default)]
    pub coordination: CoordinationMode,

    /// 领导者 Agent ID (用于 Hierarchical 模式)
    /// Leader Agent ID (for Hierarchical mode)
    #[serde(default)]
    pub leader_id: Option<String>,

    /// 任务分发策略
    /// Task Dispatch Strategy
    #[serde(default)]
    pub dispatch_strategy: DispatchStrategy,
}

impl TeamAgentConfig {
    /// 验证配置
    /// Validate Configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.members.is_empty() {
            errors.push("Team members cannot be empty".to_string());
        }

        if matches!(self.coordination, CoordinationMode::Hierarchical) && self.leader_id.is_none() {
            errors.push("Hierarchical coordination requires leader_id".to_string());
        }

        for member in &self.members {
            if member.agent_id.is_empty() {
                errors.push("Team member agent_id cannot be empty".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// 团队成员
/// Team Member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// Agent ID
    /// Agent ID
    pub agent_id: String,

    /// 角色
    /// Role
    #[serde(default)]
    pub role: Option<String>,

    /// 权重 (用于负载均衡)
    /// Weight (for Load Balancing)
    #[serde(default = "default_weight")]
    pub weight: f32,

    /// 是否为可选成员
    /// Whether Optional Member
    #[serde(default)]
    pub optional: bool,
}

fn default_weight() -> f32 {
    1.0
}

/// 协调模式
/// Coordination Mode
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationMode {
    /// 顺序执行
    /// Sequential Execution
    #[default]
    Sequential,
    /// 并行执行
    /// Parallel Execution
    Parallel,
    /// 层级执行
    /// Hierarchical Execution
    Hierarchical,
    /// 共识模式
    /// Consensus Mode
    Consensus,
    /// 投票模式
    /// Voting Mode
    Voting,
    /// 辩论模式
    /// Debate Mode
    Debate,
}

/// 任务分发策略
/// Task Dispatch Strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStrategy {
    /// 广播 (所有成员)
    /// Broadcast (All Members)
    #[default]
    Broadcast,
    /// 轮询
    /// Round Robin
    RoundRobin,
    /// 随机
    /// Random
    Random,
    /// 负载均衡
    /// Load Balanced
    LoadBalanced,
    /// 按能力匹配
    /// Capability Based
    CapabilityBased,
}

// ============================================================================
// 组件配置
// Component Configuration
// ============================================================================

/// 组件配置
/// Components Configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentsConfig {
    /// 推理器配置
    /// Reasoner Configuration
    #[serde(default)]
    pub reasoner: Option<ReasonerConfig>,

    /// 记忆配置
    /// Memory Configuration
    #[serde(default)]
    pub memory: Option<MemoryConfig>,

    /// 协调器配置
    /// Coordinator Configuration
    #[serde(default)]
    pub coordinator: Option<CoordinatorConfig>,
}

/// 推理器配置
/// Reasoner Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonerConfig {
    /// 推理策略
    /// Reasoning Strategy
    #[serde(default)]
    pub strategy: ReasonerStrategy,

    /// 自定义配置
    /// Custom Configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

/// 推理策略
/// Reasoning Strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonerStrategy {
    #[default]
    Direct,
    ChainOfThought,
    TreeOfThought,
    ReAct,
    Custom,
}

/// 记忆配置
/// Memory Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 记忆类型
    /// Memory Type
    #[serde(default)]
    pub memory_type: MemoryType,

    /// 最大记忆项数
    /// Maximum Memory Items
    #[serde(default)]
    pub max_items: Option<usize>,

    /// 向量数据库配置
    /// Vector Database Configuration
    #[serde(default)]
    pub vector_db: Option<VectorDbConfig>,
}

/// 记忆类型
/// Memory Type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    #[default]
    InMemory,
    Redis,
    Sqlite,
    VectorDb,
    Custom,
}

/// 向量数据库配置
/// Vector Database Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDbConfig {
    /// 数据库类型
    /// Database Type
    pub db_type: String,
    /// 连接 URL
    /// Connection URL
    pub url: String,
    /// 集合/索引名称
    /// Collection/Index Name
    #[serde(default)]
    pub collection: Option<String>,
}

/// 协调器配置
/// Coordinator Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// 协调模式
    /// Coordination Pattern
    #[serde(default)]
    pub pattern: CoordinationMode,

    /// 超时 (毫秒)
    /// Timeout (Milliseconds)
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// 自定义配置
    /// Custom Configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

// ============================================================================
// 能力配置
// Capabilities Configuration
// ============================================================================

/// 能力配置
/// Capabilities Configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitiesConfig {
    /// 标签
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// 支持的输入类型
    /// Supported Input Types
    #[serde(default)]
    pub input_types: Vec<String>,

    /// 支持的输出类型
    /// Supported Output Types
    #[serde(default)]
    pub output_types: Vec<String>,

    /// 是否支持流式输出
    /// Whether Streaming Output is Supported
    #[serde(default)]
    pub supports_streaming: bool,

    /// 是否支持工具调用
    /// Whether Tool Calling is Supported
    #[serde(default)]
    pub supports_tools: bool,

    /// 是否支持多 Agent 协调
    /// Whether Multi-Agent Coordination is Supported
    #[serde(default)]
    pub supports_coordination: bool,

    /// 推理策略
    /// Reasoning Strategies
    #[serde(default)]
    pub reasoning_strategies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_validation() {
        // 验证 Agent 配置
        // Validate Agent configuration
        let config = AgentConfig::new("test-agent", "Test Agent")
            .with_type(AgentType::Llm(LlmAgentConfig::default()));

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_empty_config_validation() {
        // 验证空配置
        // Validate empty configuration
        let config = AgentConfig::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_llm_config_serialization() {
        // 验证 LLM 配置序列化
        // Validate LLM config serialization
        let config = AgentConfig {
            id: "llm-agent".to_string(),
            name: "LLM Agent".to_string(),
            agent_type: AgentType::Llm(LlmAgentConfig {
                model: "gpt-4".to_string(),
                temperature: 0.8,
                ..Default::default()
            }),
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("0.8"));
    }

    #[test]
    fn test_react_config_serialization() {
        // 验证 ReAct 配置序列化
        // Validate ReAct config serialization
        let config = AgentConfig {
            id: "react-agent".to_string(),
            name: "ReAct Agent".to_string(),
            agent_type: AgentType::ReAct(ReActAgentConfig {
                max_steps: 15,
                ..Default::default()
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("react"));
        assert!(json.contains("15"));
    }

    #[test]
    fn test_team_config_validation() {
        // 验证团队配置
        // Validate team configuration
        let config = TeamAgentConfig {
            members: vec![TeamMember {
                agent_id: "agent-1".to_string(),
                role: Some("worker".to_string()),
                weight: 1.0,
                optional: false,
            }],
            coordination: CoordinationMode::Hierarchical,
            leader_id: None, // Missing leader
            dispatch_strategy: DispatchStrategy::Broadcast,
        };

        assert!(config.validate().is_err());
    }
}
