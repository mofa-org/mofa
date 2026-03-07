//! 从数据库加载 Agent 配置并流式对话示例
//! Example of loading Agent configuration from database and streaming conversation
//!
//! 本示例展示了如何从 PostgreSQL 数据库加载 Agent 配置，
//! This example shows how to load Agent configuration from a PostgreSQL database,
//! 并使用流式对话功能与用户交互。
//! and interact with users using the streaming conversation feature.
//!
//! 需要配置环境变量:
//! Environment variables need to be configured:
//! - DATABASE_URL: PostgreSQL 数据库连接字符串，例如 "postgres://postgres:password@localhost:5432/mofa"
//! - DATABASE_URL: PostgreSQL connection string, e.g., "postgres://postgres:password@localhost:5432/mofa"
//! - AGENT_CODE: 要加载的 Agent 代码（在数据库 entity_agent 表中的 agent_code 字段）
//! - AGENT_CODE: Agent code to load (agent_code field in the entity_agent table)
//! - USER_ID: 用户 ID（用于持久化消息）
//! - USER_ID: User ID (used for message persistence)
//! - TENANT_ID: 租户 ID（可选，默认为 00000000-0000-0000-0000-000000000000）
//! - TENANT_ID: Tenant ID (optional, defaults to 00000000-0000-0000-0000-000000000000)
//! - OPENAI_API_KEY: OpenAI API 密钥（用于 LLM 访问，如果数据库中 provider 未配置）
//! - OPENAI_API_KEY: OpenAI API key (for LLM access if provider is not configured in DB)
//!
//! 首先初始化数据库（包含 Agent 和 Provider 表）:
//! First, initialize the database (including Agent and Provider tables):
//! ```bash
//! psql -d your-database -f ../../scripts/sql/migrations/postgres_init.sql
//! ```
//!
//! 然后插入测试数据（可选）:
//! Then insert test data (optional):
//! ```sql
//! -- 插入 Provider
//! -- Insert Provider
//! INSERT INTO entity_provider (id, tenant_id, provider_name, provider_type, api_base, api_key, enabled, create_time, update_time)
//! VALUES (
//!     '550e8400-e29b-41d4-a716-446655440001',
//!     '00000000-0000-0000-0000-000000000000',
//!     'openai-provider',
//!     'openai',
//!     'https://api.openai.com/v1',
//!     'your-api-key-here',
//!     true,
//!     NOW(),
//!     NOW()
//! );
//!
//! -- 插入 Agent
//! -- Insert Agent
//! INSERT INTO entity_agent (
//!     id, tenant_id, agent_code, agent_name, agent_order, agent_status,
//!     model_name, provider_id, system_prompt, temperature, stream,
//!     context_limit, create_time, update_time
//! ) VALUES (
//!     '550e8400-e29b-41d4-a716-446655440002',
//!     '00000000-0000-0000-0000-000000000000',
//!     'chat-assistant',
//!     '聊天助手',
//!     1,
//!     true,
//!     'gpt-4o-mini',
//!     '550e8400-e29b-41d4-a716-446655440001',
//!     '你是一个友好且专业的 AI 助手，能够帮助用户解答问题和完成任务。',
//!     0.7,
//!     true,
//!     10,
//!     NOW(),
//!     NOW()
//! );
//! ```
//!
//! 运行示例:
//! Run example:
//! ```bash
//! export DATABASE_URL="postgres://postgres:password@localhost:5432/mofa"
//! export AGENT_CODE="chat-assistant"
//! export USER_ID="550e8400-e29b-41d4-a716-446655440003"
//! export OPENAI_API_KEY="sk-xxx"
//!
//! cargo run --release
//! ```

