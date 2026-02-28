# Chapter 3: Your First Agent

> **Learning objectives:** Understand the `MoFAAgent` trait, implement it from scratch, and run your agent using the runtime's `run_agents` function.

## The MoFAAgent Trait

Every agent in MoFA implements the `MoFAAgent` trait, defined in `mofa-kernel`. Let's look at it:

```rust
// crates/mofa-kernel/src/agent/core.rs

#[async_trait]
pub trait MoFAAgent: Send + Sync + 'static {
    // Identity
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn capabilities(&self) -> &AgentCapabilities;

    // Lifecycle
    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()>;
    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput>;
    async fn shutdown(&mut self) -> AgentResult<()>;

    // State
    fn state(&self) -> AgentState;
}
```

This is the contract every agent must fulfill. Let's break down each part.

> **Rust tip: `#[async_trait]`**
> Rust traits don't natively support `async fn` methods yet. The `async_trait` macro from the `async-trait` crate works around this by transforming `async fn` into methods that return `Pin<Box<dyn Future>>`. You'll see this macro on most MoFA traits.

## Understanding the Types

### AgentInput

What the agent receives:

```rust
pub enum AgentInput {
    Text(String),           // Simple text input
    Texts(Vec<String>),     // Multiple text inputs
    Json(serde_json::Value), // Structured JSON
    Map(HashMap<String, serde_json::Value>), // Key-value pairs
    Binary(Vec<u8>),        // Binary data
    Empty,                  // No input
}
```

You can create inputs easily:

```rust
let input = AgentInput::text("Hello, agent!");
let input = AgentInput::json(serde_json::json!({"task": "greet", "name": "Alice"}));
```

### AgentOutput

What the agent returns:

```rust
pub struct AgentOutput {
    pub content: OutputContent,
    pub metadata: HashMap<String, serde_json::Value>,
    pub tools_used: Vec<ToolUsage>,
    pub reasoning_steps: Vec<ReasoningStep>,
    pub duration_ms: u64,
    pub token_usage: Option<TokenUsage>,
}
```

The simplest way to create one:

```rust
AgentOutput::text("Hello, human!")
```

### AgentState

The lifecycle states an agent transitions through:

```
Created → Initializing → Ready → Running → Executing → Shutdown
                           ↕         ↕
                         Paused   Interrupted
```

The most important states for now:

```rust
pub enum AgentState {
    Created,     // Just constructed
    Ready,       // Initialized and ready to accept input
    Running,     // Actively processing
    Shutdown,    // Stopped
    // ... and more (Paused, Failed, Error, etc.)
}
```

### AgentContext

The execution context passed to `initialize` and `execute`:

```rust
pub struct AgentContext {
    pub execution_id: String,
    pub session_id: Option<String>,
    // ... internal fields
}
```

It provides:
- **Key-value state**: `ctx.set("key", value)` / `ctx.get::<T>("key")`
- **Event bus**: `ctx.emit_event(event)` / `ctx.subscribe("event_type")`
- **Interrupt handling**: `ctx.is_interrupted()` / `ctx.trigger_interrupt()`
- **Hierarchical contexts**: `ctx.child("sub-execution-id")`

## Build: A GreetingAgent

Let's implement a simple agent that takes a name and returns a greeting. Create a new Rust project:

```bash
cargo new greeting_agent
cd greeting_agent
```

Edit `Cargo.toml`:

```toml
[package]
name = "greeting_agent"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

> **Note:** We use `path = "../../crates/mofa-sdk"` to reference the local workspace. When MoFA is published to crates.io, you'd use `version = "0.1"` instead.

Now write `src/main.rs`:

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{
    AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentInput,
    AgentOutput, AgentResult, AgentState, MoFAAgent,
};
use mofa_sdk::runtime::run_agents;

// --- Define our agent ---

struct GreetingAgent {
    id: String,
    name: String,
    caps: AgentCapabilities,
    state: AgentState,
}

impl GreetingAgent {
    fn new() -> Self {
        Self {
            id: "greeting-001".to_string(),
            name: "GreetingAgent".to_string(),
            caps: AgentCapabilitiesBuilder::new().build(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for GreetingAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &AgentCapabilities {
        &self.caps
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        println!("[GreetingAgent] Initializing...");
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(
        &mut self,
        input: AgentInput,
        _ctx: &AgentContext,
    ) -> AgentResult<AgentOutput> {
        // Extract the name from input
        let name = match &input {
            AgentInput::Text(text) => text.clone(),
            _ => "World".to_string(),
        };

        let greeting = format!("Hello, {}! Welcome to MoFA.", name);
        Ok(AgentOutput::text(greeting))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        println!("[GreetingAgent] Shutting down...");
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }
}

// --- Run it ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = GreetingAgent::new();

    // run_agents handles the full lifecycle:
    // initialize → execute (for each input) → shutdown
    let outputs = run_agents(
        agent,
        vec![
            AgentInput::text("Alice"),
            AgentInput::text("Bob"),
            AgentInput::text("GSoC Student"),
        ],
    )
    .await?;

    for output in &outputs {
        println!("Output: {}", output.to_text());
    }

    Ok(())
}
```

