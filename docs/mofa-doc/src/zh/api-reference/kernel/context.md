# AgentContext

执行期间提供给智能体的执行上下文。

## 概述

`AgentContext` 提供:
- 执行元数据（ID、时间戳）
- 会话管理
- 用于状态的键值存储
- 访问智能体元数据

## 定义

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
    // 构造器
    pub fn new(execution_id: impl Into<String>) -> Self;
    pub fn with_session(execution_id: &str, session_id: impl Into<String>) -> Self;

    // 访问器
    pub fn execution_id(&self) -> &str;
    pub fn session_id(&self) -> Option<&str>;
    pub fn parent_id(&self) -> Option<&str>;
    pub fn created_at(&self) -> DateTime<Utc>;
    pub fn metadata(&self) -> &AgentMetadata;

    // 键值存储
    pub async fn set<T: Serialize>(&self, key: &str, value: T);
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>;
    pub async fn remove(&self, key: &str);
    pub async fn contains(&self, key: &str) -> bool;
    pub async fn clear(&self);
}
```

## 用法

### 创建上下文

```rust
// 基本上下文
let ctx = AgentContext::new("exec-001");

// 带会话
let ctx = AgentContext::with_session("exec-001", "session-123");

// 带元数据
let ctx = AgentContext::new("exec-001")
    .with_parent("parent-exec-002")
    .with_metadata("user_id", json!("user-456"));
```

### 在智能体中使用

```rust
async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
    // 获取执行信息
    let exec_id = ctx.execution_id();
    let session = ctx.session_id();

    // 存储数据
    ctx.set("last_query", input.to_text()).await;
    ctx.set("timestamp", chrono::Utc::now()).await;

    // 检索数据
    let previous: Option<String> = ctx.get("last_query").await;

    // 使用元数据
    if let Some(user_id) = ctx.metadata().get("user_id") {
        // 用户特定逻辑
    }

    Ok(AgentOutput::text("完成"))
}
```

### 共享状态

```rust
// 在一个智能体中
ctx.set("research_results", json!({
    "findings": [...],
    "sources": [...]
})).await;

// 在另一个智能体中（同一会话）
let results: Value = ctx.get("research_results").await.unwrap();
```

## 线程安全

`AgentContext` 使用 `Arc<RwLock<...>>` 实现线程安全存储:

```rust
// 可以克隆和共享
let ctx_clone = ctx.clone();

// 并发访问是安全的
tokio::spawn(async move {
    ctx_clone.set("key", "value").await;
});
```

## 最佳实践

1. **使用会话**用于多轮对话
2. **存储最少数据** — 上下文保存在内存中
3. **不再需要时清除敏感数据**
4. **使用类型化访问** `get<T>()` 确保类型安全

## 另见

- [智能体 Trait](agent.md) — MoFAAgent 接口
- [类型](types.md) — 核心类型
