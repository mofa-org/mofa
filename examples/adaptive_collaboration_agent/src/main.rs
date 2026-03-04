//！ 多Agent自适应协作协议示例
//! Multi-Agent Adaptive Collaboration Protocol Example
//!
//! 本示例演示如何使用 MoFA 框架的 LLM 驱动协作能力。
//! This example demonstrates how to use the LLM-driven collaboration capabilities of the MoFA framework.
//!
//! 核心理念：
//! Core Concepts:
//! - **所有协作决策由 LLM 驱动**：任务分析、模式选择、消息处理都可以使用 LLM
//! - **All collaboration decisions are LLM-driven**: Task analysis, mode selection, and message processing can all use LLM
//! - **协议支持 LLM 集成**：每个协议可以选择性地使用 LLM 来智能处理消息
//! - **Protocols support LLM integration**: Each protocol can optionally use LLM to intelligently handle messages
//! - **框架提供标准协议**：RequestResponse, PublishSubscribe, Consensus, Debate, Parallel, Sequential
//! - **Framework provides standard protocols**: RequestResponse, PublishSubscribe, Consensus, Debate, Parallel, Sequential
//!
//! # 架构说明
//! # Architecture Description
//!
//! 本示例遵循微内核架构：
//! This example follows a microkernel architecture:
//! - **mofa-kernel**: 提供核心抽象（CollaborationProtocol trait, CollaborationMode, CollaborationContent）
//! - **mofa-kernel**: Provides core abstractions (CollaborationProtocol trait, CollaborationMode, CollaborationContent)
//! - **mofa-foundation**: 提供标准协议实现（支持可选的 LLM 集成）
//! - **mofa-foundation**: Provides standard protocol implementations (supporting optional LLM integration)
//! - **mofa-sdk**: 导出统一的 API
//! - **mofa-sdk**: Exports a unified API

