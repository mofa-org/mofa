//! Agent 能力定义
//! Agent Capability Definitions
//!
//! 定义 Agent 的能力发现和匹配机制
//! Defines mechanisms for Agent capability discovery and matching

use super::types::{InputType, OutputType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 推理策略
/// Reasoning Strategy
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum ReasoningStrategy {
    /// 直接 LLM 推理
    /// Direct LLM reasoning
    #[default]
    Direct,
    /// ReAct 风格的思考-行动-观察循环
    /// ReAct-style Thought-Action-Observation loop
    ReAct {
        /// 最大迭代次数
        /// Maximum number of iterations
        max_iterations: usize,
    },
    /// 思维链推理
    /// Chain of Thought reasoning
    ChainOfThought,
    /// 思维树探索
    /// Tree of Thought exploration
    TreeOfThought {
        /// 分支因子
        /// Branching factor
        branching_factor: usize,
    },
    /// 自定义推理模式
    /// Custom reasoning mode
    Custom(String),
}

/// Agent 能力描述
/// Agent Capability Description
///
/// 用于能力发现和任务路由
/// Used for capability discovery and task routing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// 能力标签 (如 "llm", "coding", "research")
    /// Capability tags (e.g., "llm", "coding", "research")
    pub tags: HashSet<String>,
    /// 支持的输入类型
    /// Supported input types
    pub input_types: HashSet<InputType>,
    /// 支持的输出类型
    /// Supported output types
    pub output_types: HashSet<OutputType>,
    /// 最大上下文长度 (对于 LLM 类 Agent)
    /// Maximum context length (for LLM-based Agents)
    pub max_context_length: Option<usize>,
    /// 支持的推理策略
    /// Supported reasoning strategies
    pub reasoning_strategies: Vec<ReasoningStrategy>,
    /// 是否支持流式输出
    /// Whether streaming output is supported
    pub supports_streaming: bool,
    /// 是否支持多轮对话
    /// Whether multi-turn conversation is supported
    pub supports_conversation: bool,
    /// 是否支持工具调用
    /// Whether tool calling is supported
    pub supports_tools: bool,
    /// 是否支持多 Agent 协调
    /// Whether multi-agent coordination is supported
    pub supports_coordination: bool,
    /// 自定义能力标志
    /// Custom capability flags
    pub custom: HashMap<String, serde_json::Value>,
}

impl AgentCapabilities {
    /// 创建新的能力描述
    /// Create a new capability description
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建构建器
    /// Create a builder
    pub fn builder() -> AgentCapabilitiesBuilder {
        AgentCapabilitiesBuilder::default()
    }

    /// 检查是否有指定标签
    /// Check if a specific tag is present
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// 检查是否支持指定输入类型
    /// Check if a specific input type is supported
    pub fn supports_input(&self, input_type: &InputType) -> bool {
        self.input_types.contains(input_type)
    }

    /// 检查是否支持指定输出类型
    /// Check if a specific output type is supported
    pub fn supports_output(&self, output_type: &OutputType) -> bool {
        self.output_types.contains(output_type)
    }

    /// 检查是否匹配需求
    /// Check if requirements are matched
    pub fn matches(&self, requirements: &AgentRequirements) -> bool {
        // 检查必需标签
        // Check required tags
        if !requirements
            .required_tags
            .iter()
            .all(|t| self.tags.contains(t))
        {
            return false;
        }

        // 检查输入类型
        // Check input types
        if !requirements
            .input_types
            .iter()
            .all(|t| self.input_types.contains(t))
        {
            return false;
        }

        // 检查输出类型
        // Check output types
        if !requirements
            .output_types
            .iter()
            .all(|t| self.output_types.contains(t))
        {
            return false;
        }

        // 检查功能要求
        // Check functional requirements
        if requirements.requires_streaming && !self.supports_streaming {
            return false;
        }
        if requirements.requires_tools && !self.supports_tools {
            return false;
        }
        if requirements.requires_conversation && !self.supports_conversation {
            return false;
        }
        if requirements.requires_coordination && !self.supports_coordination {
            return false;
        }

        true
    }

