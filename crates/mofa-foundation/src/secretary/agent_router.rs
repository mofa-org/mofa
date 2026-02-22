//! Agent路由器 - 阶段3扩展: 动态Agent注入与智能决策
//!
//! 支持SDK调用者自行决定注入哪些Agent，框架通过提示词或规则引擎进行动态决策。
//!
//! ## 核心特性
//!
//! 1. **AgentProvider trait**: SDK调用者实现此trait来动态提供Agent
//! 2. **AgentRouter trait**: 动态决策使用哪个Agent执行任务
//! 3. **LLMAgentRouter**: 基于LLM提示词的智能路由
//! 4. **RuleBasedRouter**: 基于规则引擎的确定性路由
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! // 1. 实现AgentProvider提供动态Agent
//! struct MyAgentProvider {
//!     agents: Vec<AgentInfo>,
//! }
//!
//! impl AgentProvider for MyAgentProvider {
//!     async fn list_agents(&self) -> Vec<AgentInfo> {
//!         self.agents.clone()
//!     }
//!
//!     async fn get_agent(&self, id: &str) -> Option<AgentInfo> {
//!         self.agents.iter().find(|a| a.id == id).cloned()
//!     }
//! }
//!
//! // 2. 配置路由策略
//! let router = LLMAgentRouter::new(llm_provider)
//!     .with_custom_prompt(my_prompt);
//!
//! // 3. 在SecretaryAgent中使用
//! let secretary = DefaultSecretaryBuilder::new()
//!     .with_agent_provider(Arc::new(my_provider))
//!     .with_agent_router(Arc::new(router))
//!     .build()
//!     .await;
//! ```

use super::default::types::{ProjectRequirement, Subtask};
use super::llm::{ChatMessage, LLMProvider, parse_llm_json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// Agent信息与能力描述
// =============================================================================

/// Agent信息 - 描述一个可执行任务的Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent唯一标识
    pub id: String,
    /// Agent名称
    pub name: String,
    /// Agent描述（用于LLM理解Agent能力）
    pub description: String,
    /// 能力标签列表
    pub capabilities: Vec<String>,
    /// 支持的任务类型
    pub supported_task_types: Vec<String>,
    /// Agent的prompt模板（可选，用于调用时）
    pub prompt_template: Option<String>,
    /// 当前负载（0-100）
    pub current_load: u32,
    /// 是否可用
    pub available: bool,
    /// 性能评分（历史表现）
    pub performance_score: f32,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl AgentInfo {
    /// 创建新的AgentInfo
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            capabilities: Vec::new(),
            supported_task_types: Vec::new(),
            prompt_template: None,
            current_load: 0,
            available: true,
            performance_score: 0.8,
            metadata: HashMap::new(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// 添加能力
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// 批量添加能力
    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.capabilities.extend(caps);
        self
    }

    /// 添加支持的任务类型
    pub fn with_task_type(mut self, task_type: impl Into<String>) -> Self {
        self.supported_task_types.push(task_type.into());
        self
    }

    /// 设置prompt模板
    pub fn with_prompt_template(mut self, template: impl Into<String>) -> Self {
        self.prompt_template = Some(template.into());
        self
    }

    /// 设置性能评分
    pub fn with_performance_score(mut self, score: f32) -> Self {
        self.performance_score = score;
        self
    }
}

// =============================================================================
// AgentProvider Trait - 动态Agent注入
// =============================================================================

/// Agent提供者Trait
///
/// SDK调用者实现此trait来动态提供可用的Agent列表。
/// 这允许在运行时动态添加、移除、更新Agent。
#[async_trait::async_trait]
pub trait AgentProvider: Send + Sync {
    /// 获取所有可用的Agent列表
    async fn list_agents(&self) -> Vec<AgentInfo>;

    /// 根据ID获取特定Agent
    async fn get_agent(&self, agent_id: &str) -> Option<AgentInfo>;

    /// 根据能力标签筛选Agent
    async fn filter_by_capabilities(&self, capabilities: &[String]) -> Vec<AgentInfo> {
        let agents = self.list_agents().await;
        agents
            .into_iter()
            .filter(|agent| {
                capabilities.is_empty()
                    || capabilities
                        .iter()
                        .any(|cap| agent.capabilities.contains(cap))
            })
            .collect()
    }

    /// 根据任务类型筛选Agent
    async fn filter_by_task_type(&self, task_type: &str) -> Vec<AgentInfo> {
        let agents = self.list_agents().await;
        agents
            .into_iter()
            .filter(|agent| {
                agent.supported_task_types.is_empty()
                    || agent.supported_task_types.contains(&task_type.to_string())
            })
            .collect()
    }

    /// 更新Agent状态
    async fn update_agent_status(&self, agent_id: &str, load: u32, available: bool);

    /// 注册新Agent（可选实现）
    async fn register_agent(&self, _agent: AgentInfo) -> anyhow::Result<()> {
        Ok(())
    }

    /// 注销Agent（可选实现）
    async fn unregister_agent(&self, _agent_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

// =============================================================================
// 默认AgentProvider实现
// =============================================================================

/// 内存中的Agent提供者
///
/// 简单的基于内存的AgentProvider实现
#[derive(Clone)]
pub struct InMemoryAgentProvider {
    agents: Arc<RwLock<HashMap<String, AgentInfo>>>,
}

impl InMemoryAgentProvider {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加Agent
    pub async fn add_agent(&self, agent: AgentInfo) {
        let mut agents = self.agents.write().await;
        agents.insert(agent.id.clone(), agent);
    }

    /// 移除Agent
    pub async fn remove_agent(&self, agent_id: &str) {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id);
    }
}

impl Default for InMemoryAgentProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentProvider for InMemoryAgentProvider {
    async fn list_agents(&self) -> Vec<AgentInfo> {
        let agents = self.agents.read().await;
        agents.values().filter(|a| a.available).cloned().collect()
    }

    async fn get_agent(&self, agent_id: &str) -> Option<AgentInfo> {
        let agents = self.agents.read().await;
        agents.get(agent_id).cloned()
    }

    async fn update_agent_status(&self, agent_id: &str, load: u32, available: bool) {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.current_load = load;
            agent.available = available;
        }
    }

    async fn register_agent(&self, agent: AgentInfo) -> anyhow::Result<()> {
        self.add_agent(agent).await;
        Ok(())
    }

    async fn unregister_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        self.remove_agent(agent_id).await;
        Ok(())
    }
}

