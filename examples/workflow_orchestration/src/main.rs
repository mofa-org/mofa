//! 工作流编排示例
//! Workflow orchestration example
//!
//! 演示 Graph-based workflow orchestration 功能:
//! Demonstrates Graph-based workflow orchestration features:
//! 1. 使用 WorkflowBuilder 流式构建工作流
//! 1. Fluidly build workflows using WorkflowBuilder
//! 2. 条件分支执行
//! 2. Conditional branch execution
//! 3. 并行分支和聚合
//! 3. Parallel branching and aggregation
//! 4. 状态管理和数据传递
//! 4. State management and data passing
//! 5. 执行事件监听
//! 5. Execution event listening
//! 6. LLM Agent 工作流集成（Dify 风格）
//! 6. LLM Agent workflow integration (Dify style)
//!
//! 运行: cargo run --example workflow_orchestration
//! Run: cargo run --example workflow_orchestration

use mofa_sdk::workflow::{
    ExecutionEvent, ExecutorConfig, WorkflowBuilder, WorkflowExecutor, WorkflowGraph, WorkflowNode, WorkflowValue,
};
use mofa_sdk::llm::{LLMAgent, LLMAgentBuilder, openai_from_env};
use mofa_sdk::react::{ReActAgent, prelude::all_builtin_tools};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc;
use tracing::{info, Level};

/// 创建工作流专用的 LLM Agent
/// Create an LLM Agent dedicated to the workflow
///
/// 这是一个辅助函数，用于创建带有标准配置的 LLM Agent。
/// This is a helper function used to create an LLM Agent with standard configuration.
fn create_workflow_llm(name: &str, system_prompt: &str) -> Arc<LLMAgent> {
    let provider = openai_from_env().expect("OPENAI_API_KEY must be set");
    Arc::new(
        LLMAgentBuilder::new()
            .with_name(name)
            .with_provider(Arc::new(provider))
            .with_system_prompt(system_prompt)
            .with_temperature(0.7)
            .with_max_tokens(2048)
            .build()
    )
}

/// 创建工作流专用的 ReAct Agent
/// Create a ReAct Agent dedicated to the workflow
///
/// 这是一个辅助函数，用于创建带有工具的 ReAct Agent。
/// This is a helper function used to create a ReAct Agent with tools.
async fn create_react_agent(name: &str, system_prompt: &str) -> Result<Arc<ReActAgent>, Box<dyn std::error::Error>> {
    let llm = create_workflow_llm(name, system_prompt);
    let tools = all_builtin_tools();

    let agent = Arc::new(
        ReActAgent::builder()
            .with_llm(llm)
            .with_tools(tools)
            .with_max_iterations(8)
            .build()?
    );

    Ok(agent)
}

/// Prompt 模板库
/// Prompt template library
///
/// 提供 Dify 风格的 Prompt 模板。
/// Provides Dify-style Prompt templates.
mod prompts {
    pub const FINAL_SYNTHESIS: &str = r#"你是一个专业的综合分析助手。
请基于以下推理结果，生成最终的综合报告：
- 总结推理过程
- 提炼关键结论
- 给出可执行的建议

推理结果: {{input}}"#;

    pub const TECHNICAL_ANALYSIS: &str = r#"你是一个技术专家。
请从技术角度分析以下内容：
- 技术可行性
- 技术风险点
- 建议的技术方案

内容: {{input}}"#;

    pub const BUSINESS_ANALYSIS: &str = r#"你是一个商业专家。
请从商业角度分析以下内容：
- 商业价值
- 市场潜力
- 成本效益分析

内容: {{input}}"#;

    pub const MULTI_PERSPECTIVE_SYNTHESIS: &str = r#"你是一个决策分析师。
请综合以下不同视角的分析，给出平衡的决策建议：
- 权衡技术和商业因素
- 识别共同关注点
- 给出综合建议

技术分析: {{technical}}
商业分析: {{business}}"#;

    pub const COMPLEXITY_CHECK: &str = r#"你是一个任务分类助手。
请判断以下任务的复杂度等级（简单/复杂）。
简单任务：可直接处理，无需深入分析
复杂任务：需要多步骤分析和工具支持

任务: {{input}}

请只回复 "simple" 或 "complex"。"#;

    pub const SIMPLE_PROCESSING: &str = r#"你是一个快速处理助手。
请简明扼要地处理以下简单任务：
- 直接回答问题
- 提供简洁方案

任务: {{input}}"#;

    pub const DEEP_ANALYSIS: &str = r#"你是一个深度分析助手。
请对以下复杂任务进行全面分析：
- 问题拆解
- 逐步分析
- 详细方案

任务: {{input}}"#;