    /// 计算与需求的匹配分数 (0.0 - 1.0)
    /// Calculate match score with requirements (0.0 - 1.0)
    pub fn match_score(&self, requirements: &AgentRequirements) -> f64 {
        if !self.matches(requirements) {
            return 0.0;
        }

        let mut score = 0.0;
        let mut weight = 0.0;

        // 标签匹配
        // Tag matching
        weight += 1.0;
        if !requirements.required_tags.is_empty() {
            let matched = requirements
                .required_tags
                .iter()
                .filter(|t| self.tags.contains(*t))
                .count();
            score += matched as f64 / requirements.required_tags.len() as f64;
        } else {
            score += 1.0;
        }

        // 优选标签匹配
        // Preferred tag matching
        if !requirements.preferred_tags.is_empty() {
            weight += 0.5;
            let matched = requirements
                .preferred_tags
                .iter()
                .filter(|t| self.tags.contains(*t))
                .count();
            score += 0.5 * (matched as f64 / requirements.preferred_tags.len() as f64);
        }

        // 额外能力加分
        // Extra capability bonus
        if self.supports_streaming {
            score += 0.1;
            weight += 0.1;
        }
        if self.supports_tools {
            score += 0.1;
            weight += 0.1;
        }

        score / weight
    }
}

/// Agent 能力构建器
/// Agent Capability Builder
#[derive(Debug, Default)]
pub struct AgentCapabilitiesBuilder {
    capabilities: AgentCapabilities,
}

impl AgentCapabilitiesBuilder {
    /// 创建新的构建器
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加标签
    /// Add a tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.capabilities.tags.insert(tag.into());
        self
    }

    /// 添加标签 (别名)
    /// Add a tag (alias)
    pub fn with_tag(self, tag: impl Into<String>) -> Self {
        self.tag(tag)
    }

    /// 添加多个标签
    /// Add multiple tags
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for tag in tags {
            self.capabilities.tags.insert(tag.into());
        }
        self
    }

    /// 添加输入类型
    /// Add an input type
    pub fn input_type(mut self, input_type: InputType) -> Self {
        self.capabilities.input_types.insert(input_type);
        self
    }

    /// 添加输入类型 (别名)
    /// Add an input type (alias)
    pub fn with_input_type(self, input_type: InputType) -> Self {
        self.input_type(input_type)
    }

    /// 添加输出类型
    /// Add an output type
    pub fn output_type(mut self, output_type: OutputType) -> Self {
        self.capabilities.output_types.insert(output_type);
        self
    }

    /// 添加输出类型 (别名)
    /// Add an output type (alias)
    pub fn with_output_type(self, output_type: OutputType) -> Self {
        self.output_type(output_type)
    }

    /// 设置最大上下文长度
    /// Set maximum context length
    pub fn max_context_length(mut self, length: usize) -> Self {
        self.capabilities.max_context_length = Some(length);
        self
    }

    /// 添加推理策略
    /// Add a reasoning strategy
    pub fn reasoning_strategy(mut self, strategy: ReasoningStrategy) -> Self {
        self.capabilities.reasoning_strategies.push(strategy);
        self
    }

    /// 添加推理策略 (别名)
    /// Add a reasoning strategy (alias)
    pub fn with_reasoning_strategy(self, strategy: ReasoningStrategy) -> Self {
        self.reasoning_strategy(strategy)
    }

    /// 设置流式输出支持
    /// Set streaming output support
    pub fn supports_streaming(mut self, supports: bool) -> Self {
        self.capabilities.supports_streaming = supports;
        self
    }

    /// 设置多轮对话支持
    /// Set multi-turn conversation support
    pub fn supports_conversation(mut self, supports: bool) -> Self {
        self.capabilities.supports_conversation = supports;
        self
    }

    /// 设置工具调用支持
    /// Set tool calling support
    pub fn supports_tools(mut self, supports: bool) -> Self {
        self.capabilities.supports_tools = supports;
        self
    }

    /// 设置多 Agent 协调支持
    /// Set multi-agent coordination support
    pub fn supports_coordination(mut self, supports: bool) -> Self {
        self.capabilities.supports_coordination = supports;
        self
    }

    /// 添加自定义能力
    /// Add a custom capability
    pub fn custom(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.capabilities.custom.insert(key.into(), value);
        self
    }

    /// 构建能力描述
    /// Build the capability description
    #[must_use]
    pub fn build(self) -> AgentCapabilities {
        self.capabilities
    }
}