// =============================================================================
// 路由决策上下文与结果
// =============================================================================

/// 路由决策上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingContext {
    /// 子任务信息
    pub subtask: Subtask,
    /// Todo ID
    pub todo_id: String,
    /// 原始需求
    pub raw_idea: String,
    /// 项目需求（如果已澄清）
    pub requirement: Option<ProjectRequirement>,
    /// 上下文元数据
    pub metadata: HashMap<String, String>,
    /// 对话历史（用于LLM路由）
    pub conversation_history: Vec<String>,
}

impl RoutingContext {
    pub fn new(subtask: Subtask, todo_id: &str) -> Self {
        Self {
            subtask,
            todo_id: todo_id.to_string(),
            raw_idea: String::new(),
            requirement: None,
            metadata: HashMap::new(),
            conversation_history: Vec::new(),
        }
    }

    pub fn with_raw_idea(mut self, idea: &str) -> Self {
        self.raw_idea = idea.to_string();
        self
    }

    pub fn with_requirement(mut self, req: ProjectRequirement) -> Self {
        self.requirement = Some(req);
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// 路由决策结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// 选中的Agent ID
    pub agent_id: String,
    /// 决策理由
    pub reason: String,
    /// 匹配分数
    pub confidence: f32,
    /// 备选Agent列表（按优先级排序）
    pub alternatives: Vec<String>,
    /// 决策类型
    pub decision_type: RoutingDecisionType,
    /// 是否需要人类确认
    pub needs_human_confirmation: bool,
    /// 额外的执行参数
    pub execution_params: HashMap<String, String>,
}

/// 路由决策类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoutingDecisionType {
    /// 精确匹配
    ExactMatch,
    /// 能力匹配
    CapabilityMatch,
    /// LLM推理
    LLMInference,
    /// 规则匹配
    RuleMatch,
    /// 默认分配
    Default,
    /// 人类指定
    HumanAssigned,
}

// =============================================================================
// AgentRouter Trait - 动态路由决策
// =============================================================================

/// Agent路由器Trait
///
/// 负责决定将任务分配给哪个Agent。
/// 可以基于LLM提示词、规则引擎或自定义逻辑。
#[async_trait::async_trait]
pub trait AgentRouter: Send + Sync {
    /// 路由器名称
    fn name(&self) -> &str;