    pub const FINAL_DECISION: &str = r#"你是一个决策助手。
请基于分析结果，给出明确的决策建议：
- 决策结论
- 关键理由
- 风险提示

分析结果: {{input}}"#;

    pub const LLM_ANALYSIS: &str = r#"你是一个数据洞察专家。
请分析以下数据，并提供智能洞察：
- 数据趋势
- 异常点识别
- 业务建议

数据: {{input}}"#;

    pub const LLM_SUMMARY: &str = r#"你是一个综合报告专家。
请基于以下工具执行结果，生成结构化的综合报告：
- 结果汇总
- 关键发现
- 行动建议

计算结果: {{calc}}
数据时间: {{datetime}}"#;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 设置日志
    // Setup logs
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== MoFA Workflow Orchestration Example ===\n");
    // === MoFA Workflow Orchestration Example ===

    // 示例1-5: 原有数据处理示例
    // Examples 1-5: Original data processing examples
    info!("--- Example 1: Linear Workflow ---");
    // --- Example 1: Linear Workflow ---
    run_linear_workflow().await?;

    info!("\n--- Example 2: Conditional Branch Workflow ---");
    // --- Example 2: Conditional Branch Workflow ---
    run_conditional_workflow().await?;

    info!("\n--- Example 3: Parallel Execution Workflow ---");
    // --- Example 3: Parallel Execution Workflow ---
    run_parallel_workflow().await?;

    info!("\n--- Example 4: Data Processing Pipeline ---");
    // --- Example 4: Data Processing Pipeline ---
    run_data_pipeline().await?;

    info!("\n--- Example 5: Event Listening Workflow ---");
    // --- Example 5: Event Listening Workflow ---
    run_workflow_with_events().await?;

    // 示例6-10: LLM/Agent 工作流示例（需要 OPENAI_API_KEY）
    // Examples 6-10: LLM/Agent Workflow Examples (Requires OPENAI_API_KEY)
    if std::env::var("OPENAI_API_KEY").is_ok() {
        info!("\n=== Dify-style LLM/Agent Workflow Examples ===\n");
        // === Dify-style LLM/Agent Workflow Examples ===

        info!("--- Example 6: ReAct Agent Decision Workflow ---");
        // --- Example 6: ReAct Agent Decision Workflow ---
        run_react_agent_workflow().await?;

        info!("\n--- Example 7: Multi-Agent Parallel Analysis Workflow ---");
        // --- Example 7: Multi-Agent Parallel Analysis Workflow ---
        run_multi_agent_parallel_workflow().await?;

        info!("\n--- Example 8: Conditional Routing + LLM Decision Workflow ---");
        // --- Example 8: Conditional Routing + LLM Decision Workflow ---
        run_conditional_llm_workflow().await?;

        info!("\n--- Example 9: Intelligent Data Pipeline Workflow ---");
        // --- Example 9: Intelligent Data Pipeline Workflow ---
        run_intelligent_pipeline_workflow().await?;

        info!("\n--- Example 10: Tool Chain + LLM Summary Workflow ---");
        // --- Example 10: Tool Chain + LLM Summary Workflow ---
        run_tool_chain_llm_workflow().await?;
    } else {
        info!("\n=== LLM/Agent examples skipped ===");
        // === LLM/Agent Examples Skipped ===
        info!("Set OPENAI_API_KEY environment variable to run LLM/Agent examples");
        // Set OPENAI_API_KEY environment variable to run LLM/Agent examples
    }

    info!("\n=== All examples have finished executing ===");
    // === All examples execution completed ===
    Ok(())
}

/// 示例1: 简单的线性工作流
/// Example 1: Simple linear workflow
/// start -> fetch_data -> process -> save -> end
async fn run_linear_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let graph = WorkflowBuilder::new("linear_workflow", "Linear Data Processing Workflow")
        .description("A simple linear data processing workflow example")
        // "A simple linear data processing workflow example"
        .start()
        .task("fetch_data", "Fetch Data", |_ctx, input| async move {
            info!("  [fetch_data] Fetching data...");
            //   [fetch_data] Fetching data...
            let data = format!("Data source: {}", input.as_str().unwrap_or("default"));
            Ok(WorkflowValue::String(data))
        })
        .task("process", "Process Data", |_ctx, input| async move {
            info!("  [process] Processing data: {:?}", input);
            //   [process] Processing data: {:?}
            let processed = format!("Processed - {}", input.as_str().unwrap_or(""));
            Ok(WorkflowValue::String(processed))
        })
        .task("save", "Save Result", |_ctx, input| async move {
            info!("  [save] Saving result: {:?}", input);
            //   [save] Saving result: {:?}
            Ok(WorkflowValue::String("Save successful".to_string()))
            // "Save successful"
        })
        .end()
        .build();

    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let result = executor
        .execute(&graph, WorkflowValue::String("API".to_string()))
        .await?;

    info!("  Workflow status: {:?}", result.status);
    //   Workflow Status: {:?}
    info!("  Number of executed nodes: {}", result.node_records.len());
    //   Number of nodes executed: {}

    Ok(())
}

