//!/usr/bin/env cargo run --release
//! 秘书Agent模式完整用例示例 - 基于LLM交互的版本
//!
//! 本示例展示秘书Agent的完整工作循环，并结合LLM进行智能交互：
//! 1. 接收想法 → 使用LLM记录Todo
//! 2. 澄清需求 → 使用LLM转换为项目文档
//! 3. 调度分配 → 调用执行Agent
//!
//! 该示例使用了最新的API版本。

use mofa_sdk::secretary::{
    AgentInfo, ChannelConnection, DefaultInput, DefaultOutput,
    DefaultSecretaryBuilder, DispatchStrategy, SecretaryCore
};
use tracing::info;

// 导入LLM集成模块
mod llm_integration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║           秘书Agent模式 - 基于LLM交互的工作循环演示          ║");
    info!("╚══════════════════════════════════════════════════════════════╝\n");

    // 创建LLM提供者
    let llm_provider = llm_integration::create_llm_provider();
    info!("✅ LLM提供者已创建: {}\n", llm_provider.name());

    // 创建执行Agent信息
    let mut frontend_agent = AgentInfo::new("frontend_agent", "前端开发Agent");
    frontend_agent.capabilities = vec![
        "frontend".to_string(),
        "ui_design".to_string(),
        "react".to_string(),
    ];
    frontend_agent.current_load = 20;
    frontend_agent.available = true;
    frontend_agent.performance_score = 0.85;

    let mut backend_agent = AgentInfo::new("backend_agent", "后端开发Agent");
    backend_agent.capabilities = vec![
        "backend".to_string(),
        "api_design".to_string(),
        "database".to_string(),
    ];
    backend_agent.current_load = 30;
    backend_agent.available = true;
    backend_agent.performance_score = 0.9;

    let mut test_agent = AgentInfo::new("test_agent", "测试Agent");
    test_agent.capabilities = vec!["testing".to_string(), "qa".to_string()];
    test_agent.current_load = 10;
    test_agent.available = true;
    test_agent.performance_score = 0.88;

    // 创建秘书Agent并注册执行Agent和LLM
    let secretary_behavior = DefaultSecretaryBuilder::new()
        .with_name("基于LLM的开发项目秘书")
        .with_dispatch_strategy(DispatchStrategy::CapabilityFirst)
        .with_auto_clarify(true)  // 自动澄清
        .with_auto_dispatch(true) // 自动分配
        .with_llm(llm_provider)
        // 注册前端开发Agent
        .with_executor(frontend_agent)
        // 注册后端开发Agent
        .with_executor(backend_agent)
        // 注册测试Agent
        .with_executor(test_agent)
        .build();

    info!("✅ 秘书Agent已创建，使用最新API\n");

    // 创建通道连接
    let (connection, input_tx, mut output_rx) = ChannelConnection::new_pair(32);

    // 创建并启动核心引擎
    let (handle, join_handle) = SecretaryCore::new(secretary_behavior)
        .start(connection)
        .await;

    info!("✅ 秘书Agent核心引擎已启动\n");

    // 发送测试想法
    info!("📥 发送想法给秘书Agent:");
    info!("   '开发一个包含注册、登录、权限管理功能的用户管理系统'\n");

    let idea = DefaultInput::Idea {
        content: "开发一个包含注册、登录、权限管理功能的用户管理系统".to_string(),
        priority: None,
        metadata: None,
    };

    input_tx.send(idea).await?;

    // 接收响应
    info!("📤 接收秘书Agent响应:");

    // 创建一个重复计时器
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

    // 等待最多10秒
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(10));
    tokio::pin!(timeout);

    let mut received_welcome = false;
    let mut received_response = false;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                info!("   ⏰ 超时未收到完整响应");
                break;
            },
            _ = interval.tick() => {
                // 检查是否有输出
                match output_rx.try_recv() {
                    Ok(result) => {
                        match result {
                            DefaultOutput::Message { content } => {
                                if content.contains("您好！我是") {
                                    info!("   💬 欢迎消息: {}", content);
                                    received_welcome = true;
                                } else {
                                    info!("   💬 消息: {}", content);
                                }
                            },
                            DefaultOutput::Acknowledgment { message } => {
                                info!("   ✅ 确认: {}", message);
                                received_response = true;
                            },
                            DefaultOutput::Error { message } => {
                                info!("   ❌ 错误: {}", message);
                                received_response = true;
                            },
                            DefaultOutput::DecisionRequired { decision } => {
                                info!("   ⏸️ 需要决策: {}", decision.description);
                                received_response = true;
                            },
                            DefaultOutput::Report { report } => {
                                info!("   📊 汇报: {}", report.content);
                                received_response = true;
                            },
                            _ => info!("   📦 其他响应: {:?}", result),
                        }
                    },
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        info!("   🔚 连接已关闭");
                        break;
                    },
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // 没有消息，继续检查
                    },
                }

                // 如果已经收到欢迎消息和响应，退出循环
                if received_welcome && received_response {
                    break;
                }
            }
        }
    }

    // 停止Agent
    handle.stop().await;
    join_handle.abort();

    info!("\n✅ 所有示例运行完成！");

    Ok(())
}