    /// 为子任务选择最合适的Agent
    async fn route(
        &self,
        context: &RoutingContext,
        available_agents: &[AgentInfo],
    ) -> anyhow::Result<RoutingDecision>;

    /// 批量路由多个子任务
    async fn route_batch(
        &self,
        contexts: &[RoutingContext],
        available_agents: &[AgentInfo],
    ) -> anyhow::Result<Vec<RoutingDecision>> {
        let mut results = Vec::new();
        for ctx in contexts {
            let decision = self.route(ctx, available_agents).await?;
            results.push(decision);
        }
        Ok(results)
    }

    /// 验证路由决策是否有效
    async fn validate_decision(
        &self,
        decision: &RoutingDecision,
        available_agents: &[AgentInfo],
    ) -> bool {
        available_agents.iter().any(|a| a.id == decision.agent_id)
    }
}

// =============================================================================
// 基于LLM的智能路由器
// =============================================================================

/// LLM智能路由器
///
/// 使用LLM分析任务需求，智能选择最合适的Agent。
pub struct LLMAgentRouter {
    /// LLM提供者
    llm: Arc<dyn LLMProvider>,
    /// 自定义系统提示词
    system_prompt: Option<String>,
    /// 自定义路由提示词模板
    routing_prompt_template: Option<String>,
    /// 是否详细解释决策
    explain_decisions: bool,
    /// 置信度阈值（低于此值需要人类确认）
    confidence_threshold: f32,
}

