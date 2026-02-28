# 第 3 章：你的第一个智能体

> **学习目标：** 理解 `MoFAAgent` trait，从零实现它，并使用运行时的 `run_agents` 函数运行你的智能体。

## MoFAAgent Trait

MoFA 中的每个智能体都实现了定义在 `mofa-kernel` 中的 `MoFAAgent` trait。让我们来看看：

```rust
// crates/mofa-kernel/src/agent/core.rs

#[async_trait]
pub trait MoFAAgent: Send + Sync + 'static {
    // 身份标识
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn capabilities(&self) -> &AgentCapabilities;

    // 生命周期
    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()>;
    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput>;
    async fn shutdown(&mut self) -> AgentResult<()>;

    // 状态查询
    fn state(&self) -> AgentState;
}
```

这是每个智能体必须履行的契约。让我们逐一分析。

> **Rust 提示：`#[async_trait]`**
> Rust trait 原生尚不支持 `async fn` 方法。`async-trait` crate 中的 `async_trait` 宏通过将 `async fn` 转换为返回 `Pin<Box<dyn Future>>` 的方法来解决这个问题。你会在大多数 MoFA trait 上看到这个宏。

## 理解类型系统

### AgentInput

智能体接收的输入：

```rust
pub enum AgentInput {
    Text(String),           // 简单文本输入
    Texts(Vec<String>),     // 多个文本输入
    Json(serde_json::Value), // 结构化 JSON
    Map(HashMap<String, serde_json::Value>), // 键值对
    Binary(Vec<u8>),        // 二进制数据
    Empty,                  // 无输入
}
```

你可以轻松创建输入：

```rust
let input = AgentInput::text("你好，智能体！");
let input = AgentInput::json(serde_json::json!({"task": "greet", "name": "Alice"}));
```

### AgentOutput

智能体返回的输出：

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

最简单的创建方式：

```rust
AgentOutput::text("你好，人类！")
```

### AgentState

智能体经历的生命周期状态：

```
Created → Initializing → Ready → Running → Executing → Shutdown
                           ↕         ↕
                         Paused   Interrupted
```

目前最重要的状态：

```rust
pub enum AgentState {
    Created,     // 刚创建
    Ready,       // 已初始化，准备接收输入
    Running,     // 正在处理中
    Shutdown,    // 已停止
    // ... 还有更多（Paused、Failed、Error 等）
}
```

### AgentContext

传递给 `initialize` 和 `execute` 的执行上下文：

```rust
pub struct AgentContext {
    pub execution_id: String,
    pub session_id: Option<String>,
    // ... 内部字段
}
```

它提供：
- **键值状态**：`ctx.set("key", value)` / `ctx.get::<T>("key")`
- **事件总线**：`ctx.emit_event(event)` / `ctx.subscribe("event_type")`
- **中断处理**：`ctx.is_interrupted()` / `ctx.trigger_interrupt()`
- **层级上下文**：`ctx.child("sub-execution-id")`

## 构建：GreetingAgent

让我们实现一个简单的智能体，接收一个名字并返回问候。创建新的 Rust 项目：

```bash
cargo new greeting_agent
cd greeting_agent
```

编辑 `Cargo.toml`：

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

> **注意：** 我们使用 `path = "../../crates/mofa-sdk"` 引用本地工作区。当 MoFA 发布到 crates.io 时，你可以改用 `version = "0.1"`。

现在编写 `src/main.rs`：

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{
    AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentInput,
    AgentOutput, AgentResult, AgentState, MoFAAgent,
};
use mofa_sdk::runtime::run_agents;

// --- 定义我们的智能体 ---

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
        println!("[GreetingAgent] 正在初始化...");
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(
        &mut self,
        input: AgentInput,
        _ctx: &AgentContext,
    ) -> AgentResult<AgentOutput> {
        // 从输入中提取名字
        let name = match &input {
            AgentInput::Text(text) => text.clone(),
            _ => "World".to_string(),
        };

        let greeting = format!("你好，{}！欢迎来到 MoFA。", name);
        Ok(AgentOutput::text(greeting))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        println!("[GreetingAgent] 正在关闭...");
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState {
        self.state.clone()
    }
}

