//！ 多Agent自适应协作协议示例
//!
//! 本示例演示如何使用 MoFA 框架的 LLM 驱动协作能力。
//!
//! 核心理念：
//! - **所有协作决策由 LLM 驱动**：任务分析、模式选择、消息处理都可以使用 LLM
//! - **协议支持 LLM 集成**：每个协议可以选择性地使用 LLM 来智能处理消息
//! - **框架提供标准协议**：RequestResponse, PublishSubscribe, Consensus, Debate, Parallel, Sequential
//!
//! # 架构说明
//!
//! 本示例遵循微内核架构：
//! - **mofa-kernel**: 提供核心抽象（CollaborationProtocol trait, CollaborationMode, CollaborationContent）
//! - **mofa-foundation**: 提供标准协议实现（支持可选的 LLM 集成）
//! - **mofa-sdk**: 导出统一的 API

use anyhow::Result;
use std::sync::Arc;
use mofa_sdk::collaboration::LLMDrivenCollaborationManager;
use mofa_sdk::collaboration::{
    ConsensusProtocol, DebateProtocol, ParallelProtocol, PublishSubscribeProtocol,
    RequestResponseProtocol, SequentialProtocol,
};
use tracing::{info, Level};
// ============================================================================
// 主函数 - 演示多Agent协作（可选 LLM 驱动）
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("=== 多Agent协作协议演示 ===");
    info!("注意: 本示例展示基础协议功能，不包含 LLM 集成");
    info!("要使用 LLM 驱动，请创建协议时使用 .with_llm() 方法\n");

    // 创建协作管理器
    let manager = LLMDrivenCollaborationManager::new("collaboration_agent");

    // 注册标准协作协议（不使用 LLM）
    info!("注册标准协作协议...");
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

    // 获取所有协议的描述（供 LLM 理解）
    let descriptions = manager.get_protocol_descriptions().await;
    info!("可用的协作协议：");
    for (name, desc) in &descriptions {
        info!("  - {}: {}", name, desc.description);
        info!("    适用场景: {:?}", desc.scenarios);
    }
    info!("");

    // 测试请求-响应协议
    info!("\n--- 测试：请求-响应协议 ---");
    let rr_result = manager
        .execute_task_with_protocol("request_response", "请处理这个数据查询请求")
        .await?;
    info!(
        "结果: {}",
        rr_result.data.as_ref().map(|d| d.to_text()).unwrap_or_default()
    );
    info!("耗时: {}ms", rr_result.duration_ms);
    info!("模式: {}", rr_result.mode);

    // 测试发布-订阅协议
    info!("\n--- 测试：发布-订阅协议 ---");
    let ps_result = manager
        .execute_task_with_protocol("publish_subscribe", "分享这个创意想法给大家")
        .await?;
    info!(
        "结果: {}",
        ps_result.data.as_ref().map(|d| d.to_text()).unwrap_or_default()
    );
    info!("耗时: {}ms", ps_result.duration_ms);
    info!("模式: {}", ps_result.mode);

    // 测试共识协议
    info!("\n--- 测试：共识协议 ---");
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
    info!("耗时: {}ms", consensus_result.duration_ms);
    info!("模式: {}", consensus_result.mode);

    // 测试辩论协议
    info!("\n--- 测试：辩论协议 ---");
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
    info!("耗时: {}ms", debate_result.duration_ms);
    info!("模式: {}", debate_result.mode);

    // 测试并行协议
    info!("\n--- 测试：并行协议 ---");
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
    info!("耗时: {}ms", parallel_result.duration_ms);
    info!("模式: {}", parallel_result.mode);

    // 测试顺序协议
    info!("\n--- 测试：顺序协议 ---");
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
    info!("耗时: {}ms", sequential_result.duration_ms);
    info!("模式: {}", sequential_result.mode);

    // 显示统计信息
    info!("\n=== 协作统计信息 ===");
    let stats = manager.stats().await;
    info!("总任务数: {}", stats.total_tasks);
    info!("成功任务数: {}", stats.successful_tasks);
    info!("失败任务数: {}", stats.failed_tasks);
    info!("平均执行时间: {:.2}ms", stats.avg_duration_ms);
    info!("模式使用分布: {:?}", stats.mode_usage);

    info!("\n=== LLM 驱动协作说明 ===");
    info!("要让协议使用 LLM，可以这样创建：");
    info!("  let protocol = RequestResponseProtocol::with_llm(");
    info!("      \"agent_id\",");
    info!("      Arc::new(LLMClient::new(provider))");
    info!("  );");
    info!("\n这样协议会使用 LLM 来：");
    info!("  1. 理解和处理自然语言消息");
    info!("  2. 提供智能的协作响应");
    info!("  3. 记录 LLM 的推理过程");

    info!("\n=== 演示完成 ===");

    Ok(())
}