/// 示例2: 条件分支工作流
/// Example 2: Conditional branch workflow
/// start -> check_value --(true)-> high_path -> end
///                       --(false)-> low_path -> end
async fn run_conditional_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 使用手动构建方式来正确处理条件分支
    // Use manual construction to handle conditional branches correctly
    let mut graph = WorkflowGraph::new("conditional_workflow", "Conditional Branch Workflow");

    graph.add_node(WorkflowNode::start("start"));
    graph.add_node(WorkflowNode::condition("check_value", "Check Value", |_ctx, input| async move {
        let value = input.as_i64().unwrap_or(0);
        info!("  [check_value] Checking value: {} (threshold: 50)", value);
        //   [check_value] Checking value: {} (Threshold: 50)
        value > 50
    }));
    graph.add_node(WorkflowNode::task("high_path", "High Value Handling", |_ctx, input| async move {
        info!("  [high_path] Executing high-value path");
        //   [high_path] Executing high-value path
        Ok(WorkflowValue::String(format!(
            "High-value processing: {}",
            input.as_i64().unwrap_or(0)
        )))
    }));
    graph.add_node(WorkflowNode::task("low_path", "Low Value Handling", |_ctx, input| async move {
        info!("  [low_path] Executing low-value path");
        //   [low_path] Executing low-value path
        Ok(WorkflowValue::String(format!(
            "Low-value processing: {}",
            input.as_i64().unwrap_or(0)
        )))
    }));
    graph.add_node(WorkflowNode::end("end"));

    // 连接节点
    // Connect nodes
    graph.connect("start", "check_value");
    graph.connect_conditional("check_value", "high_path", "true");
    graph.connect_conditional("check_value", "low_path", "false");
    graph.connect("high_path", "end");
    graph.connect("low_path", "end");

    let executor = WorkflowExecutor::new(ExecutorConfig::default());

    // 测试高值路径
    // Test high-value path
    info!("  Testing input value: 75");
    //   Testing input value: 75
    let result = executor.execute(&graph, WorkflowValue::Int(75)).await?;
    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    // 测试低值路径
    // Test low-value path
    info!("\n  Testing input value: 30");
    //   Testing input value: 30
    let result = executor.execute(&graph, WorkflowValue::Int(30)).await?;
    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    Ok(())
}

/// 示例3: 并行执行工作流
/// Example 3: Parallel execution workflow
/// start -> parallel -+-> task_a -+-> join -> end
///                    +-> task_b -+
///                    +-> task_c -+
async fn run_parallel_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let graph = WorkflowBuilder::new("parallel_workflow", "Parallel Processing Workflow")
        .description("Execute multiple tasks in parallel and then aggregate results")
        // "Execute multiple tasks in parallel and then aggregate results"
        .start()
        .parallel("fork", "Dispatch Tasks")
        .branch("task_a", "Task A", |_ctx, _input| async move {
            info!("  [task_a] Starting Task A...");
            //   [task_a] Starting task A...
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            info!("  [task_a] Task A complete");
            //   [task_a] Task A complete
            Ok(WorkflowValue::String("Result A".to_string()))
        })
        .branch("task_b", "Task B", |_ctx, _input| async move {
            info!("  [task_b] Starting Task B...");
            //   [task_b] Starting task B...
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            info!("  [task_b] Task B complete");
            //   [task_b] Task B complete
            Ok(WorkflowValue::String("Result B".to_string()))
        })
        .branch("task_c", "Task C", |_ctx, _input| async move {
            info!("  [task_c] Starting Task C...");
            //   [task_c] Starting task C...
            tokio::time::sleep(std::time::Duration::from_millis(75)).await;
            info!("  [task_c] Task C complete");
            //   [task_c] Task C complete
            Ok(WorkflowValue::String("Result C".to_string()))
        })
        .join_with_transform("join", "Aggregate Results", |results| async move {
            info!("  [join] Aggregating all results");
            //   [join] Aggregating all results
            let combined: Vec<String> = results
                .values()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            WorkflowValue::String(format!("Aggregated results: {:?}", combined))
        })
        .end()
        .build();

    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let result = executor.execute(&graph, WorkflowValue::Null).await?;

    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}
    info!("  Number of executed nodes: {}", result.node_records.len());
    //   Number of nodes executed: {}

    Ok(())
}

