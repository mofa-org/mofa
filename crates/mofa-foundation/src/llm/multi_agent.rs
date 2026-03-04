//! 多 Agent 协作模式
//! Multi-Agent Collaboration Patterns
//!
//! 提供高级的多 Agent 协作模式，包括：
//! Provides advanced multi-agent collaboration patterns, including:
//!
//! - **链式协作**: Agent 串行执行，前一个输出是后一个输入
//! - **Chain**: Sequential execution where one's output is the next's input
//! - **并行协作**: 多个 Agent 同时处理，结果聚合
//! - **Parallel**: Multiple agents process simultaneously with result aggregation
//! - **辩论模式**: 多个 Agent 交替辩论，达成共识
//! - **Debate**: Multiple agents debate in turns to reach a consensus
//! - **监督模式**: 一个监督 Agent 评估其他 Agent 的输出
//! - **Supervised**: A supervisor agent evaluates the outputs of other agents
//! - **MapReduce**: 并行处理后归约
//! - **MapReduce**: Parallel processing followed by reduction/synthesis
//!
//! # 示例
//! # Example
//!
//! ```rust,ignore
//! use mofa_foundation::llm::multi_agent::{AgentTeam, TeamPattern};
//!
//! // 创建 Agent 团队
//! // Create an Agent team
//! let team = AgentTeam::new()
//!     .add_agent("analyst", analyst_agent)
//!     .add_agent("writer", writer_agent)
//!     .add_agent("editor", editor_agent)
//!     .with_pattern(TeamPattern::Chain)
//!     .build();
//!
//! let result = team.run("Analyze and write about Rust").await?;
//! ```

use super::agent::LLMAgent;
use super::types::{LLMError, LLMResult};
use std::collections::HashMap;
use std::sync::Arc;

/// Agent 团队协作模式
/// Agent Team Collaboration Patterns
#[derive(Debug, Clone)]
pub enum TeamPattern {
    /// 链式：按顺序执行
    /// Chain: Execute in sequential order
    Chain,
    /// 并行：同时执行，结果聚合
    /// Parallel: Execute simultaneously and aggregate results
    Parallel,
    /// 辩论：多个 Agent 交替发言
    /// Debate: Multiple agents take turns to speak
    Debate {
        /// 最大轮数
        /// Maximum number of rounds
        max_rounds: usize,
    },
    /// 监督：一个监督者评估结果
    /// Supervised: A supervisor evaluates the results
    Supervised,
    /// MapReduce：并行处理后归约
    /// MapReduce: Parallel processing followed by reduction
    MapReduce,
    /// 自定义
    /// Custom
    Custom,
}

/// Agent 角色
/// Agent Role
#[derive(Debug, Clone)]
pub struct AgentRole {
    /// 角色 ID
    /// Role ID
    pub id: String,
    /// 角色名称
    /// Role name
    pub name: String,
    /// 角色描述（会添加到系统提示中）
    /// Role description (will be added to the system prompt)
    pub description: String,
    /// 提示词模板
    /// Prompt template
    pub prompt_template: Option<String>,
}

impl AgentRole {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            prompt_template: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_template(mut self, template: impl Into<String>) -> Self {
        self.prompt_template = Some(template.into());
        self
    }
}

/// Agent 成员
/// Agent Member
pub struct AgentMember {
    /// 角色信息
    /// Role information
    pub role: AgentRole,
    /// Agent 实例
    /// Agent instance
    pub agent: Arc<LLMAgent>,
}

impl AgentMember {
    pub fn new(id: impl Into<String>, agent: Arc<LLMAgent>) -> Self {
        let id = id.into();
        Self {
            role: AgentRole::new(&id, &id),
            agent,
        }
    }

    pub fn with_role(mut self, role: AgentRole) -> Self {
        self.role = role;
        self
    }

    /// 执行任务
    /// Execute task
    pub async fn execute(&self, input: &str, context: Option<&str>) -> LLMResult<String> {
        let prompt = if let Some(ref template) = self.role.prompt_template {
            let mut p = template.replace("{input}", input);
            if let Some(ctx) = context {
                p = p.replace("{context}", ctx);
            }
            p
        } else if let Some(ctx) = context {
            format!("Context:\n{}\n\nTask:\n{}", ctx, input)
        } else {
            input.to_string()
        };

        self.agent.ask(&prompt).await
    }
}

/// Agent 团队
/// Agent Team
pub struct AgentTeam {
    /// 团队 ID
    /// Team ID
    pub id: String,
    /// 团队名称
    /// Team name
    pub name: String,
    /// 成员列表
    /// List of members
    members: Vec<AgentMember>,
    /// 成员映射（按 ID）
    /// Member mapping (by ID)
    member_map: HashMap<String, usize>,
    /// 协作模式
    /// Collaboration pattern
    pattern: TeamPattern,
    /// 监督者 ID（用于 Supervised 模式）
    /// Supervisor ID (used for Supervised pattern)
    supervisor_id: Option<String>,
    /// 聚合提示词（用于并行和 MapReduce 模式）
    /// Aggregation prompt (used for Parallel and MapReduce patterns)
    aggregate_prompt: Option<String>,
}