Run it:

```bash
cargo run
```

Expected output:

```
[GreetingAgent] Initializing...
Output: Hello, Alice! Welcome to MoFA.
Output: Hello, Bob! Welcome to MoFA.
Output: Hello, GSoC Student! Welcome to MoFA.
[GreetingAgent] Shutting down...
```

## What Just Happened?

Let's trace the execution:

1. **`GreetingAgent::new()`** — Creates an agent in `AgentState::Created`
2. **`run_agents(agent, inputs)`** — The runtime takes over:
   - Calls `agent.initialize(&ctx)` — agent transitions to `Ready`
   - For each input, calls `agent.execute(input, &ctx)` — agent processes input
   - Calls `agent.shutdown()` — agent transitions to `Shutdown`
3. **`outputs`** — We get back a `Vec<AgentOutput>`, one per input

> **Architecture note:** Notice that our `GreetingAgent` only uses types from `mofa-kernel` (the traits and types) and `mofa-runtime` (the `run_agents` function). We didn't need any foundation code because our agent doesn't use an LLM, tools, or persistence. This is the microkernel at work — minimal core, optional everything.

The `run_agents` function lives in `mofa-runtime` (`crates/mofa-runtime/src/runner.rs`). It's the simplest way to run an agent. For more control, you can use `AgentRunner` directly:

```rust
use mofa_sdk::runtime::{AgentRunner, AgentRunnerBuilder};

let runner = AgentRunnerBuilder::new()
    .with_agent(GreetingAgent::new())
    .build();

// Run with lifecycle management
let result = runner.run(AgentInput::text("Alice")).await?;
```

## Using AgentContext for State

The `AgentContext` is passed to both `initialize` and `execute`. You can use it to store state between executions:

```rust
async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()> {
    // Store initial state
    ctx.set("call_count", 0u32).await;
    self.state = AgentState::Ready;
    Ok(())
}

async fn execute(
    &mut self,
    input: AgentInput,
    ctx: &AgentContext,
) -> AgentResult<AgentOutput> {
    // Read and update state
    let count: u32 = ctx.get("call_count").await.unwrap_or(0);
    ctx.set("call_count", count + 1).await;

    let name = input.to_text();
    let greeting = format!("Hello, {}! You are caller #{}.", name, count + 1);
    Ok(AgentOutput::text(greeting))
}
```

> **Rust tip: `Arc` and `RwLock`**
> Inside `AgentContext`, the state is stored in `Arc<RwLock<HashMap<...>>>`. `Arc` (Atomic Reference Counting) lets multiple parts of the code share ownership of the data. `RwLock` allows multiple readers OR one writer at a time. This is how Rust handles shared mutable state safely in async code — no data races possible.

## Key Takeaways

- Every agent implements `MoFAAgent` with 7 required methods: `id`, `name`, `capabilities`, `initialize`, `execute`, `shutdown`, `state`
- `AgentInput` is an enum — agents can receive text, JSON, binary, or nothing
- `AgentOutput::text("...")` is the simplest way to return a response
- `run_agents()` handles the full lifecycle: initialize → execute → shutdown
- `AgentContext` provides key-value state, events, and interrupt handling
- Your agent code uses only kernel traits and runtime functions — no LLM needed

---

**Next:** [Chapter 4: LLM-Powered Agent](04-llm-agent.md) — Connect your agent to a real LLM.

[← Back to Table of Contents](README.md)

---

**English** | [简体中文](../zh-CN/tutorial/03-first-agent.md)