impl LLMAgentRouter {
    pub fn new(llm: Arc<dyn LLMProvider>) -> Self {
        Self {
            llm,
            system_prompt: None,
            routing_prompt_template: None,
            explain_decisions: true,
            confidence_threshold: 0.7,
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_routing_prompt_template(mut self, template: impl Into<String>) -> Self {
        self.routing_prompt_template = Some(template.into());
        self
    }

    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    pub fn with_explain_decisions(mut self, explain: bool) -> Self {
        self.explain_decisions = explain;
        self
    }

    /// 生成路由提示词
    fn generate_routing_prompt(&self, context: &RoutingContext, agents: &[AgentInfo]) -> String {
        if let Some(ref template) = self.routing_prompt_template {
            // 使用自定义模板
            template
                .replace("{subtask_description}", &context.subtask.description)
                .replace(
                    "{required_capabilities}",
                    &context.subtask.required_capabilities.join(", "),
                )
                .replace("{raw_idea}", &context.raw_idea)
                .replace(
                    "{agents}",
                    &agents
                        .iter()
                        .map(|a| {
                            format!(
                                "- {}: {} (能力: {})",
                                a.id,
                                a.description,
                                a.capabilities.join(", ")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
        } else {
            // 默认模板
            format!(
                r#"请为以下任务选择最合适的执行Agent。

## 任务信息
- 任务描述: {}
- 所需能力: {}
- 原始需求: {}

## 可用Agent列表
{}

## 要求
请以JSON格式返回路由决策：
```json
{{
    "agent_id": "选中的Agent ID",
    "reason": "选择理由",
    "confidence": 0.0-1.0的置信度,
    "alternatives": ["备选Agent ID列表"]
}}
```

请基于任务需求和Agent能力做出最佳匹配。"#,
                context.subtask.description,
                context.subtask.required_capabilities.join(", "),
                context.raw_idea,
                agents
                    .iter()
                    .map(|a| format!(
                        "- ID: {}\n  名称: {}\n  描述: {}\n  能力: {}\n  任务类型: {}\n  性能评分: {:.2}",
                        a.id,
                        a.name,
                        a.description,
                        a.capabilities.join(", "),
                        a.supported_task_types.join(", "),
                        a.performance_score
                    ))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            )
        }
    }

    /// 默认系统提示词
    fn default_system_prompt(&self) -> &'static str {
        r#"你是一个任务路由专家，负责将任务分配给最合适的执行Agent。

你需要考虑以下因素：
1. 任务所需的能力与Agent的能力是否匹配
2. Agent的历史性能评分
3. 任务类型与Agent支持的任务类型是否匹配
4. Agent的当前负载情况

做出决策时，请确保：
- 选择能力最匹配的Agent
- 如果有多个匹配的Agent，优先选择性能评分高的
- 如果没有完全匹配的，选择最接近的并降低置信度
- 始终提供清晰的决策理由"#
    }
}

#[async_trait::async_trait]
impl AgentRouter for LLMAgentRouter {
    fn name(&self) -> &str {
        "llm_router"
    }

    async fn route(
        &self,
        context: &RoutingContext,
        available_agents: &[AgentInfo],
    ) -> anyhow::Result<RoutingDecision> {
        if available_agents.is_empty() {
            return Err(anyhow::anyhow!("No available agents"));
        }

        // 构建消息
        let messages = vec![
            ChatMessage::system(
                self.system_prompt
                    .as_deref()
                    .unwrap_or_else(|| self.default_system_prompt()),
            ),
            ChatMessage::user(self.generate_routing_prompt(context, available_agents)),
        ];

        // 调用LLM
        let response = self.llm.chat(messages).await?;

        // 解析响应
        #[derive(Deserialize)]
        struct LLMRoutingResponse {
            agent_id: String,
            reason: String,
            confidence: Option<f32>,
            alternatives: Option<Vec<String>>,
        }

        match parse_llm_json::<LLMRoutingResponse>(&response) {
            Ok(parsed) => {
                let confidence = parsed.confidence.unwrap_or(0.8);
                Ok(RoutingDecision {
                    agent_id: parsed.agent_id,
                    reason: parsed.reason,
                    confidence,
                    alternatives: parsed.alternatives.unwrap_or_default(),
                    decision_type: RoutingDecisionType::LLMInference,
                    needs_human_confirmation: confidence < self.confidence_threshold,
                    execution_params: HashMap::new(),
                })
            }
            Err(_) => {
                // 回退：选择第一个可用Agent
                let fallback_agent = available_agents
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("No available agents"))?;

                Ok(RoutingDecision {
                    agent_id: fallback_agent.id.clone(),
                    reason: "LLM响应解析失败，使用默认分配".to_string(),
                    confidence: 0.5,
                    alternatives: available_agents
                        .iter()
                        .skip(1)
                        .map(|a| a.id.clone())
                        .collect(),
                    decision_type: RoutingDecisionType::Default,
                    needs_human_confirmation: true,
                    execution_params: HashMap::new(),
                })
            }
        }
    }
}

// =============================================================================
// 基于规则的路由器
// =============================================================================

/// 路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// 规则ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 规则优先级（数字越大优先级越高）
    pub priority: i32,
    /// 匹配条件
    pub conditions: Vec<RuleCondition>,
    /// 条件组合方式
    pub condition_logic: ConditionLogic,
    /// 目标Agent ID
    pub target_agent_id: String,
    /// 规则启用状态
    pub enabled: bool,
}

/// 规则条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    /// 条件字段
    pub field: RuleField,
    /// 操作符
    pub operator: RuleOperator,
    /// 匹配值
    pub value: String,
}

/// 规则字段
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuleField {
    /// 子任务描述
    SubtaskDescription,
    /// 所需能力
    RequiredCapability,
    /// 任务类型
    TaskType,
    /// 优先级
    Priority,
    /// 元数据字段
    Metadata(String),
    /// 原始需求
    RawIdea,
}

/// 规则操作符
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuleOperator {
    /// 等于
    Equals,
    /// 不等于
    NotEquals,
    /// 包含
    Contains,
    /// 不包含
    NotContains,
    /// 以...开头
    StartsWith,
    /// 以...结尾
    EndsWith,
    /// 正则匹配
    Regex,
    /// 在列表中
    In,
    /// 不在列表中
    NotIn,
}

/// 条件组合逻辑
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConditionLogic {
    /// 所有条件都满足
    All,
    /// 任一条件满足
    Any,
}

/// 基于规则的路由器
pub struct RuleBasedRouter {
    /// 规则列表
    rules: Arc<RwLock<Vec<RoutingRule>>>,
    /// 默认Agent ID（无规则匹配时使用）
    default_agent_id: Option<String>,
    /// 是否需要人类确认规则匹配
    confirm_on_match: bool,
}

impl RuleBasedRouter {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            default_agent_id: None,
            confirm_on_match: false,
        }
    }