impl AgentTeam {
    /// 创建新的 Agent 团队构建器
    /// Create a new Agent team builder
    pub fn builder(id: impl Into<String>) -> AgentTeamBuilder {
        AgentTeamBuilder::new(id)
    }

    /// 链式执行
    /// Chain execution
    async fn run_chain(&self, input: &str) -> LLMResult<String> {
        let mut current_output = input.to_string();

        for member in &self.members {
            current_output = member.execute(&current_output, None).await?;
        }

        Ok(current_output)
    }

    /// 并行执行
    /// Parallel execution
    async fn run_parallel(&self, input: &str) -> LLMResult<String> {
        let mut results = Vec::new();

        // 由于 Agent 包含不可跨线程的闭包，这里顺序执行
        // Due to Agents containing non-thread-safe closures, this executes sequentially
        // 未来可以通过重构 Agent 来实现真正的并行
        // True parallelism can be achieved by refactoring the Agent in the future
        for member in &self.members {
            let result = member.execute(input, None).await?;
            results.push((member.role.id.clone(), result));
        }

        // 聚合结果
        // Aggregate results
        let aggregated = results
            .iter()
            .map(|(id, result)| format!("=== {} ===\n{}", id, result))
            .collect::<Vec<_>>()
            .join("\n\n");

        // 如果有聚合提示词，使用第一个 Agent 进行聚合
        // If an aggregation prompt exists, use the first agent to aggregate
        if let Some(ref aggregate_prompt) = self.aggregate_prompt
            && let Some(first_member) = self.members.first()
        {
            let prompt = aggregate_prompt
                .replace("{results}", &aggregated)
                .replace("{input}", input);
            return first_member.agent.ask(&prompt).await;
        }

        Ok(aggregated)
    }

    /// 辩论执行
    /// Debate execution
    async fn run_debate(&self, input: &str, max_rounds: usize) -> LLMResult<String> {
        if self.members.len() < 2 {
            return Err(LLMError::Other(
                "Debate requires at least 2 agents".to_string(),
            ));
        }

        let mut context = format!("Initial topic: {}\n\n", input);
        let mut last_response = String::new();

        for round in 0..max_rounds {
            for (i, member) in self.members.iter().enumerate() {
                let prompt = format!(
                    "Round {}, Speaker {}: {}\n\n\
                    Previous discussion:\n{}\n\n\
                    Please provide your perspective. Be constructive and build on previous points.",
                    round + 1,
                    i + 1,
                    member.role.name,
                    context
                );

                let response = member.execute(&prompt, None).await?;
                context.push_str(&format!(
                    "[{} - Round {}]:\n{}\n\n",
                    member.role.name,
                    round + 1,
                    response
                ));
                last_response = response;
            }
        }

        // 最后总结
        // Final summary
        if let Some(first_member) = self.members.first() {
            let summary_prompt = format!(
                "Based on the following debate, provide a concise summary of the key points \
                and conclusions:\n\n{}",
                context
            );
            first_member.agent.ask(&summary_prompt).await
        } else {
            Ok(last_response)
        }
    }

    /// 监督执行
    /// Supervised execution
    async fn run_supervised(&self, input: &str) -> LLMResult<String> {
        let supervisor_id = self.supervisor_id.as_ref().ok_or_else(|| {
            LLMError::Other("Supervisor not specified for Supervised pattern".to_string())
        })?;

        let supervisor_idx = self
            .member_map
            .get(supervisor_id)
            .ok_or_else(|| LLMError::Other(format!("Supervisor '{}' not found", supervisor_id)))?;

        // 收集工作者结果
        // Collect results from workers
        let mut worker_results = Vec::new();
        for (i, member) in self.members.iter().enumerate() {
            if i != *supervisor_idx {
                let result = member.execute(input, None).await?;
                worker_results.push((member.role.id.clone(), member.role.name.clone(), result));
            }
        }

        // 让监督者评估
        // Let the supervisor evaluate
        let results_text = worker_results
            .iter()
            .map(|(id, name, result)| format!("=== {} ({}) ===\n{}", name, id, result))
            .collect::<Vec<_>>()
            .join("\n\n");

        let supervisor = &self.members[*supervisor_idx];
        let eval_prompt = format!(
            "You are the supervisor. Evaluate the following responses to the task: \"{}\"\n\n\
            Responses:\n{}\n\n\
            Please provide:\n\
            1. An evaluation of each response\n\
            2. The best response or a synthesized improved response\n\
            3. Suggestions for improvement",
            input, results_text
        );

        supervisor.agent.ask(&eval_prompt).await
    }