/// 示例4: 数据处理管道
/// Example 4: Data processing pipeline
/// 模拟 ETL 工作流: 提取 -> 转换 -> 加载
/// Simulate ETL workflow: Extract -> Transform -> Load
async fn run_data_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    let graph = WorkflowBuilder::new("data_pipeline", "ETL Data Pipeline")
        .description("Extract-Transform-Load data processing pipeline")
        // "Extract-Transform-Load data processing pipeline"
        .start()
        // 提取阶段
        // Extraction stage
        .task("extract", "Extract Data", |ctx, _input| async move {
            info!("  [extract] Extracting data from source...");
            //   [extract] Extracting data from source...
            let raw_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

            // 使用上下文变量传递元信息
            // Use context variables to pass meta-information
            ctx.set_variable("record_count", WorkflowValue::Int(raw_data.len() as i64)).await;

            let data: Vec<WorkflowValue> = raw_data
                .into_iter()
                .map(WorkflowValue::Int)
                .collect();
            Ok(WorkflowValue::List(data))
        })
        // 转换阶段
        // Transformation stage
        .task("transform", "Transform Data", |ctx, input| async move {
            info!("  [transform] Transforming data...");
            //   [transform] Transforming data...
            if let Some(list) = input.as_list() {
                let transformed: Vec<WorkflowValue> = list
                    .iter()
                    .filter_map(|v| v.as_i64())
                    .filter(|&n| n % 2 == 0) // Keep only even numbers
                    // Keep only even numbers
                    .map(|n| WorkflowValue::Int(n * 10)) // Multiply by 10
                    // Multiply by 10
                    .collect();

                ctx.set_variable(
                    "transformed_count",
                    WorkflowValue::Int(transformed.len() as i64),
                )
                .await;

                info!(
                    "  [transform] {} records remaining after filtering",
                    transformed.len()
                );
                //   [transform] {} records remaining after filtering
                Ok(WorkflowValue::List(transformed))
            } else {
                Err("Input data format error".to_string())
                // "Input data format error"
            }
        })
        // 加载阶段
        // Loading stage
        .task("load", "Load Data", |ctx, input| async move {
            info!("  [load] Loading data to target storage...");
            //   [load] Loading data to target storage...

            let original_count = ctx.get_variable("record_count").await
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let transformed_count = ctx.get_variable("transformed_count").await
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let summary = format!(
                "ETL complete: {} original records, {} after transformation",
                original_count, transformed_count
            );
            // "ETL Complete: {} original records, {} after transformation"

            info!("  [load] {}", summary);
            Ok(WorkflowValue::Map({
                let mut m = HashMap::new();
                m.insert("status".to_string(), WorkflowValue::String("success".to_string()));
                m.insert("summary".to_string(), WorkflowValue::String(summary));
                m.insert("data".to_string(), input);
                m
            }))
        })
        .end()
        .build();

    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let result = executor.execute(&graph, WorkflowValue::Null).await?;

    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    Ok(())
}

