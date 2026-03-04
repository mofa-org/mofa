# AgentContext

Execution context provided to agents during execution.

## Overview

`AgentContext` provides:
- Execution metadata (ID, timestamps)
- Session management
- Key-value storage for state
- Access to agent metadata

## Definition

```rust
pub struct AgentContext {
    execution_id: String,
    session_id: Option<String>,
    parent_id: Option<String>,
    metadata: AgentMetadata,
    storage: Arc<RwLock<HashMap<String, Value>>>,
    created_at: DateTime<Utc>,
}

impl AgentContext {
    // Constructors
    pub fn new(execution_id: impl Into<String>) -> Self;
    pub fn with_session(execution_id: &str, session_id: impl Into<String>) -> Self;

    // Accessors
    pub fn execution_id(&self) -> &str;
    pub fn session_id(&self) -> Option<&str>;
    pub fn parent_id(&self) -> Option<&str>;
    pub fn created_at(&self) -> DateTime<Utc>;
    pub fn metadata(&self) -> &AgentMetadata;

    // Key-value storage
    pub async fn set<T: Serialize>(&self, key: &str, value: T);
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>;
    pub async fn remove(&self, key: &str);
    pub async fn contains(&self, key: &str) -> bool;
    pub async fn clear(&self);
}
```

## Usage

### Creating Context

```rust
// Basic context
let ctx = AgentContext::new("exec-001");

// With session
let ctx = AgentContext::with_session("exec-001", "session-123");

// With metadata
let ctx = AgentContext::new("exec-001")
    .with_parent("parent-exec-002")
    .with_metadata("user_id", json!("user-456"));
```

### Using in Agent

```rust
async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
    // Get execution info
    let exec_id = ctx.execution_id();
    let session = ctx.session_id();

    // Store data
    ctx.set("last_query", input.to_text()).await;
    ctx.set("timestamp", chrono::Utc::now()).await;

    // Retrieve data
    let previous: Option<String> = ctx.get("last_query").await;

    // Use metadata
    if let Some(user_id) = ctx.metadata().get("user_id") {
        // User-specific logic
    }

    Ok(AgentOutput::text("Done"))
}
```

### Sharing State

```rust
// In one agent
ctx.set("research_results", json!({
    "findings": [...],
    "sources": [...]
})).await;

// In another agent (same session)
let results: Value = ctx.get("research_results").await.unwrap();
```

## Thread Safety

`AgentContext` uses `Arc<RwLock<...>>` for thread-safe storage:

```rust
// Can be cloned and shared
let ctx_clone = ctx.clone();

// Concurrent access is safe
tokio::spawn(async move {
    ctx_clone.set("key", "value").await;
});
```

## Best Practices

1. **Use sessions** for multi-turn conversations
2. **Store minimal data** — context is kept in memory
3. **Clear sensitive data** when no longer needed
4. **Use typed access** with `get<T>()` for type safety

## See Also

- [Agent Trait](agent.md) — MoFAAgent interface
- [Types](types.md) — Core types