    /// MapReduce 执行
    /// MapReduce execution
    async fn run_map_reduce(&self, input: &str) -> LLMResult<String> {
        // Map 阶段：每个 Agent 处理输入
        // Map phase: Each agent processes the input
        let mut mapped_results = Vec::new();
        for member in &self.members {
            let result = member.execute(input, None).await?;
            mapped_results.push((member.role.id.clone(), result));
        }

        // Reduce 阶段：聚合结果
        // Reduce phase: Aggregate results
        let reduce_input = mapped_results
            .iter()
            .map(|(id, result)| format!("[{}]: {}", id, result))
            .collect::<Vec<_>>()
            .join("\n\n");

        let reduce_prompt = if let Some(ref aggregate_prompt) = self.aggregate_prompt {
            aggregate_prompt
                .replace("{results}", &reduce_input)
                .replace("{input}", input)
        } else {
            format!(
                "Synthesize the following results into a coherent response:\n\n{}\n\n\
                Original task: {}",
                reduce_input, input
            )
        };

        // 使用第一个 Agent 进行 reduce
        // Use the first agent to perform the reduce operation
        if let Some(first_member) = self.members.first() {
            first_member.agent.ask(&reduce_prompt).await
        } else {
            Ok(reduce_input)
        }
    }

    /// 执行团队任务
    /// Execute team task
    pub async fn run(&self, input: impl Into<String>) -> LLMResult<String> {
        let input = input.into();

        match &self.pattern {
            TeamPattern::Chain => self.run_chain(&input).await,
            TeamPattern::Parallel => self.run_parallel(&input).await,
            TeamPattern::Debate { max_rounds } => self.run_debate(&input, *max_rounds).await,
            TeamPattern::Supervised => self.run_supervised(&input).await,
            TeamPattern::MapReduce => self.run_map_reduce(&input).await,
            TeamPattern::Custom => {
                // 自定义模式默认使用链式
                // Custom pattern defaults to Chain
                self.run_chain(&input).await
            }
        }
    }

    /// 获取成员
    /// Get member
    pub fn get_member(&self, id: &str) -> Option<&AgentMember> {
        self.member_map.get(id).map(|idx| &self.members[*idx])
    }

    /// 获取所有成员 ID
    /// Get all member IDs
    pub fn member_ids(&self) -> Vec<&str> {
        self.members.iter().map(|m| m.role.id.as_str()).collect()
    }
}

/// Agent 团队构建器
/// Agent Team Builder
pub struct AgentTeamBuilder {
    id: String,
    name: String,
    members: Vec<AgentMember>,
    pattern: TeamPattern,
    supervisor_id: Option<String>,
    aggregate_prompt: Option<String>,
}

impl AgentTeamBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            members: Vec::new(),
            pattern: TeamPattern::Chain,
            supervisor_id: None,
            aggregate_prompt: None,
        }
    }

    /// 设置名称
    /// Set name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 添加成员
    /// Add member
    pub fn add_member(mut self, id: impl Into<String>, agent: Arc<LLMAgent>) -> Self {
        self.members.push(AgentMember::new(id, agent));
        self
    }

    /// 添加带角色的成员
    /// Add member with role
    pub fn add_member_with_role(mut self, agent: Arc<LLMAgent>, role: AgentRole) -> Self {
        let member = AgentMember::new(&role.id, agent).with_role(role);
        self.members.push(member);
        self
    }

    /// 设置协作模式
    /// Set collaboration pattern
    pub fn with_pattern(mut self, pattern: TeamPattern) -> Self {
        self.pattern = pattern;
        self
    }

    /// 设置监督者
    /// Set supervisor
    pub fn with_supervisor(mut self, supervisor_id: impl Into<String>) -> Self {
        self.supervisor_id = Some(supervisor_id.into());
        self.pattern = TeamPattern::Supervised;
        self
    }

    /// 设置聚合提示词
    /// Set aggregation prompt
    pub fn with_aggregate_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.aggregate_prompt = Some(prompt.into());
        self
    }

    /// 构建团队
    /// Build team
    #[must_use]
    pub fn build(self) -> AgentTeam {
        let member_map: HashMap<String, usize> = self
            .members
            .iter()
            .enumerate()
            .map(|(i, m)| (m.role.id.clone(), i))
            .collect();

        AgentTeam {
            id: self.id,
            name: self.name,
            members: self.members,
            member_map,
            pattern: self.pattern,
            supervisor_id: self.supervisor_id,
            aggregate_prompt: self.aggregate_prompt,
        }
    }
}