    pub fn with_default_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.default_agent_id = Some(agent_id.into());
        self
    }

    pub fn with_confirm_on_match(mut self, confirm: bool) -> Self {
        self.confirm_on_match = confirm;
        self
    }

    /// 添加规则
    pub async fn add_rule(&self, rule: RoutingRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule);
        // 按优先级排序
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// 移除规则
    pub async fn remove_rule(&self, rule_id: &str) {
        let mut rules = self.rules.write().await;
        rules.retain(|r| r.id != rule_id);
    }

    /// 检查条件是否匹配
    fn check_condition(&self, condition: &RuleCondition, context: &RoutingContext) -> bool {
        let field_value = match &condition.field {
            RuleField::SubtaskDescription => context.subtask.description.clone(),
            RuleField::RequiredCapability => context.subtask.required_capabilities.join(","),
            RuleField::TaskType => context
                .metadata
                .get("task_type")
                .cloned()
                .unwrap_or_default(),
            RuleField::Priority => context
                .metadata
                .get("priority")
                .cloned()
                .unwrap_or_default(),
            RuleField::Metadata(key) => context.metadata.get(key).cloned().unwrap_or_default(),
            RuleField::RawIdea => context.raw_idea.clone(),
        };

        match &condition.operator {
            RuleOperator::Equals => field_value == condition.value,
            RuleOperator::NotEquals => field_value != condition.value,
            RuleOperator::Contains => field_value.contains(&condition.value),
            RuleOperator::NotContains => !field_value.contains(&condition.value),
            RuleOperator::StartsWith => field_value.starts_with(&condition.value),
            RuleOperator::EndsWith => field_value.ends_with(&condition.value),
            RuleOperator::Regex => regex::Regex::new(&condition.value)
                .map(|re| re.is_match(&field_value))
                .unwrap_or(false),
            RuleOperator::In => condition.value.split(',').any(|v| v.trim() == field_value),
            RuleOperator::NotIn => !condition.value.split(',').any(|v| v.trim() == field_value),
        }
    }

    /// 检查规则是否匹配
    fn check_rule(&self, rule: &RoutingRule, context: &RoutingContext) -> bool {
        if !rule.enabled {
            return false;
        }

        match rule.condition_logic {
            ConditionLogic::All => rule
                .conditions
                .iter()
                .all(|c| self.check_condition(c, context)),
            ConditionLogic::Any => rule
                .conditions
                .iter()
                .any(|c| self.check_condition(c, context)),
        }
    }
}

impl Default for RuleBasedRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentRouter for RuleBasedRouter {
    fn name(&self) -> &str {
        "rule_based_router"
    }

    async fn route(
        &self,
        context: &RoutingContext,
        available_agents: &[AgentInfo],
    ) -> anyhow::Result<RoutingDecision> {
        let rules = self.rules.read().await;

        // 查找第一个匹配的规则
        for rule in rules.iter() {
            if self.check_rule(rule, context) {
                // 验证目标Agent是否可用
                if available_agents
                    .iter()
                    .any(|a| a.id == rule.target_agent_id)
                {
                    return Ok(RoutingDecision {
                        agent_id: rule.target_agent_id.clone(),
                        reason: format!("匹配规则: {} ({})", rule.name, rule.id),
                        confidence: 1.0,
                        alternatives: Vec::new(),
                        decision_type: RoutingDecisionType::RuleMatch,
                        needs_human_confirmation: self.confirm_on_match,
                        execution_params: HashMap::new(),
                    });
                }
            }
        }

        // 没有匹配的规则，使用默认Agent或第一个可用Agent
        let agent_id = self
            .default_agent_id
            .clone()
            .or_else(|| available_agents.first().map(|a| a.id.clone()))
            .ok_or_else(|| anyhow::anyhow!("No available agents"))?;

        Ok(RoutingDecision {
            agent_id,
            reason: "无匹配规则，使用默认分配".to_string(),
            confidence: 0.5,
            alternatives: available_agents.iter().map(|a| a.id.clone()).collect(),
            decision_type: RoutingDecisionType::Default,
            needs_human_confirmation: true,
            execution_params: HashMap::new(),
        })
    }
}

// =============================================================================
// 能力匹配路由器
// =============================================================================

/// 能力匹配路由器
///
/// 基于能力标签进行简单的匹配路由
pub struct CapabilityRouter {
    /// 能力权重配置
    capability_weights: HashMap<String, f32>,
    /// 是否启用负载均衡
    load_balancing: bool,
    /// 性能权重
    performance_weight: f32,
}

impl CapabilityRouter {
    pub fn new() -> Self {
        Self {
            capability_weights: HashMap::new(),
            load_balancing: true,
            performance_weight: 0.3,
        }
    }

