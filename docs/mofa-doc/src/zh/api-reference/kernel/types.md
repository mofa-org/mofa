# 核心类型

MoFA 中使用的基本类型。

## AgentInput

智能体输入数据的包装器。

```rust
pub struct AgentInput {
    content: InputContent,
    metadata: HashMap<String, Value>,
    session_id: Option<String>,
}

pub enum InputContent {
    Text(String),
    Json(Value),
    Binary(Vec<u8>),
}

impl AgentInput {
    // 构造器
    pub fn text(content: impl Into<String>) -> Self;
    pub fn json(value: Value) -> Self;
    pub fn binary(data: Vec<u8>) -> Self;

    // 访问器
    pub fn to_text(&self) -> String;
    pub fn to_json(&self) -> Option<&Value>;
    pub fn as_binary(&self) -> Option<&[u8]>;

    // 元数据
    pub fn with_session_id(self, id: impl Into<String>) -> Self;
    pub fn with_metadata(self, key: &str, value: Value) -> Self;
    pub fn get_metadata(&self, key: &str) -> Option<&Value>;
}
```

### 用法

```rust
// 文本输入
let input = AgentInput::text("什么是 Rust?");

// JSON 输入
let input = AgentInput::json(json!({
    "query": "搜索词",
    "limit": 10
}));

// 带元数据
let input = AgentInput::text("你好")
    .with_session_id("session-123")
    .with_metadata("source", json!("web"));
```

## AgentOutput

智能体输出数据的包装器。

```rust
pub struct AgentOutput {
    content: OutputContent,
    metadata: OutputMetadata,
}

pub enum OutputContent {
    Text(String),
    Json(Value),
    Binary(Vec<u8>),
    Multi(Vec<AgentOutput>),
}

pub struct OutputMetadata {
    tokens_used: Option<u32>,
    latency_ms: Option<u64>,
    model: Option<String>,
    finish_reason: Option<String>,
}

impl AgentOutput {
    // 构造器
    pub fn text(content: impl Into<String>) -> Self;
    pub fn json(value: Value) -> Self;
    pub fn binary(data: Vec<u8>) -> Self;

    // 访问器
    pub fn as_text(&self) -> Option<&str>;
    pub fn as_json(&self) -> Option<&Value>;
    pub fn as_binary(&self) -> Option<&[u8]>;

    // 元数据
    pub fn with_tokens_used(self, tokens: u32) -> Self;
    pub fn with_latency_ms(self, ms: u64) -> Self;
    pub fn tokens_used(&self) -> Option<u32>;
    pub fn latency_ms(&self) -> Option<u64>;
}
```

### 用法

```rust
// 文本输出
let output = AgentOutput::text("你好，世界!");

// JSON 输出
let output = AgentOutput::json(json!({
    "answer": "42",
    "confidence": 0.95
}));

// 带元数据
let output = AgentOutput::text("响应")
    .with_tokens_used(150)
    .with_latency_ms(250);
```

## AgentState

智能体的生命周期状态。

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentState {
    Created,
    Ready,
    Executing,
    Paused,
    Error { message: String },
    Shutdown,
}
```

## AgentCapabilities

描述智能体能做什么。

```rust
pub struct AgentCapabilities {
    pub tags: Vec<String>,
    pub input_type: InputType,
    pub output_type: OutputType,
    pub max_concurrency: usize,
    pub supports_streaming: bool,
}

pub enum InputType {
    Text,
    Json,
    Binary,
    Any,
}

pub enum OutputType {
    Text,
    Json,
    Binary,
    Any,
}

impl AgentCapabilities {
    pub fn builder() -> AgentCapabilitiesBuilder;
}
```

### 用法

```rust
let capabilities = AgentCapabilities::builder()
    .tag("llm")
    .tag("qa")
    .input_type(InputType::Text)
    .output_type(OutputType::Text)
    .max_concurrency(10)
    .supports_streaming(true)
    .build();
```

## AgentError

智能体操作的错误类型。

```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("初始化失败: {0}")]
    InitializationFailed(String),

    #[error("执行失败: {0}")]
    ExecutionFailed(String),

    #[error("无效输入: {0}")]
    InvalidInput(String),

    #[error("工具未找到: {0}")]
    ToolNotFound(String),

    #[error("{0:?} 后超时")]
    Timeout(Duration),

    #[error("速率限制，{retry_after}秒后重试")]
    RateLimited { retry_after: u64 },

    #[error("未产生输出")]
    NoOutput,
}

pub type AgentResult<T> = Result<T, AgentError>;
```

## 另见

- [智能体 Trait](agent.md) — MoFAAgent 接口
- [上下文](context.md) — AgentContext 类型