/// 示例5: 带事件监听的工作流
/// Example 5: Workflow with event listening
async fn run_workflow_with_events() -> Result<(), Box<dyn std::error::Error>> {
    // 创建事件通道
    // Create event channel
    let (event_tx, mut event_rx) = mpsc::channel::<ExecutionEvent>(100);

    // 创建简单工作流
    // Create simple workflow
    let graph = WorkflowBuilder::new("event_workflow", "Event Listening Workflow")
        .start()
        .task("step1", "Step 1", |_ctx, _input| async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(WorkflowValue::String("step1_done".to_string()))
        })
        .task("step2", "Step 2", |_ctx, _input| async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(WorkflowValue::String("step2_done".to_string()))
        })
        .task("step3", "Step 3", |_ctx, _input| async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(WorkflowValue::String("step3_done".to_string()))
        })
        .end()
        .build();

    // 创建带事件发送器的执行器
    // Create an executor with an event sender
    let executor = WorkflowExecutor::new(ExecutorConfig {
        enable_checkpoints: true,
        checkpoint_interval: 2,
        ..Default::default()
    })
    .with_event_sender(event_tx);

    // 启动事件监听任务
    // Start event monitoring task
    let _event_handle = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(event) = event_rx.recv().await {
            match &event {
                ExecutionEvent::WorkflowStarted { workflow_id, execution_id } => {
                    info!("  [EVENT] Workflow started: {} ({})", workflow_id, execution_id);
                    //   [EVENT] Workflow started: {} ({})
                }
                ExecutionEvent::NodeStarted { node_id } => {
                    info!("  [EVENT] Node started: {}", node_id);
                    //   [EVENT] Node started: {}
                }
                ExecutionEvent::NodeCompleted { node_id, result } => {
                    info!("  [EVENT] Node completed: {} - {:?}", node_id, result.status);
                    //   [EVENT] Node completed: {} - {:?}
                }
                ExecutionEvent::CheckpointCreated { label } => {
                    info!("  [EVENT] Checkpoint created: {}", label);
                    //   [EVENT] Checkpoint created: {}
                }
                ExecutionEvent::WorkflowCompleted { workflow_id, status, .. } => {
                    info!("  [EVENT] Workflow completed: {} - {:?}", workflow_id, status);
                    //   [EVENT] Workflow completed: {} - {:?}
                }
                _ => {}
            }
            events.push(event);
        }
        events
    });

    // 执行工作流
    // Execute workflow
    let result = executor.execute(&graph, WorkflowValue::Null).await?;

    // 等待事件处理完成
    // Wait for event processing to complete
    drop(executor);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    info!("  Workflow final status: {:?}", result.status);
    //   Workflow final status: {:?}

    Ok(())
}

// =============================================================================
// 示例 6-10: Dify 风格 LLM/Agent 工作流
// Examples 6-10: Dify-style LLM/Agent workflows
// =============================================================================

/// 示例6: ReAct Agent 决策工作流
/// Example 6: ReAct Agent decision workflow
///
/// 结构: start -> gather_context -> react_agent -> final_synthesis -> end
/// Structure: start -> gather_context -> react_agent -> final_synthesis -> end
///
/// 展示如何将 ReAct Agent 的推理能力集成到工作流中，
/// Shows how to integrate ReAct Agent's reasoning into the workflow,
/// LLM 节点最终综合推理结果。
/// with the LLM node finally synthesizing reasoning results.
async fn run_react_agent_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 ReAct Agent
    // Create ReAct Agent
    let react_agent = create_react_agent(
        "decision-agent",
        "You are a professional decision-making assistant who can use tools for reasoning and analysis."
    ).await?;

    // 创建用于综合分析的 LLM Agent
    // Create an LLM Agent for synthesis analysis
    let synthesis_agent = create_workflow_llm(
        "synthesis-agent",
        prompts::FINAL_SYNTHESIS
    );

    // 使用手动构建方式来集成 ReAct Agent
    // Use manual construction to integrate the ReAct Agent
    let mut graph = WorkflowGraph::new("react_agent_workflow", "ReAct Agent Decision Workflow");

    // 添加节点
    // Add nodes
    graph.add_node(WorkflowNode::start("start"));

    graph.add_node(WorkflowNode::task("gather_context", "Gather Context", |_ctx, input| async move {
        info!("  [gather_context] Gathering context information...");
        //   [gather_context] Gathering context information...
        let prompt = input.as_str().unwrap_or("");
        let context = format!("Task: {}\n\nRelated background information has been collected.", prompt);
        Ok(WorkflowValue::String(context))
    }));

    // 集成 ReAct Agent
    // Integrate ReAct Agent
    graph.add_node(WorkflowNode::task("react_agent", "ReAct Reasoning", {
        let agent_clone = Arc::clone(&react_agent);
        move |_ctx, input| {
            let agent = Arc::clone(&agent_clone);
            async move {
                info!("  [react_agent] Starting ReAct reasoning...");
                //   [react_agent] Starting ReAct reasoning...
                let task = input.as_str().unwrap_or("Please analyze the current situation");
                match agent.run(task).await {
                    Ok(result) => {
                        info!("  [react_agent] Reasoning complete, iterations: {}", result.iterations);
                        //   [react_agent] Reasoning complete, iterations: {}
                        // 构建步骤描述
                        // Build step descriptions
                        let steps_desc: Vec<String> = result.steps.iter()
                            .map(|s| {
                                let step_type_str = match s.step_type {
                                    mofa_sdk::react::ReActStepType::Thought => "Thought",
                                    // Thought
                                    mofa_sdk::react::ReActStepType::Action => "Action",
                                    // Action
                                    mofa_sdk::react::ReActStepType::Observation => "Observation",
                                    // Observation
                                    mofa_sdk::react::ReActStepType::FinalAnswer => "Final Answer",
                                    // Final Answer
                                };
                                format!("[{}] {}", step_type_str, s.content)
                            })
                            .collect();
                        Ok(WorkflowValue::String(format!(
                            "Reasoning steps:\n{}\n\nFinal answer: {}",
                            steps_desc.join("\n"),
                            result.answer
                        )))
                    }
                    Err(e) => {
                        info!("  [react_agent] Reasoning failed: {}", e);
                        //   [react_agent] Reasoning failed: {}
                        Ok(WorkflowValue::String(format!("Reasoning failed: {}", e)))
                    }
                }
            }
        }
    }));

    // LLM 节点最终综合
    // Final synthesis by LLM node
    graph.add_node(WorkflowNode::llm_agent(
        "final_synthesis",
        "Final Synthesis",
        synthesis_agent
    ));

    graph.add_node(WorkflowNode::end("end"));

    // 连接节点
    // Connect nodes
    graph.connect("start", "gather_context");
    graph.connect("gather_context", "react_agent");
    graph.connect("react_agent", "final_synthesis");
    graph.connect("final_synthesis", "end");

    // 执行工作流
    // Execute workflow
    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let input = WorkflowValue::String(
        "Calculate the result of 123 * 456.".to_string()
    );
    let result = executor.execute(&graph, input).await?;

    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    Ok(())
}