use mofa_sdk::persistence::ChatSession;
use std::io::Write;
use std::sync::Arc;
use futures::StreamExt;
use mofa_sdk::{llm::{LLMAgentBuilder, Role}, persistence::{PostgresStore, PersistencePlugin}};
use tracing::{info, Level};
use uuid::Uuid;
use mofa_sdk::llm::LLMError;
use mofa_sdk::persistence::{AgentStore, SessionStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    info!("=============================================");
    info!("从数据库加载 Agent 配置 - 流式对话示例");
    // Loading Agent config from database - Streaming dialogue example
    info!("=============================================");

    // 从环境变量读取配置
    // Read configuration from environment variables
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL 环境变量未设置");
        // DATABASE_URL environment variable is not set
    let agent_code = std::env::var("AGENT_CODE")
        .unwrap_or_else(|_| "chat-assistant".to_string());
    let user_id_str = std::env::var("USER_ID")
        .expect("USER_ID 环境变量未设置");
        // USER_ID environment variable is not set
    let tenant_id_str = std::env::var("TENANT_ID")
        .unwrap_or_else(|_| "00000000-0000-0000-0000-000000000000".to_string());

    let user_id = Uuid::parse_str(&user_id_str)
        .expect("USER_ID 格式无效，需要 UUID 格式");
        // Invalid USER_ID format, UUID format is required
    let tenant_id = Uuid::parse_str(&tenant_id_str)
        .expect("TENANT_ID 格式无效，需要 UUID 格式");
        // Invalid TENANT_ID format, UUID format is required

    info!("数据库地址: {}", database_url);
    // Database URL: {}
    info!("Agent 代码: {}", agent_code);
    // Agent Code: {}
    info!("用户 ID: {}", user_id);
    // User ID: {}
    info!("租户 ID: {}", tenant_id);
    // Tenant ID: {}

    // 1. 连接到数据库
    // 1. Connect to the database
    info!("正在连接数据库...");
    // Connecting to database...
    let store = PostgresStore::connect(&database_url).await
        .map_err(|e| -> Box<dyn std::error::Error> { format!("数据库连接失败: {}", e).into() })?;
        // Database connection failed: {}
    info!("数据库连接成功");
    // Database connection successful

    // 2. 从数据库加载 Agent 配置
    // 2. Load Agent configuration from database
    info!("正在从数据库加载 Agent 配置: {}...", agent_code);
    // Loading Agent configuration from database: {}...
    let config = store
        .get_agent_by_code_and_tenant_with_provider(tenant_id, &agent_code)
        .await
        .map_err(|e| LLMError::Other(format!("Failed to load agent from database: {}", e)))?
        .ok_or_else(|| {
            LLMError::Other(format!(
                "Agent with code '{}' not found for tenant {}",
                agent_code, tenant_id
            ))
        })?;

    info!("Agent 配置加载成功:");
    // Agent configuration loaded successfully:
    info!("  - Agent 代码: {}", agent_code);
    //   - Agent Code: {}
    info!("  - 租户 ID: {}", tenant_id);
    //   - Tenant ID: {}

    // 3.5 创建会话记录到数据库
    // 3.5 Create session record in the database
    let session_id = Uuid::now_v7();

    let chat_session = ChatSession::new(user_id, config.agent.id)
        .with_id(session_id)
        .with_tenant_id(tenant_id)
        .with_title("新对话");
        // .with_title("New Conversation")

    store.create_session(&chat_session).await
        .map_err(|e| -> Box<dyn std::error::Error> { format!("创建会话失败: {}", e).into() })?;
        // Failed to create session: {}
    info!("会话创建成功: {}", session_id);
    // Session created successfully: {}

    let persistence_plugin = PersistencePlugin::from_store(
        "persistence-plugin",
        store,
        user_id,
        tenant_id,
        config.agent.id,
        session_id,
    );

    let agent_builder = LLMAgentBuilder::from_agent_config(&config)?
        .with_persistence_plugin(persistence_plugin);

    // 4. 构建 Agent（异步，支持从数据库加载会话历史）
    // 4. Build Agent (Async, supports loading session history from DB)
    info!("正在构建 Agent...");
    // Building Agent...
    let agent = agent_builder.build_async().await;
    info!("Agent 构建完成");
    // Agent construction complete

    // 5. 打印 Agent 配置信息
    // 5. Print Agent configuration information
    print_agent_info(&agent).await;

    // 6. 开始流式对话
    // 6. Start streaming conversation
    info!("\n=============================================");
    info!("开始流式对话 (输入 'quit' 或 'exit' 退出):");
    // Start streaming dialogue (type 'quit' or 'exit' to exit):
    info!("=============================================");

    let mut round = 0;

    loop {
        // 获取用户输入
        // Get user input
        print!("\n用户: ");
        // User: 
        std::io::stdout().flush().unwrap();

        let mut user_input = String::new();
        std::io::stdin().read_line(&mut user_input).unwrap();
        let user_input = user_input.trim().to_string();

        if user_input.is_empty() {
            continue;
        }

        if user_input.to_lowercase() == "quit" || user_input.to_lowercase() == "exit" {
            info!("退出程序...");
            // Exiting program...
            break;
        }

        round += 1;

        // 使用当前 Agent 进行流式对话
        // Use current Agent for streaming conversation
        print!("助手: ");
        // Assistant: 
        std::io::stdout().flush().unwrap();

        // 开始流式对话
        // Begin streaming dialogue
        let mut stream = agent.chat_stream(&user_input).await?;
        let mut full_response = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(text) => {
                    print!("{}", text);
                    std::io::stdout().flush().unwrap();
                    full_response.push_str(&text);
                }
                Err(e) => {
                    info!("\n错误: {}", e);
                    // Error: {}
                    break;
                }
            }
        }

        println!();

        // 打印上下文信息
        // Print context information
        print_context(&agent, round);
    }

    info!("=============================================");
    info!("对话结束。所有会话和消息已持久化到数据库。");
    // Conversation ended. All sessions and messages persisted to database.
    info!("=============================================");

    Ok(())
}

/// 打印 Agent 配置信息
/// Print Agent configuration information
async fn print_agent_info(agent: &mofa_sdk::llm::LLMAgent) {
    info!("\n=============================================");
    info!("Agent 配置信息:");
    // Agent configuration info:
    info!("=============================================");

    // 获取系统提示词
    // Get system prompt
    let history = agent.history().await;
    if let Some(system_msg) = history.first() {
        if matches!(system_msg.role, Role::System) {
            let prompt = system_msg.content.as_ref()
                .and_then(|c| {
                    if let mofa_sdk::llm::MessageContent::Text(text) = c {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or("");
            info!("  系统提示词: {}", prompt);
            //   System Prompt: {}
        }
    }

    info!("  当前上下文消息数: {} 条", history.len());
    //   Current context message count: {}
    info!("=============================================");
}

/// 打印当前上下文信息
/// Print current context information
fn print_context(agent: &mofa_sdk::llm::LLMAgent, round: usize) {
    use mofa_sdk::llm::Role;

    info!("\n------------ 第 {} 轮对话后上下文状态 ------------", round);
    // ------------ Context status after round {} ------------

    let history = futures::executor::block_on(agent.history());

    // 统计消息数量
    // Count number of messages
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
    // Current context total messages: {}
    info!("  - 系统消息: {} 条 (始终保留)", system_count);
    //   - System messages: {} (always retained)
    info!("  - 用户消息: {} 条", user_count);
    //   - User messages: {}
    info!("  - 助手消息: {} 条", assistant_count);
    //   - Assistant messages: {}
    info!("  - 对话轮数: {} 轮", user_count);
    //   - Dialogue rounds: {}

    info!("---------------------------------------------------");
}