/// Agent 需求描述
/// Agent Requirements Description
///
/// 用于查找满足特定需求的 Agent
/// Used to find Agents that meet specific requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRequirements {
    /// 必需的标签
    /// Required tags
    pub required_tags: HashSet<String>,
    /// 优选的标签 (用于排序)
    /// Preferred tags (used for sorting)
    pub preferred_tags: HashSet<String>,
    /// 必需的输入类型
    /// Required input types
    pub input_types: HashSet<InputType>,
    /// 必需的输出类型
    /// Required output types
    pub output_types: HashSet<OutputType>,
    /// 是否需要流式输出
    /// Whether streaming output is required
    pub requires_streaming: bool,
    /// 是否需要工具支持
    /// Whether tool support is required
    pub requires_tools: bool,
    /// 是否需要多轮对话
    /// Whether multi-turn conversation is required
    pub requires_conversation: bool,
    /// 是否需要多 Agent 协调
    /// Whether multi-agent coordination is required
    pub requires_coordination: bool,
}

impl AgentRequirements {
    /// 创建新的需求描述
    /// Create a new requirements description
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建构建器
    /// Create a builder
    pub fn builder() -> AgentRequirementsBuilder {
        AgentRequirementsBuilder::default()
    }

    /// 检查给定的能力是否满足需求
    /// Check if given capabilities meet requirements
    pub fn matches(&self, capabilities: &AgentCapabilities) -> bool {
        // 检查必需标签
        // Check required tags
        for tag in &self.required_tags {
            if !capabilities.tags.contains(tag) {
                return false;
            }
        }

        // 检查输入类型
        // Check input types
        for input_type in &self.input_types {
            if !capabilities.input_types.contains(input_type) {
                return false;
            }
        }

        // 检查输出类型
        // Check output types
        for output_type in &self.output_types {
            if !capabilities.output_types.contains(output_type) {
                return false;
            }
        }

        // 检查流式输出
        // Check streaming output
        if self.requires_streaming && !capabilities.supports_streaming {
            return false;
        }

        // 检查工具支持
        // Check tool support
        if self.requires_tools && !capabilities.supports_tools {
            return false;
        }

        // 检查多轮对话
        // Check multi-turn conversation
        if self.requires_conversation && !capabilities.supports_conversation {
            return false;
        }

        // 检查多 Agent 协调
        // Check multi-agent coordination
        if self.requires_coordination && !capabilities.supports_coordination {
            return false;
        }

        true
    }

    /// 计算匹配分数 (用于排序)
    /// Calculate match score (used for sorting)
    pub fn score(&self, capabilities: &AgentCapabilities) -> f32 {
        if !self.matches(capabilities) {
            return 0.0;
        }

        let mut score = 1.0;

        // 优选标签匹配加分
        // Bonus for matching preferred tags
        let preferred_count = self
            .preferred_tags
            .iter()
            .filter(|tag| capabilities.tags.contains(*tag))
            .count();

        if !self.preferred_tags.is_empty() {
            score += (preferred_count as f32) / (self.preferred_tags.len() as f32);
        }

        score
    }
}