// --- 运行它 ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = GreetingAgent::new();

    // run_agents 处理完整的生命周期：
    // initialize → execute（对每个输入）→ shutdown
    let outputs = run_agents(
        agent,
        vec![
            AgentInput::text("Alice"),
            AgentInput::text("Bob"),
            AgentInput::text("GSoC 学生"),
        ],
    )
    .await?;

    for output in &outputs {
        println!("输出: {}", output.to_text());
    }

    Ok(())
}
```

运行它：

```bash
cargo run
```

预期输出：

```
[GreetingAgent] 正在初始化...
输出: 你好，Alice！欢迎来到 MoFA。
输出: 你好，Bob！欢迎来到 MoFA。
输出: 你好，GSoC 学生！欢迎来到 MoFA。
[GreetingAgent] 正在关闭...
```

## 刚才发生了什么？

让我们追踪执行过程：

1. **`GreetingAgent::new()`** — 创建一个处于 `AgentState::Created` 状态的智能体
2. **`run_agents(agent, inputs)`** — 运行时接管：
   - 调用 `agent.initialize(&ctx)` — 智能体转换到 `Ready`
   - 对每个输入，调用 `agent.execute(input, &ctx)` — 智能体处理输入
   - 调用 `agent.shutdown()` — 智能体转换到 `Shutdown`
3. **`outputs`** — 我们得到 `Vec<AgentOutput>`，每个输入对应一个

> **架构说明：** 注意我们的 `GreetingAgent` 只使用了 `mofa-kernel` 的类型（trait 和类型）和 `mofa-runtime` 的 `run_agents` 函数。我们不需要任何 foundation 代码，因为我们的智能体不使用 LLM、工具或持久化。这就是微内核在起作用——最小核心，其他一切都是可选的。

`run_agents` 函数位于 `mofa-runtime`（`crates/mofa-runtime/src/runner.rs`）。它是运行智能体的最简单方式。如果需要更多控制，你可以直接使用 `AgentRunner`：

```rust
use mofa_sdk::runtime::{AgentRunner, AgentRunnerBuilder};

let runner = AgentRunnerBuilder::new()
    .with_agent(GreetingAgent::new())
    .build();

// 带生命周期管理的运行
let result = runner.run(AgentInput::text("Alice")).await?;
```

## 使用 AgentContext 存储状态

`AgentContext` 被传递给 `initialize` 和 `execute`。你可以用它在执行之间存储状态：

```rust
async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()> {
    // 存储初始状态
    ctx.set("call_count", 0u32).await;
    self.state = AgentState::Ready;
    Ok(())
}

async fn execute(
    &mut self,
    input: AgentInput,
    ctx: &AgentContext,
) -> AgentResult<AgentOutput> {
    // 读取并更新状态
    let count: u32 = ctx.get("call_count").await.unwrap_or(0);
    ctx.set("call_count", count + 1).await;

    let name = input.to_text();
    let greeting = format!("你好，{}！你是第 {} 位来访者。", name, count + 1);
    Ok(AgentOutput::text(greeting))
}
```

> **Rust 提示：`Arc` 和 `RwLock`**
> 在 `AgentContext` 内部，状态存储在 `Arc<RwLock<HashMap<...>>>` 中。`Arc`（原子引用计数）让代码的多个部分共享数据的所有权。`RwLock` 允许多个读者或一个写者同时访问。这就是 Rust 在异步代码中安全处理共享可变状态的方式——不可能发生数据竞争。

## 关键要点

- 每个智能体实现 `MoFAAgent`，包含 7 个必需方法：`id`、`name`、`capabilities`、`initialize`、`execute`、`shutdown`、`state`
- `AgentInput` 是枚举——智能体可以接收文本、JSON、二进制数据或空值
- `AgentOutput::text("...")` 是返回响应的最简单方式
- `run_agents()` 处理完整的生命周期：initialize → execute → shutdown
- `AgentContext` 提供键值状态、事件和中断处理
- 你的智能体代码只使用 kernel trait 和 runtime 函数——不需要 LLM

---

**下一章：** [第 4 章：LLM 驱动的智能体](04-llm-agent.md) — 将你的智能体连接到真实的 LLM。

[← 返回目录](README.md)

---

[English](../../tutorial/03-first-agent.md) | **简体中文**
