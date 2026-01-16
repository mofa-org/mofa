//! 工作流编排示例
//!
//! 演示 Graph-based workflow orchestration 功能:
//! 1. 使用 WorkflowBuilder 流式构建工作流
//! 2. 条件分支执行
//! 3. 并行分支和聚合
//! 4. 状态管理和数据传递
//! 5. 执行事件监听
//!
//! 运行: cargo run --example workflow_orchestration

use mofa_sdk::{ExecutionEvent, ExecutorConfig, WorkflowBuilder, WorkflowExecutor, WorkflowGraph, WorkflowNode, WorkflowValue};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 设置日志
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== MoFA 工作流编排示例 ===\n");

    // 示例1: 简单的线性工作流
    info!("--- 示例1: 线性工作流 ---");
    run_linear_workflow().await?;

    // 示例2: 条件分支工作流
    info!("\n--- 示例2: 条件分支工作流 ---");
    run_conditional_workflow().await?;

    // 示例3: 并行执行工作流
    info!("\n--- 示例3: 并行执行工作流 ---");
    run_parallel_workflow().await?;

    // 示例4: 数据处理管道
    info!("\n--- 示例4: 数据处理管道 ---");
    run_data_pipeline().await?;

    // 示例5: 带事件监听的工作流
    info!("\n--- 示例5: 事件监听工作流 ---");
    run_workflow_with_events().await?;

    info!("\n=== 所有示例执行完成 ===");
    Ok(())
}

/// 示例1: 简单的线性工作流
/// start -> fetch_data -> process -> save -> end
async fn run_linear_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let graph = WorkflowBuilder::new("linear_workflow", "线性数据处理工作流")
        .description("一个简单的线性数据处理工作流示例")
        .start()
        .task("fetch_data", "获取数据", |_ctx, input| async move {
            info!("  [fetch_data] 获取数据中...");
            let data = format!("数据来源: {}", input.as_str().unwrap_or("default"));
            Ok(WorkflowValue::String(data))
        })
        .task("process", "处理数据", |_ctx, input| async move {
            info!("  [process] 处理数据: {:?}", input);
            let processed = format!("已处理 - {}", input.as_str().unwrap_or(""));
            Ok(WorkflowValue::String(processed))
        })
        .task("save", "保存结果", |_ctx, input| async move {
            info!("  [save] 保存结果: {:?}", input);
            Ok(WorkflowValue::String("保存成功".to_string()))
        })
        .end()
        .build();

    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let result = executor
        .execute(&graph, WorkflowValue::String("API".to_string()))
        .await?;

    info!("  工作流状态: {:?}", result.status);
    info!("  执行的节点数: {}", result.node_records.len());

    Ok(())
}

/// 示例2: 条件分支工作流
/// start -> check_value --(true)-> high_path -> end
///                      --(false)-> low_path -> end
async fn run_conditional_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // 使用手动构建方式来正确处理条件分支
    let mut graph = WorkflowGraph::new("conditional_workflow", "条件分支工作流");

    graph.add_node(WorkflowNode::start("start"));
    graph.add_node(WorkflowNode::condition("check_value", "检查值大小", |_ctx, input| async move {
        let value = input.as_i64().unwrap_or(0);
        info!("  [check_value] 检查值: {} (阈值: 50)", value);
        value > 50
    }));
    graph.add_node(WorkflowNode::task("high_path", "高值处理", |_ctx, input| async move {
        info!("  [high_path] 执行高值路径");
        Ok(WorkflowValue::String(format!("高值处理: {}", input.as_i64().unwrap_or(0))))
    }));
    graph.add_node(WorkflowNode::task("low_path", "低值处理", |_ctx, input| async move {
        info!("  [low_path] 执行低值路径");
        Ok(WorkflowValue::String(format!("低值处理: {}", input.as_i64().unwrap_or(0))))
    }));
    graph.add_node(WorkflowNode::end("end"));

    // 连接节点
    graph.connect("start", "check_value");
    graph.connect_conditional("check_value", "high_path", "true");
    graph.connect_conditional("check_value", "low_path", "false");
    graph.connect("high_path", "end");
    graph.connect("low_path", "end");

    let executor = WorkflowExecutor::new(ExecutorConfig::default());

    // 测试高值路径
    info!("  测试输入值: 75");
    let result = executor.execute(&graph, WorkflowValue::Int(75)).await?;
    info!("  工作流状态: {:?}", result.status);

    // 测试低值路径
    info!("\n  测试输入值: 30");
    let result = executor.execute(&graph, WorkflowValue::Int(30)).await?;
    info!("  工作流状态: {:?}", result.status);

    Ok(())
}

