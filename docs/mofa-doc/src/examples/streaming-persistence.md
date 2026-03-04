# Streaming & Persistence

Examples demonstrating streaming LLM conversations with database persistence.

## Streaming Persistence (Automatic)

Automatic persistence with PostgreSQL for streaming conversations.

**Location:** `examples/streaming_persistence/`

```rust
use mofa_sdk::persistence::quick_agent_with_postgres;

#[tokio::main]
async fn main() -> LLMResult<()> {
    // Create agent with automatic PostgreSQL persistence
    let agent = quick_agent_with_postgres(
        "You are a professional AI assistant."
    ).await?
    .with_session_id("019bda9f-9ffd-7a80-a9e5-88b05e81a7d4")
    .with_name("Streaming Persistence Agent")
    .with_sliding_window(2)  // Keep last 2 rounds
    .build_async()
    .await;

    // Stream chat with automatic persistence
    let mut stream = agent.chat_stream(&user_input).await?;
    while let Some(result) = stream.next().await {
        match result {
            Ok(text) => print!("{}", text),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}
```

### Features

- **Automatic persistence**: Messages saved to database automatically
- **Sliding window**: Configurable context window size
- **Session management**: Resume conversations across restarts

## Manual Persistence

Full control over what and when to persist.

**Location:** `examples/streaming_manual_persistence/`

```rust
use mofa_sdk::persistence::{PersistenceContext, PostgresStore};

#[tokio::main]
async fn main() -> LLMResult<()> {
    // Connect to database
    let store = PostgresStore::shared(&database_url).await?;

    // Create persistence context (new or existing session)
    let ctx = PersistenceContext::new(store, user_id, tenant_id, agent_id).await?;

    // Manually save user message
    let user_msg_id = ctx.save_user_message(&user_input).await?;

    // Stream response
    let mut stream = agent.chat_stream(&user_input).await?;
    let mut full_response = String::new();

    while let Some(result) = stream.next().await {
        if let Ok(text) = result {
            print!("{}", text);
            full_response.push_str(&text);
        }
    }

    // Manually save assistant response
    let assistant_msg_id = ctx.save_assistant_message(&full_response).await?;

    Ok(())
}
```

### When to Use Manual Persistence

- Fine-grained control over what's saved
- Custom metadata on messages
- Conditional persistence based on response quality
- Integration with existing transaction boundaries

## Database-Driven Agent Configuration

Load agent configuration from PostgreSQL database.

**Location:** `examples/agent_from_database_streaming/`

```rust
use mofa_sdk::persistence::{AgentStore, PostgresStore, PersistencePlugin};

#[tokio::main]
async fn main() -> Result<()> {
    let store = PostgresStore::connect(&database_url).await?;

    // Load agent config from database
    let config = store
        .get_agent_by_code_and_tenant_with_provider(tenant_id, "chat-assistant")
        .await?
        .ok_or_else(|| format!("Agent not found"))?;

    // Create persistence plugin
    let persistence = PersistencePlugin::from_store(
        "persistence-plugin",
        store,
        user_id,
        tenant_id,
        config.agent.id,
        session_id,
    );

    // Build agent from database config
    let agent = LLMAgentBuilder::from_agent_config(&config)?
        .with_persistence_plugin(persistence)
        .build_async()
        .await;

    // Stream with database-backed session
    let mut stream = agent.chat_stream(&user_input).await?;
    // ...

    Ok(())
}
```

### Database Schema

Required tables:
- `entity_agent` - Agent configurations
- `entity_provider` - LLM provider configurations
- `entity_session` - Conversation sessions
- `entity_message` - Message history

## Running Examples

```bash
# Initialize database
psql -d your-database -f scripts/sql/migrations/postgres_init.sql

# Set environment variables
export DATABASE_URL="postgres://user:pass@localhost:5432/mofa"
export OPENAI_API_KEY="sk-xxx"

# Run automatic persistence
cargo run -p streaming_persistence

# Run manual persistence
cargo run -p streaming_manual_persistence

# Run database-driven configuration
export AGENT_CODE="chat-assistant"
export USER_ID="550e8400-e29b-41d4-a716-446655440003"
cargo run -p agent_from_database_streaming
```

## Available Examples

| Example | Description |
|---------|-------------|
| `streaming_persistence` | Auto-persistence with sliding window |
| `streaming_manual_persistence` | Manual message persistence control |
| `agent_from_database_streaming` | Load agent config from database |

## See Also

- [Persistence Guide](../guides/persistence.md) — Detailed persistence concepts
- [API Reference: Persistence](../api-reference/foundation/persistence.md) — Persistence API