    pub fn with_capability_weight(mut self, capability: impl Into<String>, weight: f32) -> Self {
        self.capability_weights.insert(capability.into(), weight);
        self
    }

    pub fn with_load_balancing(mut self, enabled: bool) -> Self {
        self.load_balancing = enabled;
        self
    }

    pub fn with_performance_weight(mut self, weight: f32) -> Self {
        self.performance_weight = weight;
        self
    }

    /// 计算Agent与任务的匹配分数
    fn calculate_match_score(&self, agent: &AgentInfo, required_caps: &[String]) -> f32 {
        if required_caps.is_empty() {
            return 1.0;
        }

        let mut score = 0.0;
        let mut total_weight = 0.0;

        for cap in required_caps {
            let weight = self.capability_weights.get(cap).copied().unwrap_or(1.0);
            total_weight += weight;

            if agent.capabilities.contains(cap) {
                score += weight;
            }
        }

        let capability_score = if total_weight > 0.0 {
            score / total_weight
        } else {
            1.0
        };

        // 加权计算
        let load_score = if self.load_balancing {
            1.0 - (agent.current_load as f32 / 100.0)
        } else {
            1.0
        };

        let performance_score = agent.performance_score;

        // 综合评分
        capability_score * 0.5 + load_score * 0.2 + performance_score * self.performance_weight
    }
}

impl Default for CapabilityRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentRouter for CapabilityRouter {
    fn name(&self) -> &str {
        "capability_router"
    }

    async fn route(
        &self,
        context: &RoutingContext,
        available_agents: &[AgentInfo],
    ) -> anyhow::Result<RoutingDecision> {
        if available_agents.is_empty() {
            return Err(anyhow::anyhow!("No available agents"));
        }

        let required_caps = &context.subtask.required_capabilities;

        // 计算所有Agent的匹配分数
        let mut scored_agents: Vec<(&AgentInfo, f32)> = available_agents
            .iter()
            .map(|a| (a, self.calculate_match_score(a, required_caps)))
            .collect();

        // 按分数排序
        scored_agents.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (best_agent, best_score) = scored_agents[0];

        Ok(RoutingDecision {
            agent_id: best_agent.id.clone(),
            reason: format!(
                "能力匹配得分最高 ({:.2})，匹配能力: {:?}",
                best_score,
                required_caps
                    .iter()
                    .filter(|c| best_agent.capabilities.contains(c))
                    .collect::<Vec<_>>()
            ),
            confidence: best_score,
            alternatives: scored_agents
                .iter()
                .skip(1)
                .map(|(a, _)| a.id.clone())
                .collect(),
            decision_type: RoutingDecisionType::CapabilityMatch,
            needs_human_confirmation: best_score < 0.5,
            execution_params: HashMap::new(),
        })
    }
}

// =============================================================================
// 复合路由器
// =============================================================================

/// 复合路由器
///
/// 组合多个路由器，按优先级顺序尝试
pub struct CompositeRouter {
    /// 路由器列表（按优先级排序）
    routers: Vec<Arc<dyn AgentRouter>>,
    /// 回退路由器
    fallback_router: Option<Arc<dyn AgentRouter>>,
}

impl CompositeRouter {
    pub fn new() -> Self {
        Self {
            routers: Vec::new(),
            fallback_router: None,
        }
    }

    /// 添加路由器
    pub fn add_router(mut self, router: Arc<dyn AgentRouter>) -> Self {
        self.routers.push(router);
        self
    }

    /// 设置回退路由器
    pub fn with_fallback(mut self, router: Arc<dyn AgentRouter>) -> Self {
        self.fallback_router = Some(router);
        self
    }
}

impl Default for CompositeRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentRouter for CompositeRouter {
    fn name(&self) -> &str {
        "composite_router"
    }