/// 示例3: 并行执行工作流
/// start -> parallel -+-> task_a -+-> join -> end
///                    +-> task_b -+
///                    +-> task_c -+
async fn run_parallel_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let graph = WorkflowBuilder::new("parallel_workflow", "并行处理工作流")
        .description("并行执行多个任务然后聚合结果")
        .start()
        .parallel("fork", "分发任务")
        .branch("task_a", "任务A", |_ctx, _input| async move {
            info!("  [task_a] 开始执行任务A...");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            info!("  [task_a] 任务A完成");
            Ok(WorkflowValue::String("结果A".to_string()))
        })
        .branch("task_b", "任务B", |_ctx, _input| async move {
            info!("  [task_b] 开始执行任务B...");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            info!("  [task_b] 任务B完成");
            Ok(WorkflowValue::String("结果B".to_string()))
        })
        .branch("task_c", "任务C", |_ctx, _input| async move {
            info!("  [task_c] 开始执行任务C...");
            tokio::time::sleep(std::time::Duration::from_millis(75)).await;
            info!("  [task_c] 任务C完成");
            Ok(WorkflowValue::String("结果C".to_string()))
        })
        .join_with_transform("join", "聚合结果", |results| async move {
            info!("  [join] 聚合所有结果");
            let combined: Vec<String> = results
                .values()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            WorkflowValue::String(format!("聚合结果: {:?}", combined))
        })
        .end()
        .build();

    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let result = executor.execute(&graph, WorkflowValue::Null).await?;

    info!("  工作流状态: {:?}", result.status);
    info!("  执行的节点数: {}", result.node_records.len());

    Ok(())
}

/// 示例4: 数据处理管道
/// 模拟 ETL 工作流: 提取 -> 转换 -> 加载
async fn run_data_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    let graph = WorkflowBuilder::new("data_pipeline", "ETL数据管道")
        .description("Extract-Transform-Load 数据处理管道")
        .start()
        // 提取阶段
        .task("extract", "提取数据", |ctx, _input| async move {
            info!("  [extract] 从数据源提取数据...");
            let raw_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

            // 使用上下文变量传递元信息
            ctx.set_variable("record_count", WorkflowValue::Int(raw_data.len() as i64)).await;

            let data: Vec<WorkflowValue> = raw_data
                .into_iter()
                .map(WorkflowValue::Int)
                .collect();
            Ok(WorkflowValue::List(data))
        })
        // 转换阶段
        .task("transform", "转换数据", |ctx, input| async move {
            info!("  [transform] 转换数据...");
            if let Some(list) = input.as_list() {
                let transformed: Vec<WorkflowValue> = list
                    .iter()
                    .filter_map(|v| v.as_i64())
                    .filter(|&n| n % 2 == 0) // 只保留偶数
                    .map(|n| WorkflowValue::Int(n * 10)) // 乘以10
                    .collect();

                ctx.set_variable("transformed_count", WorkflowValue::Int(transformed.len() as i64)).await;

                info!("  [transform] 过滤后剩余 {} 条记录", transformed.len());
                Ok(WorkflowValue::List(transformed))
            } else {
                Err("输入数据格式错误".to_string())
            }
        })
        // 加载阶段
        .task("load", "加载数据", |ctx, input| async move {
            info!("  [load] 加载数据到目标存储...");

            let original_count = ctx.get_variable("record_count").await
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let transformed_count = ctx.get_variable("transformed_count").await
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let summary = format!(
                "ETL完成: 原始记录 {} 条, 转换后 {} 条",
                original_count, transformed_count
            );

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

    info!("  工作流状态: {:?}", result.status);

    Ok(())
}

/// 示例5: 带事件监听的工作流
async fn run_workflow_with_events() -> Result<(), Box<dyn std::error::Error>> {
    // 创建事件通道
    let (event_tx, mut event_rx) = mpsc::channel::<ExecutionEvent>(100);

    // 创建简单工作流
    let graph = WorkflowBuilder::new("event_workflow", "事件监听工作流")
        .start()
        .task("step1", "步骤1", |_ctx, _input| async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(WorkflowValue::String("step1_done".to_string()))
        })
        .task("step2", "步骤2", |_ctx, _input| async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(WorkflowValue::String("step2_done".to_string()))
        })
        .task("step3", "步骤3", |_ctx, _input| async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(WorkflowValue::String("step3_done".to_string()))
        })
        .end()
        .build();

    // 创建带事件发送器的执行器
    let executor = WorkflowExecutor::new(ExecutorConfig {
        enable_checkpoints: true,
        checkpoint_interval: 2,
        ..Default::default()
    })
    .with_event_sender(event_tx);

    // 启动事件监听任务
    let _event_handle = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(event) = event_rx.recv().await {
            match &event {
                ExecutionEvent::WorkflowStarted { workflow_id, execution_id } => {
                    info!("  [EVENT] 工作流开始: {} ({})", workflow_id, execution_id);
                }
                ExecutionEvent::NodeStarted { node_id } => {
                    info!("  [EVENT] 节点开始: {}", node_id);
                }
                ExecutionEvent::NodeCompleted { node_id, result } => {
                    info!("  [EVENT] 节点完成: {} - {:?}", node_id, result.status);
                }
                ExecutionEvent::CheckpointCreated { label } => {
                    info!("  [EVENT] 检查点创建: {}", label);
                }
                ExecutionEvent::WorkflowCompleted { workflow_id, status, .. } => {
                    info!("  [EVENT] 工作流完成: {} - {:?}", workflow_id, status);
                }
                _ => {}
            }
            events.push(event);
        }
        events
    });

    // 执行工作流
    let result = executor.execute(&graph, WorkflowValue::Null).await?;

    // 等待事件处理完成
    drop(executor);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    info!("  工作流最终状态: {:?}", result.status);

    Ok(())
}
