//! ReAct (Reasoning + Acting) Agent 框架
//! ReAct (Reasoning + Acting) Agent Framework
//!
//! 基于 ractor Actor 模型实现的 ReAct Agent，支持：
//! ReAct Agent implemented based on ractor Actor model, supporting:
//!
//! - **思考-行动-观察循环**: 标准 ReAct 推理模式
//! - **Thought-Action-Observation loop**: Standard ReAct reasoning pattern
//! - **工具调用**: 支持自定义工具注册和执行
//! - **Tool calling**: Supports custom tool registration and execution
//! - **Actor 模型**: 基于 ractor 实现，支持并发和消息传递
//! - **Actor model**: Implemented via ractor, supporting concurrency and messaging
//! - **AutoAgent**: 自动选择最佳行动策略
//! - **AutoAgent**: Automatically selects the best action strategy
//! - **流式输出**: 支持流式思考过程输出
//! - **Stream output**: Supports streaming of the reasoning process
//!
//! # 架构
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                 ReAct Agent 架构                                │
//! │                 ReAct Agent Architecture                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐      │
//! │  │   Input     │─────▶│   Thought   │─────▶│    Action  │      │
//! │  │   (任务)     │      │   (推理)    │      │    (行动)   │      │
//! │  │   (Task)    │      │ (Reasoning) │      │   (Action)  │      │
//! │  └─────────────┘      └─────────────┘      └──────┬──────┘      │
//! │                                                   │             │
//! │                                                   ▼             │
//! │  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐      │
//! │  │   Output    │◀─────│    Final    │◀─────│ Observation │     │
//! │  │   (结果)     │      │   Answer    │      │    (观察)   │      │
//! │  │  (Result)   │      │             │      │(Observation)│      │
//! │  └─────────────┘      └─────────────┘      └─────────────┘      │
//! │                                                                 │
//! │  ┌─────────────────────────────────────────────────────────┐    │
//! │  │                     Tool Registry                       │    │
//! │  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐                │    │
//! │  │  │Tool1│ │Tool2│ │Tool3│ │Tool4│ │ ... │                │    │
//! │  │  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘                │    │
//! │  └─────────────────────────────────────────────────────────┘    │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 示例
//! # Examples
//!
//! ## 基本用法
//! ## Basic Usage
//!
//! ```rust,ignore
//! use mofa_foundation::react::{ReActAgent, ReActTool};
//! use std::sync::Arc;
//!
//! // 定义工具
//! // Define tools
//! struct SearchTool;
//!
//! #[async_trait::async_trait]
//! impl ReActTool for SearchTool {
//!     fn name(&self) -> &str { "search" }
//!     fn description(&self) -> &str { "Search the web for information" }
//!
//!     async fn execute(&self, input: &str) -> Result<String, String> {
//!         Ok(format!("Search results for: {}", input))
//!     }
//! }
//!
//! // 创建 ReAct Agent
//! // Create ReAct Agent
//! let agent = ReActAgent::builder()
//!     .with_llm(llm_agent)
//!     .with_tool(Arc::new(SearchTool))
//!     .with_max_iterations(5)
//!     .build()?;
//!
//! // 执行任务
//! // Execute task
//! let result = agent.run("What is the capital of France?").await?;
//! info!("Answer: {}", result.answer);
//! ```
//!
//! ## 使用 Actor 模型
//! ## Using Actor Model
//!
//! ```rust,ignore
//! use mofa_foundation::react::{ReActActorRef, spawn_react_agent};
//!
//! // 启动 ReAct Actor
//! // Start ReAct Actor
//! let (actor, handle) = spawn_react_agent(config).await?;
//!
//! // 发送任务
//! // Send task
//! let result = actor.run_task("Analyze this data").await?;
//! ```

mod actor;
mod core;
pub mod patterns;
pub mod reflection;
pub mod tools;

pub use actor::*;
pub use core::*;
pub use patterns::*;
pub use reflection::*;
pub use tools::*;

/// 便捷 prelude 模块
/// Convenient prelude module
pub mod prelude {
    pub use super::patterns::{
        AgentOutput, AgentUnit, AggregationStrategy, ChainAgent, ChainResult, ChainStepResult,
        MapReduceAgent, MapReduceResult, ParallelAgent, ParallelResult, ParallelStepResult,
        chain_agents, parallel_agents, parallel_agents_with_summarizer,
    };
    pub use super::reflection::{
        ReflectionAgent, ReflectionAgentBuilder, ReflectionConfig, ReflectionResult, ReflectionStep,
    };
    pub use super::tools::prelude::*;
}