    async fn route(
        &self,
        context: &RoutingContext,
        available_agents: &[AgentInfo],
    ) -> anyhow::Result<RoutingDecision> {
        // 尝试每个路由器
        for router in &self.routers {
            match router.route(context, available_agents).await {
                Ok(decision) if decision.confidence >= 0.5 => {
                    return Ok(decision);
                }
                _ => continue,
            }
        }

        // 使用回退路由器
        if let Some(ref fallback) = self.fallback_router {
            return fallback.route(context, available_agents).await;
        }

        // 默认选择第一个Agent
        let agent = available_agents
            .first()
            .ok_or_else(|| anyhow::anyhow!("No available agents"))?;

        Ok(RoutingDecision {
            agent_id: agent.id.clone(),
            reason: "所有路由器均无高置信度匹配，使用默认分配".to_string(),
            confidence: 0.3,
            alternatives: available_agents
                .iter()
                .skip(1)
                .map(|a| a.id.clone())
                .collect(),
            decision_type: RoutingDecisionType::Default,
            needs_human_confirmation: true,
            execution_params: HashMap::new(),
        })
    }
}

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_agent_provider() {
        let provider = InMemoryAgentProvider::new();

        // 添加Agent
        provider
            .add_agent(
                AgentInfo::new("agent_1", "Test Agent 1")
                    .with_capability("backend")
                    .with_performance_score(0.9),
            )
            .await;

        provider
            .add_agent(
                AgentInfo::new("agent_2", "Test Agent 2")
                    .with_capability("frontend")
                    .with_performance_score(0.85),
            )
            .await;

        // 列出所有Agent
        let agents = provider.list_agents().await;
        assert_eq!(agents.len(), 2);

        // 按能力筛选
        let backend_agents = provider
            .filter_by_capabilities(&["backend".to_string()])
            .await;
        assert_eq!(backend_agents.len(), 1);
        assert_eq!(backend_agents[0].id, "agent_1");
    }

    #[tokio::test]
    async fn test_capability_router() {
        let router = CapabilityRouter::new()
            .with_load_balancing(true)
            .with_performance_weight(0.3);

        let agents = vec![
            AgentInfo::new("agent_1", "Backend Agent")
                .with_capability("backend")
                .with_capability("database")
                .with_performance_score(0.9),
            AgentInfo::new("agent_2", "Frontend Agent")
                .with_capability("frontend")
                .with_capability("ui")
                .with_performance_score(0.85),
        ];

        let context = RoutingContext::new(
            Subtask {
                id: "task_1".to_string(),
                description: "Build API".to_string(),
                required_capabilities: vec!["backend".to_string()],
                order: 1,
                depends_on: Vec::new(),
            },
            "todo_1",
        );

        let decision = router.route(&context, &agents).await.unwrap();
        assert_eq!(decision.agent_id, "agent_1");
        assert_eq!(decision.decision_type, RoutingDecisionType::CapabilityMatch);
    }

    #[tokio::test]
    async fn test_rule_based_router() {
        let router = RuleBasedRouter::new();

        // 添加规则
        router
            .add_rule(RoutingRule {
                id: "rule_1".to_string(),
                name: "Backend Rule".to_string(),
                priority: 10,
                conditions: vec![RuleCondition {
                    field: RuleField::RequiredCapability,
                    operator: RuleOperator::Contains,
                    value: "backend".to_string(),
                }],
                condition_logic: ConditionLogic::All,
                target_agent_id: "backend_agent".to_string(),
                enabled: true,
            })
            .await;

        let agents = vec![
            AgentInfo::new("backend_agent", "Backend Agent"),
            AgentInfo::new("frontend_agent", "Frontend Agent"),
        ];

        let context = RoutingContext::new(
            Subtask {
                id: "task_1".to_string(),
                description: "Build API".to_string(),
                required_capabilities: vec!["backend".to_string()],
                order: 1,
                depends_on: Vec::new(),
            },
            "todo_1",
        );

        let decision = router.route(&context, &agents).await.unwrap();
        assert_eq!(decision.agent_id, "backend_agent");
        assert_eq!(decision.decision_type, RoutingDecisionType::RuleMatch);
    }

    #[tokio::test]
    async fn test_composite_router() {
        let rule_router = Arc::new(RuleBasedRouter::new());
        let capability_router = Arc::new(CapabilityRouter::new());

        let composite = CompositeRouter::new()
            .add_router(rule_router)
            .with_fallback(capability_router);

        let agents = vec![
            AgentInfo::new("agent_1", "Agent 1")
                .with_capability("general")
                .with_performance_score(0.8),
        ];

        let context = RoutingContext::new(
            Subtask {
                id: "task_1".to_string(),
                description: "General task".to_string(),
                required_capabilities: vec![],
                order: 1,
                depends_on: Vec::new(),
            },
            "todo_1",
        );

        let decision = composite.route(&context, &agents).await.unwrap();
        assert_eq!(decision.agent_id, "agent_1");
    }
}
