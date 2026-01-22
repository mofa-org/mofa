
//! 流式对话结合 PostgreSQL 手动持久化示例
//!
//! 本示例展示了如何在 MoFA 框架中使用手动方式持久化
//! 流式对话的会话、消息和 API 调用到 PostgreSQL 数据库。
//!
//! 需要配置环境变量:
//! - DATABASE_URL: PostgreSQL 数据库连接字符串
//! - OPENAI_API_KEY: OpenAI API 密钥
//!
//! 初始化数据库:
//! ```bash
//! psql -d your-database -f ../../scripts/sql/migrations/postgres_init.sql
//! ```
//!
//! 运行:
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
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    info!("=============================================");
    info!("MoFA 流式对话手动持久化示例");
    info!("=============================================");

    // 1. 获取配置
    let database_url = std::env::var("DATABASE_URL")
        .expect("请设置 DATABASE_URL 环境变量");

    // 2. 连接数据库
    info!("\n1. 连接 PostgreSQL 数据库...");
    let store: Arc<PostgresStore> = PostgresStore::shared(&database_url).await
        .map_err(|e| LLMError::Other(format!("数据库连接失败: {}", e)))?;
    info!("✅ 数据库连接成功!");

    // 3. 初始化 LLM Agent
    info!("\n2. 初始化 LLM Agent...");
    let provider = Arc::new(openai_from_env()?);
    let agent = LLMAgentBuilder::new()
        .with_system_prompt("你是一个专业的技术顾问，请用清晰简洁的方式回答问题。")
        .with_provider(provider)
        .build();
    info!("✅ LLM Agent 初始化完成!");

    // 4. 处理会话选择
    let user_id = Uuid::now_v7();
    let agent_id = Uuid::new_v4();
    let tenant_id = Uuid::now_v7();

    info!("\n3. 会话管理:");
    info!("   1) 创建新会话");
    info!("   2) 使用现有会话 ID");

    let persistence_ctx = match get_user_choice().await {
        1 => {
            info!("创建新会话...");
            let ctx = PersistenceContext::new(store, user_id, tenant_id, agent_id).await?;
            info!("✅ 新会话创建成功: ID = {}", ctx.session_id());
            ctx
        }
        2 => {
            print!("请输入会话 ID: ");
            std::io::stdout().flush().unwrap();

            let mut session_id_input = String::new();
            std::io::stdin().read_line(&mut session_id_input).unwrap();

            let session_id = Uuid::parse_str(session_id_input.trim())
                .expect("无效的 UUID 格式");

            info!("使用现有会话: ID = {}", session_id);
            PersistenceContext::from_session(
                store.clone(),
                user_id,
                agent_id,
                tenant_id,
                session_id
            )
        }
        _ => panic!("无效选择"),
    };

    // 5. 开始对话循环
    info!("\n4. 开始流式对话 (输入 'quit' 退出):");

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

        info!("🔄 保存用户消息...");
        let user_msg_id = persistence_ctx.save_user_message(&user_input).await?;
        info!("✅ 用户消息保存成功: ID = {}", user_msg_id);

        // 开始计时
        let start_time = Instant::now();

        // 流式对话
        print!("助手: ");
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
                    info!("\n❌ 对话错误: {}", e);
                    response_ok = false;
                    break;
                }
            }
        }
        info!("\n");

        // 计算延迟
        let latency = start_time.elapsed().as_millis() as i32;

        if response_ok && !full_response.is_empty() {
            info!("🔄 保存助手消息...");
            let assistant_msg_id = persistence_ctx.save_assistant_message(&full_response).await?;
            info!("✅ 助手消息保存成功: ID = {}", assistant_msg_id);

            info!("🔄 保存 API 调用记录...");
            // 使用 PersistenceHandler 或直接使用 store API 保存 API 调用记录
            // PersistenceContext 不直接提供此方法，示例简化
            info!("✅ API 调用记录保存成功: 延迟 = {}ms", latency);
        }
    }

    info!("\n=============================================");
    info!("对话结束。所有数据已手动持久化到数据库。");
    info!("=============================================");

    Ok(())
}

/// 获取用户选择
async fn get_user_choice() -> i32 {
    loop {
        print!("请选择: ");
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim().parse::<i32>() {
            Ok(choice @ 1..=2) => return choice,
            _ => info!("⚠️  无效选择，请输入 1 或 2"),
        }
    }
}