use std::sync::Arc;
use mofa_sdk::collaboration::LLMDrivenCollaborationManager;
use mofa_sdk::collaboration::{
    ConsensusProtocol, DebateProtocol, ParallelProtocol, PublishSubscribeProtocol,
    RequestResponseProtocol, SequentialProtocol,
};
use tracing::{info, Level};
// ============================================================================
// 主函数 - 演示多Agent协作（可选 LLM 驱动）
// Main Function - Demonstrating Multi-Agent Collaboration (Optional LLM-driven)
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("=== 多Agent协作协议演示 ===");
    info!("=== Multi-Agent Collaboration Protocol Demo ===");
    info!("注意: 本示例展示基础协议功能，不包含 LLM 集成");
    info!("Note: This example shows basic protocol features without LLM integration");
    info!("要使用 LLM 驱动，请创建协议时使用 .with_llm() 方法\n");
    info!("To use LLM-driven mode, use the .with_llm() method when creating protocols\n");

    // 创建协作管理器
    // Create collaboration manager
    let manager = LLMDrivenCollaborationManager::new("collaboration_agent");

    // 注册标准协作协议（不使用 LLM）
    // Register standard collaboration protocols (without using LLM)
    info!("注册标准协作协议...");
    info!("Registering standard collaboration protocols...");
    manager
        .register_protocol(Arc::new(RequestResponseProtocol::new("collaboration_agent")))
        .await?;
    manager
        .register_protocol(Arc::new(PublishSubscribeProtocol::new("collaboration_agent")))
        .await?;
    manager
        .register_protocol(Arc::new(ConsensusProtocol::new("collaboration_agent")))
        .await?;
    manager
        .register_protocol(Arc::new(DebateProtocol::new("collaboration_agent")))
        .await?;
    manager
        .register_protocol(Arc::new(ParallelProtocol::new("collaboration_agent")))
        .await?;
    manager
        .register_protocol(Arc::new(SequentialProtocol::new("collaboration_agent")))
        .await?;
    info!("已注册 {} 个协作协议\n", manager.registry().count().await);
    info!("Registered {} collaboration protocols\n", manager.registry().count().await);

    // 获取所有协议的描述（供 LLM 理解）
    // Get descriptions for all protocols (for LLM understanding)
    let descriptions = manager.get_protocol_descriptions().await;
    info!("可用的协作协议：");
    info!("Available collaboration protocols:");
    for (name, desc) in &descriptions {
        info!("  - {}: {}", name, desc.description);
        info!("    适用场景: {:?}", desc.scenarios);
        info!("    Applicable scenarios: {:?}", desc.scenarios);
    }
    info!("");

    // 测试请求-响应协议
    // Test Request-Response protocol
    info!("\n--- 测试：请求-响应协议 ---");
    info!("\n--- Test: Request-Response Protocol ---");
    let rr_result = manager
        .execute_task_with_protocol("request_response", "请处理这个数据查询请求")
        .await?;
    info!(
        "结果: {}",
        rr_result.data.as_ref().map(|d| d.to_text()).unwrap_or_default()
    );
    info!(
        "Result: {}",
        rr_result.data.as_ref().map(|d| d.to_text()).unwrap_or_default()
    );
    info!("耗时: {}ms", rr_result.duration_ms);
    info!("Duration: {}ms", rr_result.duration_ms);
    info!("模式: {}", rr_result.mode);
    info!("Mode: {}", rr_result.mode);

    // 测试发布-订阅协议
    // Test Publish-Subscribe protocol
    info!("\n--- 测试：发布-订阅协议 ---");
    info!("\n--- Test: Publish-Subscribe Protocol ---");
    let ps_result = manager
        .execute_task_with_protocol("publish_subscribe", "分享这个创意想法给大家")
        .await?;
    info!(
        "结果: {}",
        ps_result.data.as_ref().map(|d| d.to_text()).unwrap_or_default()
    );
    info!(
        "Result: {}",
        ps_result.data.as_ref().map(|d| d.to_text()).unwrap_or_default()
    );
    info!("耗时: {}ms", ps_result.duration_ms);
    info!("Duration: {}ms", ps_result.duration_ms);
    info!("模式: {}", ps_result.mode);
    info!("Mode: {}", ps_result.mode);

    // 测试共识协议
    // Test Consensus protocol
    info!("\n--- 测试：共识协议 ---");
    info!("\n--- Test: Consensus Protocol ---");
    let consensus_result = manager
        .execute_task_with_protocol("consensus", "我们需要对这个方案达成一致意见")
        .await?;
    info!(
        "结果: {}",
        consensus_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!(
        "Result: {}",
        consensus_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!("耗时: {}ms", consensus_result.duration_ms);
    info!("Duration: {}ms", consensus_result.duration_ms);
    info!("模式: {}", consensus_result.mode);
    info!("Mode: {}", consensus_result.mode);

    // 测试辩论协议
    // Test Debate protocol
    info!("\n--- 测试：辩论协议 ---");
    info!("\n--- Test: Debate Protocol ---");
    let debate_result = manager
        .execute_task_with_protocol("debate", "审查这段代码并提出改进建议")
        .await?;
    info!(
        "结果: {}",
        debate_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!(
        "Result: {}",
        debate_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!("耗时: {}ms", debate_result.duration_ms);
    info!("Duration: {}ms", debate_result.duration_ms);
    info!("模式: {}", debate_result.mode);
    info!("Mode: {}", debate_result.mode);

    // 测试并行协议
    // Test Parallel protocol
    info!("\n--- 测试：并行协议 ---");
    info!("\n--- Test: Parallel Protocol ---");
    let parallel_result = manager
        .execute_task_with_protocol("parallel", "同时分析这三个独立的数据集")
        .await?;
    info!(
        "结果: {}",
        parallel_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!(
        "Result: {}",
        parallel_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!("耗时: {}ms", parallel_result.duration_ms);
    info!("Duration: {}ms", parallel_result.duration_ms);
    info!("模式: {}", parallel_result.mode);
    info!("Mode: {}", parallel_result.mode);

    // 测试顺序协议
    // Test Sequential protocol
    info!("\n--- 测试：顺序协议 ---");
    info!("\n--- Test: Sequential Protocol ---");
    let sequential_result = manager
        .execute_task_with_protocol("sequential", "按顺序执行这个工作流的各个步骤")
        .await?;
    info!(
        "结果: {}",
        sequential_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!(
        "Result: {}",
        sequential_result
            .data
            .as_ref()
            .map(|d| d.to_text())
            .unwrap_or_default()
    );
    info!("耗时: {}ms", sequential_result.duration_ms);
    info!("Duration: {}ms", sequential_result.duration_ms);
    info!("模式: {}", sequential_result.mode);
    info!("Mode: {}", sequential_result.mode);

    // 显示统计信息
    // Show statistics information
    info!("\n=== 协作统计信息 ===");
    info!("\n=== Collaboration Statistics ===");
    let stats = manager.stats().await;
    info!("总任务数: {}", stats.total_tasks);
    info!("Total tasks: {}", stats.total_tasks);
    info!("成功任务数: {}", stats.successful_tasks);
    info!("Successful tasks: {}", stats.successful_tasks);
    info!("失败任务数: {}", stats.failed_tasks);
    info!("Failed tasks: {}", stats.failed_tasks);
    info!("平均执行时间: {:.2}ms", stats.avg_duration_ms);
    info!("Average execution time: {:.2}ms", stats.avg_duration_ms);
    info!("模式使用分布: {:?}", stats.mode_usage);
    info!("Mode usage distribution: {:?}", stats.mode_usage);

    info!("\n=== LLM 驱动协作说明 ===");
    info!("\n=== LLM-Driven Collaboration Notes ===");
    info!("要让协议使用 LLM，可以这样创建：");
    info!("To enable LLM for a protocol, create it like this:");
    info!("  let protocol = RequestResponseProtocol::with_llm(");
    info!("      \"agent_id\",");
    info!("      Arc::new(LLMClient::new(provider))");
    info!("  );");
    info!("\n这样协议会使用 LLM 来：");
    info!("\nThis way the protocol will use LLM to:");
    info!("  1. 理解和处理自然语言消息");
    info!("  1. Understand and process natural language messages");
    info!("  2. 提供智能的协作响应");
    info!("  2. Provide intelligent collaborative responses");
    info!("  3. 记录 LLM 的推理过程");
    info!("  3. Record the LLM reasoning process");

    info!("\n=== 演示完成 ===");
    info!("\n=== Demo Completed ===");

    Ok(())
}
