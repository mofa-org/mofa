//!/usr/bin/env cargo run --release
//! 秘书Agent模式完整用例示例 - 基于LLM交互的版本
//! Secretary Agent Mode Complete Use Case Example - LLM-Interaction Based Version
//!
//! 本示例展示秘书Agent的完整工作循环，并结合LLM进行智能交互：
//! This example demonstrates the complete work cycle of a Secretary Agent, integrated with LLM for smart interaction:
//! 1. 接收想法 → 使用LLM记录Todo
//! 1. Receive Ideas -> Use LLM to record Todo
//! 2. 澄清需求 → 使用LLM转换为项目文档
//! 2. Clarify Requirements -> Use LLM to convert into project documentation
//! 3. 调度分配 → 调用执行Agent
//! 3. Dispatch and Allocation -> Call Execution Agents
//!
//! 该示例使用了最新的API版本。
//! This example uses the latest API version.

use mofa_sdk::secretary::{
    AgentInfo, ChannelConnection, DefaultInput, DefaultOutput,
    DefaultSecretaryBuilder, DispatchStrategy, SecretaryCore
};
use tracing::info;

// 导入LLM集成模块
// Import LLM integration module
mod llm_integration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║   Secretary Agent Mode - LLM-based Work Cycle Demonstration  ║");
    info!("╚══════════════════════════════════════════════════════════════╝\n");

    // 创建LLM提供者
    // Create LLM provider
    let llm_provider = llm_integration::create_llm_provider();
    info!("✅ LLM provider created: {}\n", llm_provider.name());

    // 创建执行Agent信息
    // Create Execution Agent information
    let mut frontend_agent = AgentInfo::new("frontend_agent", "Frontend Development Agent");
    // Frontend Development Agent
    frontend_agent.capabilities = vec![
        "frontend".to_string(),
        "ui_design".to_string(),
        "react".to_string(),
    ];
    frontend_agent.current_load = 20;
    frontend_agent.available = true;
    frontend_agent.performance_score = 0.85;

    let mut backend_agent = AgentInfo::new("backend_agent", "Backend Development Agent");
    // Backend Development Agent
    backend_agent.capabilities = vec![
        "backend".to_string(),
        "api_design".to_string(),
        "database".to_string(),
    ];
    backend_agent.current_load = 30;
    backend_agent.available = true;
    backend_agent.performance_score = 0.9;

    let mut test_agent = AgentInfo::new("test_agent", "Testing Agent");
    // Testing Agent
    test_agent.capabilities = vec!["testing".to_string(), "qa".to_string()];
    test_agent.current_load = 10;
    test_agent.available = true;
    test_agent.performance_score = 0.88;

    // 创建秘书Agent并注册执行Agent和LLM
    // Create Secretary Agent and register Execution Agents and LLM
    let secretary_behavior = DefaultSecretaryBuilder::new()
        .with_name("LLM-based Development Project Secretary")
        // LLM-based development project secretary
        .with_dispatch_strategy(DispatchStrategy::CapabilityFirst)
        .with_auto_clarify(true)  // 自动澄清
        // Auto-clarification
        .with_auto_dispatch(true) // 自动分配
        // Auto-dispatch
        .with_llm(llm_provider)
        // 注册前端开发Agent
        // Register Frontend Development Agent
        .with_executor(frontend_agent)
        // 注册后端开发Agent
        // Register Backend Development Agent
        .with_executor(backend_agent)
        // 注册测试Agent
        // Register Testing Agent
        .with_executor(test_agent)
        .build();

    info!("✅ Secretary Agent created using the latest API\n");

    // 创建通道连接
    // Create channel connection
    let (connection, input_tx, mut output_rx) = ChannelConnection::new_pair(32);

    // 创建并启动核心引擎
    // Create and start core engine
    let (handle, join_handle) = SecretaryCore::new(secretary_behavior)
        .start(connection)
        .await;

    info!("✅ Secretary Agent core engine started\n");

    // 发送测试想法
    // Send test idea
    info!("📥 Sending idea to Secretary Agent:");
    info!("   'Develop a user management system with registration, login, and role-based access control'\n");

    let idea = DefaultInput::Idea {
        content: "Develop a user management system with registration, login, and role-based access control".to_string(),
        priority: None,
        metadata: None,
    };

    input_tx.send(idea).await?;

    // 接收响应
    // Receive response
    info!("📤 Receiving Secretary Agent response:");

    // 创建一个重复计时器
    // Create a repeating interval timer
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

    // 等待最多10秒
    // Wait for a maximum of 10 seconds
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(10));
    tokio::pin!(timeout);

    let mut received_welcome = false;
    let mut received_response = false;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                info!("   ⏰ Timeout reached without receiving complete response");
                // Timeout reached without receiving complete response
                break;
            },
            _ = interval.tick() => {
                // 检查是否有输出
                // Check if there is output
                match output_rx.try_recv() {
                    Ok(result) => {
                        match result {
                            DefaultOutput::Message { content } => {
                                if content.contains("Welcome") || content.contains("您好！我是") {
                                    info!("   💬 Welcome message: {}", content);
                                    // Welcome message
                                    received_welcome = true;
                                } else {
                                    info!("   💬 Message: {}", content);
                                    // Message
                                }
                            },
                            DefaultOutput::Acknowledgment { message } => {
                                info!("   ✅ Acknowledgment: {}", message);
                                // Acknowledgment
                                received_response = true;
                            },
                            DefaultOutput::Error { message } => {
                                info!("   ❌ Error: {}", message);
                                // Error
                                received_response = true;
                            },
                            DefaultOutput::DecisionRequired { decision } => {
                                info!("   ⏸️ Decision required: {}", decision.description);
                                // Decision Required
                                received_response = true;
                            },
                            DefaultOutput::Report { report } => {
                                info!("   📊 Report: {}", report.content);
                                // Report
                                received_response = true;
                            },
                            _ => info!("   📦 其他响应: {:?}", result),
                            // Other response
                        }
                    },
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        info!("   🔚 Connection closed");
                        // Connection closed
                        break;
                    },
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // 没有消息，继续检查
                        // No message, continue checking
                    },
                }

                // 如果已经收到欢迎消息和响应，退出循环
                // If welcome message and response are received, exit loop
                if received_welcome && received_response {
                    break;
                }
            }
        }
    }

    // 停止Agent
    // Stop the Agent
    handle.stop().await;
    join_handle.abort();

    info!("\n✅ All examples finished running!");
    // All examples finished running!

    Ok(())
}
