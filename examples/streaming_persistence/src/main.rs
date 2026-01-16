
//! 流式对话结合 PostgreSQL 持久化示例
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

use futures::StreamExt;
use mofa_sdk::{
    llm::agent::LLMAgentBuilder,
    llm::{openai_from_env, LLMError, LLMResult},
    persistence::{AgentPersistenceHandler, PersistenceHandler, PostgresStore},
};
use tracing::{info, Level};
use uuid::Uuid;

#[tokio::main]
async fn main() -> LLMResult<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    info!("=============================================");
    info!("MoFA 流式对话 PostgreSQL 持久化示例");
    info!("=============================================");

    // 1. 配置参数
    let database_url = std::env::var("DATABASE_URL")
        .expect("请设置 DATABASE_URL 环境变量");

    // 2. 初始化数据库连接
    info!("\n1. 连接 PostgreSQL 数据库...");
    let store: Arc<PostgresStore> = PostgresStore::shared(&database_url).await
        .map_err(|e| LLMError::Other(format!("数据库连接失败: {}", e)))?;
    info!("✅ 数据库连接成功!");

    // 3. 初始化持久化处理器
    info!("\n2. 初始化持久化系统...");
    let user_id = Uuid::now_v7();  // 替换为实际业务中的用户 ID
    let agent_id = Uuid::parse_str("9c1377d7-4c7f-49cf-b72f-66b24916a404").unwrap();  // Agent 固定 ID

    let persistence = Arc::new(PersistenceHandler::new(
        store.clone(),
        user_id,
        agent_id
    ));
    info!("✅ 持久化系统初始化完成!");
    info!("   - 用户 ID: {}", user_id);
    info!("   - Agent ID: {}", agent_id);

    // 4. 创建 LLM Agent
    info!("\n3. 创建 LLM Agent...");
    let provider = Arc::new(openai_from_env()?);
    // 设置事件处理器
    let event_handler = Box::new(AgentPersistenceHandler::new(persistence.clone()));
    // 配置流式 Agent
    let agent = LLMAgentBuilder::new()
        .with_id("streaming-persistence-agent")
        .with_name("流式持久化 Agent")
        .with_provider(provider)
        .with_system_prompt("你是一个专业的 AI 助手，回答问题要清晰、准确、有帮助。")
        .with_event_handler(event_handler)
        .build();

    info!("✅ LLM Agent 创建完成!");

    // 5. 开始交互
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

        // 使用当前活动会话进行流式对话
        print!("助手: ");
        std::io::stdout().flush().unwrap();

        // 开始流式对话
        let mut stream = agent.chat_stream(&user_input).await?;
        let mut _full_response = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(text) => {
                    print!("{}", text);
                    std::io::stdout().flush().unwrap();
                    _full_response.push_str(&text);
                }
                Err(e) => {
                    info!("\n❌ 对话错误: {}", e);
                    break;
                }
            }
        }

        info!("\n");
    }

    info!("\n=============================================");
    info!("对话结束。所有会话和消息已持久化到数据库。");
    info!("=============================================");

    Ok(())
}
