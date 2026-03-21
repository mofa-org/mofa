//! 流式对话结合 PostgreSQL 持久化示例（简化版）
//! Streaming dialogue with PostgreSQL persistence example (simplified version)
//!
//! 本示例展示了如何在 MoFA 框架中使用流式对话功能，
//! This example demonstrates how to use streaming dialogue features in the MoFA framework,
//! 并将会话、消息和 API 调用持久化到 PostgreSQL 数据库。
//! and persist sessions, messages, and API calls to a PostgreSQL database.
//!
//! 需要配置环境变量:
//! Environment variables need to be configured:
//! - DATABASE_URL: PostgreSQL 数据库连接字符串，例如 "postgres://postgres:password@localhost:5432/mofa"
//! - DATABASE_URL: PostgreSQL connection string, e.g., "postgres://postgres:password@localhost:5432/mofa"
//! - OPENAI_API_KEY: OpenAI API 密钥 (用于 LLM 访问)
//! - OPENAI_API_KEY: OpenAI API key (for LLM access)
//!
//! 首先初始化数据库:
//! First, initialize the database:
//! ```bash
//! psql -d your-database -f ../../scripts/sql/migrations/postgres_init.sql
//! ```
//!
//! 运行示例:
//! Run the example:
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
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    info!("=============================================");
    info!("MoFA Streaming Dialogue PostgreSQL Persistence Example (Simplified)");
    info!("=============================================");

    let agent = quick_agent_with_postgres(
        "You are a professional AI assistant; provide clear, accurate, and helpful answers."
        // "You are a professional AI assistant; provide clear, accurate, and helpful answers."
    ).await?
    .with_session_id("019bda9f-9ffd-7a80-a9e5-88b05e81a7d4")
    .with_name("Streaming Persistence Agent")
    // .with_name("Streaming Persistence Agent")
    .with_sliding_window(2)
    .build_async()
    .await;

    info!("Agent created, starting streaming dialogue (type 'quit' to exit):");
    // Agent created, starting streaming dialogue (type 'quit' to exit):
    info!("Sliding window size: 2 rounds (each round = 1 user message + 1 assistant response)");
    // Sliding window size: 2 rounds (each round = 1 user message + 1 assistant response)

    let mut round = 0;

    loop {
        // 获取用户输入
        // Get user input
        print!("\nUser: ");
        // User:
        std::io::stdout().flush().unwrap();

        let mut user_input = String::new();
        std::io::stdin().read_line(&mut user_input).unwrap();
        let user_input = user_input.trim().to_string();

        if user_input.to_lowercase() == "quit" {
            break;
        }

        round += 1;

        // 使用当前活动会话进行流式对话
        // Use the current active session for streaming dialogue
        print!("Assistant: ");
        // Assistant:
        std::io::stdout().flush().unwrap();

        // 开始流式对话
        // Start streaming dialogue
        let mut stream = agent.chat_stream(&user_input).await?;
        while let Some(result) = stream.next().await {
            match result {
                Ok(text) => {
                    print!("{}", text);
                    std::io::stdout().flush().unwrap();
                }
                Err(e) => {
                    info!("\nError: {}", e);
                    // Error: {}
                    break;
                }
            }
        }

        println!();

        // 打印上下文信息
        // Print context information
        print_context(&agent, round).await;
    }

    info!("=============================================");
    info!("Dialogue ended. All sessions and messages persisted to the database.");
    // Dialogue ended. All sessions and messages persisted to the database.
    info!("=============================================");

    Ok(())
}

/// 打印当前上下文信息
/// Print current context information
async fn print_context(agent: &mofa_sdk::llm::LLMAgent, round: usize) {
    use mofa_sdk::llm::Role;

    info!("");
    info!("------------ Context status after round {} ------------", round);
    // ------------ Context status after round {} ------------

    let history = agent.history().await;

    // 统计消息数量
    // Count the number of messages
    let user_count = history.iter()
        .filter(|m| matches!(m.role, Role::User))
        .count();
    let assistant_count = history.iter()
        .filter(|m| matches!(m.role, Role::Assistant))
        .count();
    let system_count = history.iter()
        .filter(|m| matches!(m.role, Role::System))
        .count();

    info!("Total messages in current context: {}", history.len());
    // Total messages in current context: {}
    info!("  - System messages: {} (always retained)", system_count);
    //   - System messages: {} (always retained)
    info!("  - User messages: {}", user_count);
    //   - User messages: {}
    info!("  - Assistant messages: {}", assistant_count);
    //   - Assistant messages: {}
    info!("  - Dialogue rounds: {}", user_count);
    //   - Dialogue rounds: {}

    // 打印详细消息列表
    // Print detailed message list
    info!("Current context message list:");
    // Current context message list:
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
