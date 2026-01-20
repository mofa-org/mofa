//! 流式对话结合 PostgreSQL 持久化示例（简化版）
//!
//! 本示例展示了如何在 MoFA 框架中使用流式对话功能，
//! 并将会话、消息和 API 调用持久化到 PostgreSQL 数据库。
//!
//! 需要配置环境变量:
//! - DATABASE_URL: PostgreSQL 数据库连接字符串，例如 "postgres://postgres:password@localhost:5432/mofa"
//! - OPENAI_API_KEY: OpenAI API 密钥 (用于 LLM 访问)
//!
//! 首先初始化数据库:
//! ```bash
//! psql -d your-database -f ../../scripts/sql/migrations/postgres_init.sql
//! ```
//!
//! 运行示例:
//! ```bash
//! cargo run --release
//! ```

use std::io::Write;
use futures::StreamExt;
use mofa_sdk::{
    llm::{LLMResult, Role},
    persistence::quick_agent_with_postgres,
};
use tracing::{info, Level};

#[tokio::main]
async fn main() -> LLMResult<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    info!("=============================================");
    info!("MoFA 流式对话 PostgreSQL 持久化示例（简化版）");
    info!("=============================================");

    let agent = quick_agent_with_postgres(
        "你是一个专业的 AI 助手，回答问题要清晰、准确、有帮助。"
    ).await?
    .with_session_id("019bda9f-9ffd-7a80-a9e5-88b05e81a7d4")
    .with_name("流式持久化 Agent")
    .with_sliding_window(2)
    .build_async()
    .await;

    info!("Agent 已创建，开始流式对话 (输入 'quit' 退出):");
    info!("滑动窗口大小: 2 轮（每轮 = 1个用户消息 + 1个助手响应）");

    let mut round = 0;

    loop {
        // 获取用户输入
        print!("\n用户: ");
        std::io::stdout().flush().unwrap();

        let mut user_input = String::new();
        std::io::stdin().read_line(&mut user_input).unwrap();
        let user_input = user_input.trim().to_string();

        if user_input.to_lowercase() == "quit" {
            break;
        }

        round += 1;

        // 使用当前活动会话进行流式对话
        print!("助手: ");
        std::io::stdout().flush().unwrap();

        // 开始流式对话
        let mut stream = agent.chat_stream(&user_input).await?;
        while let Some(result) = stream.next().await {
            match result {
                Ok(text) => {
                    print!("{}", text);
                    std::io::stdout().flush().unwrap();
                }
                Err(e) => {
                    info!("\n错误: {}", e);
                    break;
                }
            }
        }

        println!();

        // 打印上下文信息
        print_context(&agent, round).await;
    }

    info!("=============================================");
    info!("对话结束。所有会话和消息已持久化到数据库。");
    info!("=============================================");

    Ok(())
}

/// 打印当前上下文信息
async fn print_context(agent: &mofa_sdk::llm::LLMAgent, round: usize) {
    use mofa_sdk::llm::Role;

    info!("");
    info!("------------ 第 {} 轮对话后上下文状态 ------------", round);

    let history = agent.history().await;

    // 统计消息数量
    let user_count = history.iter()
        .filter(|m| matches!(m.role, Role::User))
        .count();
    let assistant_count = history.iter()
        .filter(|m| matches!(m.role, Role::Assistant))
        .count();
    let system_count = history.iter()
        .filter(|m| matches!(m.role, Role::System))
        .count();

    info!("当前上下文消息总数: {} 条", history.len());
    info!("  - 系统消息: {} 条 (始终保留)", system_count);
    info!("  - 用户消息: {} 条", user_count);
    info!("  - 助手消息: {} 条", assistant_count);
    info!("  - 对话轮数: {} 轮", user_count);

    // 打印详细消息列表
    info!("当前上下文消息列表:");
    for (i, msg) in history.iter().enumerate() {
        let content = msg.content.as_ref()
            .and_then(|c| {
                if let mofa_sdk::llm::MessageContent::Text(text) = c {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .unwrap_or("");

        match msg.role {
            Role::System => {
                info!("  [{}] System: {:.50}...", i, content);
            }
            Role::User => {
                info!("  [{}] User: {:.50}...", i, content);
            }
            Role::Assistant => {
                info!("  [{}] Assistant: {:.50}...", i, content);
            }
            _ => {}
        }
    }

    info!("---------------------------------------------------");
    info!("");
}