/// 示例7: 多 Agent 并行分析工作流
/// Example 7: Multi-Agent parallel analysis workflow
///
/// 结构:
/// Structure:
///         -> technical_agent ->
/// start ->                     -> join -> final_synthesis -> end
///         -> business_agent ->
///
/// 展示多个专家 LLM Agent 并行分析不同视角，
/// Shows multiple expert LLM Agents analyzing different perspectives in parallel,
/// LLM 节点综合多视角意见。
/// with the LLM node synthesizing multi-perspective opinions.
async fn run_multi_agent_parallel_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 创建两个专家 Agent
    // Create two expert Agents
    let technical_agent = create_workflow_llm(
        "technical-expert",
        prompts::TECHNICAL_ANALYSIS
    );

    let business_agent = create_workflow_llm(
        "business-expert",
        prompts::BUSINESS_ANALYSIS
    );

    // 创建综合分析 Agent
    // Create a synthesis analysis Agent
    let synthesis_agent = create_workflow_llm(
        "synthesis-agent",
        prompts::MULTI_PERSPECTIVE_SYNTHESIS
    );

    // 构建并行工作流
    // Build parallel workflow
    let graph = WorkflowBuilder::new("multi_agent_workflow", "Multi-Agent Parallel Analysis Workflow")
        .description("Execute multiple expert Agents in parallel and then synthesize opinions")
        // "Execute multiple expert Agents in parallel and then synthesize opinions"
        .start()
        .parallel("fork", "Distribute Analysis Tasks")
        // 技术专家分支
        // Technical expert branch
        .llm_agent_branch("technical_agent", "Technical Analysis", technical_agent)
        // 商业专家分支
        // Business expert branch
        .llm_agent_branch("business_agent", "Business Analysis", business_agent)
        // 聚合结果
        // Aggregate results
        .join_with_transform("join", "Aggregate Analysis Results", |results| async move {
            info!("  [join] Aggregating multi-perspective analysis results");
            //   [join] Aggregating multi-perspective analysis results
            let technical = results.get("technical_agent")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let business = results.get("business_agent")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            WorkflowValue::Map({
                let mut m = std::collections::HashMap::new();
                m.insert("technical".to_string(), WorkflowValue::String(technical.to_string()));
                m.insert("business".to_string(), WorkflowValue::String(business.to_string()));
                m
            })
        })
        // LLM 节点综合分析
        // LLM node synthesis analysis
        .llm_agent_with_template(
            "final_synthesis",
            "Comprehensive Decision Recommendation",
            synthesis_agent,
            prompts::MULTI_PERSPECTIVE_SYNTHESIS.to_string()
        )
        .end()
        .build();

    // 执行工作流
    // Execute workflow
    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let input = WorkflowValue::String(
        "Develop an AI Agent framework based on Rust.".to_string()
    );
    let result = executor.execute(&graph, input).await?;

    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    Ok(())
}