/// Agent 需求构建器
/// Agent Requirements Builder
#[derive(Debug, Default)]
pub struct AgentRequirementsBuilder {
    requirements: AgentRequirements,
}

impl AgentRequirementsBuilder {
    /// 创建新的构建器
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加必需标签
    /// Add a required tag
    pub fn require_tag(mut self, tag: impl Into<String>) -> Self {
        self.requirements.required_tags.insert(tag.into());
        self
    }

    /// 添加优选标签
    /// Add a preferred tag
    pub fn prefer_tag(mut self, tag: impl Into<String>) -> Self {
        self.requirements.preferred_tags.insert(tag.into());
        self
    }

    /// 添加输入类型需求
    /// Add an input type requirement
    pub fn require_input(mut self, input_type: InputType) -> Self {
        self.requirements.input_types.insert(input_type);
        self
    }

    /// 添加输出类型需求
    /// Add an output type requirement
    pub fn require_output(mut self, output_type: OutputType) -> Self {
        self.requirements.output_types.insert(output_type);
        self
    }

    /// 要求流式输出
    /// Require streaming output
    pub fn require_streaming(mut self) -> Self {
        self.requirements.requires_streaming = true;
        self
    }

    /// 要求工具支持
    /// Require tool support
    pub fn require_tools(mut self) -> Self {
        self.requirements.requires_tools = true;
        self
    }

    /// 要求多轮对话
    /// Require multi-turn conversation
    pub fn require_conversation(mut self) -> Self {
        self.requirements.requires_conversation = true;
        self
    }

    /// 要求多 Agent 协调
    /// Require multi-agent coordination
    pub fn require_coordination(mut self) -> Self {
        self.requirements.requires_coordination = true;
        self
    }

    /// 构建需求描述
    /// Build the requirements description
    #[must_use]
    pub fn build(self) -> AgentRequirements {
        self.requirements
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_builder() {
        /// 测试能力构建器
        /// Test the capabilities builder
        let caps = AgentCapabilities::builder()
            .tag("llm")
            .tag("coding")
            .input_type(InputType::Text)
            .output_type(OutputType::Text)
            .supports_streaming(true)
            .supports_tools(true)
            .build();

        assert!(caps.has_tag("llm"));
        assert!(caps.has_tag("coding"));
        assert!(caps.supports_input(&InputType::Text));
        assert!(caps.supports_streaming);
        assert!(caps.supports_tools);
    }

    #[test]
    fn test_capabilities_matching() {
        /// 测试能力匹配
        /// Test capability matching
        let caps = AgentCapabilities::builder()
            .tag("llm")
            .tag("coding")
            .input_type(InputType::Text)
            .output_type(OutputType::Text)
            .supports_tools(true)
            .build();

        let requirements = AgentRequirements::builder()
            .require_tag("llm")
            .require_input(InputType::Text)
            .require_tools()
            .build();

        assert!(caps.matches(&requirements));
    }

    #[test]
    fn test_capabilities_mismatch() {
        /// 测试能力不匹配
        /// Test capability mismatch
        let caps = AgentCapabilities::builder()
            .tag("llm")
            .input_type(InputType::Text)
            .build();

        let requirements = AgentRequirements::builder()
            .require_tag("coding") // Not present
            .build();

        assert!(!caps.matches(&requirements));
    }

    #[test]
    fn test_match_score() {
        /// 测试匹配分数
        /// Test match score
        let caps = AgentCapabilities::builder()
            .tag("llm")
            .tag("coding")
            .tag("research")
            .supports_streaming(true)
            .supports_tools(true)
            .build();

        let requirements = AgentRequirements::builder()
            .require_tag("llm")
            .prefer_tag("coding")
            .prefer_tag("research")
            .build();

        let score = caps.match_score(&requirements);
        assert!(score > 0.8);
    }
}