// ============================================================================
// 预定义团队模式
// Predefined Team Patterns
// ============================================================================

/// 创建内容创作团队
/// Create content creation team
///
/// 包含：研究员、写手、编辑
/// Includes: researcher, writer, editor
pub fn content_creation_team(
    researcher: Arc<LLMAgent>,
    writer: Arc<LLMAgent>,
    editor: Arc<LLMAgent>,
) -> AgentTeam {
    AgentTeamBuilder::new("content-creation")
        .with_name("Content Creation Team")
        .add_member_with_role(
            researcher,
            AgentRole::new("researcher", "Researcher")
                .with_description("Research and gather information on the topic")
                .with_template(
                    "Research the following topic thoroughly and provide key findings:\n\n{input}",
                ),
        )
        .add_member_with_role(
            writer,
            AgentRole::new("writer", "Writer")
                .with_description("Write engaging content based on research")
                .with_template(
                    "Based on the following research, write an engaging article:\n\n{input}",
                ),
        )
        .add_member_with_role(
            editor,
            AgentRole::new("editor", "Editor")
                .with_description("Edit and polish the content")
                .with_template(
                    "Edit and improve the following article for clarity and engagement:\n\n{input}",
                ),
        )
        .with_pattern(TeamPattern::Chain)
        .build()
}

/// 创建代码审查团队
/// Create code review team
///
/// 包含：安全审查员、性能审查员、风格审查员、监督者
/// Includes: security reviewer, performance reviewer, style reviewer, supervisor
pub fn code_review_team(
    security_reviewer: Arc<LLMAgent>,
    performance_reviewer: Arc<LLMAgent>,
    style_reviewer: Arc<LLMAgent>,
    supervisor: Arc<LLMAgent>,
) -> AgentTeam {
    AgentTeamBuilder::new("code-review")
        .with_name("Code Review Team")
        .add_member_with_role(
            security_reviewer,
            AgentRole::new("security", "Security Reviewer")
                .with_description("Review code for security vulnerabilities"),
        )
        .add_member_with_role(
            performance_reviewer,
            AgentRole::new("performance", "Performance Reviewer")
                .with_description("Review code for performance issues"),
        )
        .add_member_with_role(
            style_reviewer,
            AgentRole::new("style", "Style Reviewer")
                .with_description("Review code for style and best practices"),
        )
        .add_member_with_role(
            supervisor,
            AgentRole::new("supervisor", "Lead Reviewer")
                .with_description("Synthesize reviews and provide final feedback"),
        )
        .with_supervisor("supervisor")
        .build()
}

/// 创建辩论团队
/// Create debate team
///
/// 两个 Agent 进行辩论
/// Two agents engage in debate
pub fn debate_team(agent1: Arc<LLMAgent>, agent2: Arc<LLMAgent>, max_rounds: usize) -> AgentTeam {
    AgentTeamBuilder::new("debate")
        .with_name("Debate Team")
        .add_member_with_role(
            agent1,
            AgentRole::new("debater1", "Debater 1")
                .with_description("Present and defend your position"),
        )
        .add_member_with_role(
            agent2,
            AgentRole::new("debater2", "Debater 2")
                .with_description("Present an alternative perspective"),
        )
        .with_pattern(TeamPattern::Debate { max_rounds })
        .build()
}

/// 创建分析团队
/// Create analysis team
///
/// 多个 Agent 并行分析，然后聚合结果
/// Multiple agents analyze in parallel, then aggregate results
pub fn analysis_team(analysts: Vec<(impl Into<String>, Arc<LLMAgent>)>) -> AgentTeam {
    let mut builder = AgentTeamBuilder::new("analysis")
        .with_name("Analysis Team")
        .with_pattern(TeamPattern::MapReduce)
        .with_aggregate_prompt(
            "Synthesize the following analyses into a comprehensive report:\n\n{results}\n\n\
            Original question: {input}",
        );

    for (id, agent) in analysts {
        builder = builder.add_member(id, agent);
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_builder() {
        // 创建一个没有实际 Agent 的团队（仅测试构建器）
        // Create a team without real agents (only testing the builder)
        let builder = AgentTeamBuilder::new("test-team")
            .with_name("Test Team")
            .with_pattern(TeamPattern::Chain);

        // 只测试构建器的配置，不测试实际执行
        // Only test the builder configuration, not the actual execution
        assert_eq!(builder.id, "test-team");
        assert_eq!(builder.name, "Test Team");
    }

    #[test]
    fn test_agent_role() {
        let role = AgentRole::new("researcher", "Researcher")
            .with_description("Research topics")
            .with_template("{input}");

        assert_eq!(role.id, "researcher");
        assert_eq!(role.name, "Researcher");
        assert_eq!(role.description, "Research topics");
        assert!(role.prompt_template.is_some());
    }
}
