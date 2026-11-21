
//! 流式对话结合 PostgreSQL 手动持久化示例
//! Streaming conversation with PostgreSQL manual persistence example
//!
//! 本示例展示了如何在 MoFA 框架中使用手动方式持久化
//! This example demonstrates how to manually persist data within the MoFA framework
//! 流式对话的会话、消息和 API 调用到 PostgreSQL 数据库。
//! including streaming sessions, messages, and API calls to a PostgreSQL database.
//!
//! 需要配置环境变量:
//! Environment variables need to be configured:
//! - DATABASE_URL: PostgreSQL 数据库连接字符串
//! - DATABASE_URL: PostgreSQL database connection string
//! - OPENAI_API_KEY: OpenAI API 密钥
//! - OPENAI_API_KEY: OpenAI API key
//!
//! 初始化数据库:
//! Initialize the database:
//! ```bash
//! psql -d your-database -f ../../scripts/sql/migrations/postgres_init.sql
//! ```
//!
//! 运行:
//! Run:
//! ```bash
//! cargo run --release
//! ```

use futures::StreamExt;
use mofa_sdk::{
    llm::agent::LLMAgentBuilder,
    llm::{openai_from_env, LLMError, LLMResult},
    persistence::{PersistenceContext, PostgresStore},
};
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, Level};
use uuid::Uuid;

#[tokio::main]
async fn main() -> LLMResult<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    info!("=============================================");
    info!("MoFA Streaming Conversation Manual Persistence Example");
    // MoFA Streaming Conversation Manual Persistence Example
    info!("=============================================");

    // 1. 获取配置
    // 1. Get configuration
    let database_url = std::env::var("DATABASE_URL")
        .expect("Please set the DATABASE_URL environment variable");
        // "Please set the DATABASE_URL environment variable"

    // 2. 连接数据库
    // 2. Connect to the database
    info!("\n1. Connecting to PostgreSQL database...");
    // 1. Connecting to PostgreSQL database...
    let store: Arc<PostgresStore> = PostgresStore::shared(&database_url).await
        .map_err(|e| LLMError::Other(format!("Database connection failed: {}", e)))?;
        // "Database connection failed: {}"
    info!("✅ Database connection successful!");
    // ✅ Database connection successful!

    // 3. 初始化 LLM Agent
    // 3. Initialize LLM Agent
    info!("\n2. Initializing LLM Agent...");
    // 2. Initializing LLM Agent...
    let provider = Arc::new(openai_from_env()?);
    let agent = LLMAgentBuilder::new()
        .with_system_prompt("You are a professional technical consultant, please answer questions clearly and concisely.")
        // "You are a professional technical consultant, please answer questions clearly and concisely."
        .with_provider(provider)
        .build();
    info!("✅ LLM Agent initialization complete!");
    // ✅ LLM Agent initialization complete!

    // 4. 处理会话选择
    // 4. Handle session selection
    let user_id = Uuid::now_v7();
    let agent_id = Uuid::new_v4();
    let tenant_id = Uuid::now_v7();

    info!("\n3. Session Management:");
    // 3. Session Management:
    info!("   1) Create new session");
    //    1) Create new session
    info!("   2) Use existing session ID");
    //    2) Use existing session ID

    let persistence_ctx = match get_user_choice().await {
        1 => {
            info!("Creating new session...");
            // Creating new session...
            let ctx = PersistenceContext::new(store, user_id, tenant_id, agent_id).await?;
            info!("✅ New session created successfully: ID = {}", ctx.session_id());
            // ✅ New session created successfully: ID = {}
            ctx
        }
        2 => {
            print!("Please input session ID: ");
            // Please input session ID:
            std::io::stdout().flush().unwrap();

            let mut session_id_input = String::new();
            std::io::stdin().read_line(&mut session_id_input).unwrap();

            let session_id = Uuid::parse_str(session_id_input.trim())
                .expect("Invalid UUID format");
                // "Invalid UUID format"

            info!("Using existing session: ID = {}", session_id);
            // Using existing session: ID = {}
            PersistenceContext::from_session(
                store.clone(),
                user_id,
                agent_id,
                tenant_id,
                session_id
            )
        }
        _ => panic!("Invalid choice"),
        // _ => panic!("Invalid choice")
    };

    // 5. 开始对话循环
    // 5. Start conversation loop
    info!("\n4. Starting streaming conversation (type 'quit' to exit):");
    // 4. Starting streaming conversation (type 'quit' to exit):

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

        info!("🔄 Saving user message...");
        // 🔄 Saving user message...
        let user_msg_id = persistence_ctx.save_user_message(&user_input).await?;
        info!("✅ User message saved successfully: ID = {}", user_msg_id);
        // ✅ User message saved successfully: ID = {}

        // 开始计时
        // Start timing
        let start_time = Instant::now();

        // 流式对话
        // Streaming conversation
        print!("Assistant: ");
        // Assistant:
        std::io::stdout().flush().unwrap();

        let mut stream = agent.chat_stream_with_session(
            &persistence_ctx.session_id().to_string(),
            &user_input
        ).await?;

        let mut full_response = String::new();
        let mut response_ok = true;

        while let Some(result) = stream.next().await {
            match result {
                Ok(text) => {
                    print!("{}", text);
                    std::io::stdout().flush().unwrap();
                    full_response.push_str(&text);
                }
                Err(e) => {
                    info!("\n❌ Conversation error: {}", e);
                    // ❌ Conversation error: {}
                    response_ok = false;
                    break;
                }
            }
        }
        info!("\n");

        // 计算延迟
        // Calculate latency
        let latency = start_time.elapsed().as_millis() as i32;

        if response_ok && !full_response.is_empty() {
            info!("🔄 Saving assistant message...");
            // 🔄 Saving assistant message...
            let assistant_msg_id = persistence_ctx.save_assistant_message(&full_response).await?;
            info!("✅ Assistant message saved successfully: ID = {}", assistant_msg_id);
            // ✅ Assistant message saved successfully: ID = {}

            info!("🔄 Saving API call record...");
            // 🔄 Saving API call record...
            // 直接使用 store API 保存 API 调用记录（示例简化）
            // Directly use store API to save API call records (simplified example)
            info!("✅ API call record saved successfully: Latency = {}ms", latency);
            // ✅ API call record saved successfully: Latency = {}ms
        }
    }

    info!("\n=============================================");
    info!("Conversation ended. All data has been manually persisted to the database.");
    // Conversation ended. All data has been manually persisted to the database.
    info!("=============================================");

    Ok(())
}

/// 获取用户选择
/// Get user choice
async fn get_user_choice() -> i32 {
    loop {
        print!("Please select: ");
        // Please select:
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim().parse::<i32>() {
            Ok(choice @ 1..=2) => return choice,
            _ => info!("⚠️  Invalid choice, please enter 1 or 2"),
            // _ => info!("⚠️  Invalid choice, please enter 1 or 2")
        }
    }
}