/// 示例8: 条件路由 + LLM 决策工作流
/// Example 8: Conditional routing + LLM decision workflow
///
/// 结构: start -> complexity_check --(simple)--> simple_processing --> final_decision --> end
/// Structure: start -> complexity_check --(simple)--> simple_processing --> final_decision --> end
//                                  --(complex)--> deep_analysis ------|
///
/// 展示 LLM 分类输入复杂度，根据分类走不同路径，
/// Shows LLM classifying input complexity, following different paths based on classification,
/// LLM 节点提供最终决策。
/// with the LLM node providing the final decision.
async fn run_conditional_llm_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 创建分类 Agent
    // Create classification Agent
    let classifier_agent = create_workflow_llm(
        "classifier",
        prompts::COMPLEXITY_CHECK
    );

    // 创建简单处理 Agent
    // Create simple processing Agent
    let simple_agent = create_workflow_llm(
        "simple-handler",
        prompts::SIMPLE_PROCESSING
    );

    // 创建深度分析 Agent
    // Create deep analysis Agent
    let deep_agent = create_workflow_llm(
        "deep-analyzer",
        prompts::DEEP_ANALYSIS
    );

    // 创建最终决策 Agent
    // Create final decision Agent
    let decision_agent = create_workflow_llm(
        "decision-maker",
        prompts::FINAL_DECISION
    );

    // 手动构建带条件分支的工作流
    // Manually build workflow with conditional branches
    let mut graph = WorkflowGraph::new("conditional_llm_workflow", "Conditional Routing + LLM Decision Workflow");

    // 添加节点
    // Add nodes
    graph.add_node(WorkflowNode::start("start"));

    // LLM 分类节点
    // LLM classification node
    graph.add_node(WorkflowNode::llm_agent("complexity_check", "Complexity Classification", classifier_agent));

    // 条件分支
    // Conditional branch
    graph.add_node(WorkflowNode::condition("check_route", "Check Classification Result", |_ctx, input| async move {
        let mut response = input.as_str().unwrap_or("").to_lowercase();
        info!("  [check_route] Classification result: {}", response);
        //   [check_route] Classification result: {}
        response.contains("complex")
    }));

    // 简单处理分支
    // Simple processing branch
    graph.add_node(WorkflowNode::llm_agent("simple_processing", "Simple Processing", simple_agent));

    // 深度分析分支
    // Deep analysis branch
    graph.add_node(WorkflowNode::llm_agent("deep_analysis", "Deep Analysis", deep_agent));

    // 最终决策节点
    // Final decision node
    graph.add_node(WorkflowNode::llm_agent("final_decision", "Final Decision", decision_agent));

    graph.add_node(WorkflowNode::end("end"));

    // 连接节点
    // Connect nodes
    graph.connect("start", "complexity_check");
    graph.connect("complexity_check", "check_route");
    graph.connect_conditional("check_route", "simple_processing", "false");
    graph.connect_conditional("check_route", "deep_analysis", "true");
    graph.connect("simple_processing", "final_decision");
    graph.connect("deep_analysis", "final_decision");
    graph.connect("final_decision", "end");

    // 执行工作流
    // Execute workflow
    let executor = WorkflowExecutor::new(ExecutorConfig::default());

    // 测试简单任务
    // Test simple task
    info!("  Testing simple task: \"What is Rust?\"");
    //   Testing simple task: "What is Rust?"
    let result = executor.execute(
        &graph,
        WorkflowValue::String("What is Rust?".to_string())
    ).await?;
    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    // 测试复杂任务
    // Test complex task
    info!("\n  Testing complex task: \"Design a high-concurrency distributed system architecture supporting 1M RPS\"");
    //   Testing complex task: "Design a high-concurrency distributed system architecture supporting 1M RPS"
    let result = executor.execute(
        &graph,
        WorkflowValue::String("Design a high-concurrency distributed system architecture supporting 1M RPS".to_string())
    ).await?;
    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    Ok(())
}

