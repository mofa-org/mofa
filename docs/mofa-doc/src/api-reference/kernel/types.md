# Core Types

Essential types used throughout MoFA.

## AgentInput

Wrapper for input data to agents.

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
    // Constructors
    pub fn text(content: impl Into<String>) -> Self;
    pub fn json(value: Value) -> Self;
    pub fn binary(data: Vec<u8>) -> Self;

    // Accessors
    pub fn to_text(&self) -> String;
    pub fn to_json(&self) -> Option<&Value>;
    pub fn as_binary(&self) -> Option<&[u8]>;

    // Metadata
    pub fn with_session_id(self, id: impl Into<String>) -> Self;
    pub fn with_metadata(self, key: &str, value: Value) -> Self;
    pub fn get_metadata(&self, key: &str) -> Option<&Value>;
}
```

### Usage

```rust
// Text input
let input = AgentInput::text("What is Rust?");

// JSON input
let input = AgentInput::json(json!({
    "query": "search term",
    "limit": 10
}));

// With metadata
let input = AgentInput::text("Hello")
    .with_session_id("session-123")
    .with_metadata("source", json!("web"));
```

## AgentOutput

Wrapper for output data from agents.

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
    // Constructors
    pub fn text(content: impl Into<String>) -> Self;
    pub fn json(value: Value) -> Self;
    pub fn binary(data: Vec<u8>) -> Self;

    // Accessors
    pub fn as_text(&self) -> Option<&str>;
    pub fn as_json(&self) -> Option<&Value>;
    pub fn as_binary(&self) -> Option<&[u8]>;

    // Metadata
    pub fn with_tokens_used(self, tokens: u32) -> Self;
    pub fn with_latency_ms(self, ms: u64) -> Self;
    pub fn tokens_used(&self) -> Option<u32>;
    pub fn latency_ms(&self) -> Option<u64>;
}
```

### Usage

```rust
// Text output
let output = AgentOutput::text("Hello, world!");

// JSON output
let output = AgentOutput::json(json!({
    "answer": "42",
    "confidence": 0.95
}));

// With metadata
let output = AgentOutput::text("Response")
    .with_tokens_used(150)
    .with_latency_ms(250);
```

## AgentState

Lifecycle state of an agent.

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

Describes what an agent can do.

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

### Usage

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

Error type for agent operations.

```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    #[error("No output produced")]
    NoOutput,
}

pub type AgentResult<T> = Result<T, AgentError>;
```

## See Also

- [Agent Trait](agent.md) — MoFAAgent interface
- [Context](context.md) — AgentContext type