/// 示例9: 智能数据管道工作流
/// Example 9: Intelligent data pipeline workflow
///
/// 结构: start -> extract -> transform -> llm_analysis -> end
/// Structure: start -> extract -> transform -> llm_analysis -> end
///
/// 展示 ETL 管道处理数据后，LLM 节点进行智能分析和洞察生成。
/// Shows an ETL pipeline processing data, followed by an LLM node for intelligent analysis and insight generation.
async fn run_intelligent_pipeline_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 创建分析 Agent
    // Create analysis Agent
    let analysis_agent = create_workflow_llm(
        "data-analyst",
        prompts::LLM_ANALYSIS
    );

    // 构建智能数据管道
    // Build intelligent data pipeline
    let graph = WorkflowBuilder::new("intelligent_pipeline", "Intelligent Data Pipeline")
        .description("ETL Pipeline + LLM Intelligent Analysis")
        // "ETL Pipeline + LLM Intelligent Analysis"
        .start()
        // 提取阶段：获取销售数据
        // Extraction stage: Obtain sales data
        .task("extract", "Extract Sales Data", |_ctx, _input| async move {
            info!("  [extract] Extracting sales data from database...");
            //   [extract] Extracting sales data from database...
            let sales_data = vec![
                ("Q1", 150000), ("Q2", 180000), ("Q3", 210000), ("Q4", 280000)
            ];
            let data_str = format!("{:?}", sales_data);
            Ok(WorkflowValue::String(data_str))
        })
        // 转换阶段：计算同比增长
        // Transformation stage: Calculate year-over-year growth
        .task("transform", "Transform Data", |_ctx, input| async move {
            info!("  [transform] Calculating quarterly growth rates...");
            //   [transform] Calculating quarterly growth rates...
            let data_str = input.as_str().unwrap_or("");
            let transformed = format!(
                "{}\n\nTransformed result: quarterly growth rates Q2=+20%, Q3=+16.7%, Q4=+33.3%",
                data_str
            );
            Ok(WorkflowValue::String(transformed))
        })
        // LLM 分析阶段：生成洞察
        // LLM analysis stage: Generate insights
        .llm_agent_with_template(
            "llm_analysis",
            "Intelligent Insight Analysis",
            analysis_agent,
            prompts::LLM_ANALYSIS.to_string()
        )
        .end()
        .build();

    // 执行工作流
    // Execute workflow
    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let result = executor.execute(&graph, WorkflowValue::Null).await?;

    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    Ok(())
}

/// 示例10: 工具链 + LLM 总结工作流
/// Example 10: Tool chain + LLM summary workflow
///
/// 结构: start -> (calculator, datetime) -> join -> llm_summary -> end
/// Structure: start -> (calculator, datetime) -> join -> llm_summary -> end
///
/// 展示并行执行多个工具，LLM 节点综合工具结果。
/// Shows parallel execution of multiple tools, with the LLM node synthesizing the tool results.
async fn run_tool_chain_llm_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 创建总结 Agent
    // Create summary Agent
    let summary_agent = create_workflow_llm(
        "summarizer",
        prompts::LLM_SUMMARY
    );

    // 构建工具链工作流
    // Build tool chain workflow
    let graph = WorkflowBuilder::new("tool_chain_workflow", "Tool Chain + LLM Summary")
        .description("Execute tools in parallel and synthesize results with LLM")
        // "Execute tools in parallel and synthesize results with LLM"
        .start()
        .parallel("fork", "Dispatch Tool Calls")
        // 计算工具分支
        // Calculator tool branch
        .branch("calculator", "Calculator", |_ctx, _input| async move {
            info!("  [calculator] Performing calculation: 123 * 456");
            //   [calculator] Performing calculation: 123 * 456
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let result = 123 * 456;
            Ok(WorkflowValue::String(format!("Calculation result: {}", result)))
        })
        // 日期时间工具分支
        // Date-time tool branch
        .branch("datetime", "DateTime", |_ctx, _input| async move {
            info!("  [datetime] Getting current time");
            //   [datetime] Getting current time
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            // 简单转换为可读格式
            // Simply convert to readable format
            let days = now / 86400;
            let hours = (now % 86400) / 3600;
            let minutes = (now % 3600) / 60;
            let seconds = now % 60;
            Ok(WorkflowValue::String(format!(
                "Unix timestamp: {} (about {} days {} hours {} minutes {} seconds)",
                now, days, hours, minutes, seconds
            )))
        })
        // 聚合工具结果
        // Aggregate tool results
        .join_with_transform("join", "Aggregate Tool Results", |results| async move {
            info!("  [join] Aggregating tool execution results");
            //   [join] Aggregating tool execution results
            let calc = results.get("calculator")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let datetime = results.get("datetime")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            WorkflowValue::Map({
                let mut m = std::collections::HashMap::new();
                m.insert("calc".to_string(), WorkflowValue::String(calc.to_string()));
                m.insert("datetime".to_string(), WorkflowValue::String(datetime.to_string()));
                m
            })
        })
        // LLM 总结节点
        // LLM summary node
        .llm_agent_with_template(
            "llm_summary",
            "Comprehensive Summary",
            summary_agent,
            prompts::LLM_SUMMARY.to_string()
        )
        .end()
        .build();

    // 执行工作流
    // Execute workflow
    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let result = executor.execute(&graph, WorkflowValue::Null).await?;

    info!("  Workflow status: {:?}", result.status);
    //   Workflow status: {:?}

    Ok(())
}
