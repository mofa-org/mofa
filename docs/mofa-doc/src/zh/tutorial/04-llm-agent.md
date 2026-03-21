# 第 4 章：LLM 驱动的智能体

> **学习目标：** 将智能体连接到真实的 LLM，使用 `LLMAgentBuilder`，处理流式响应，以及管理多轮对话。

## MoFA 中的 LLM 提供者

MoFA 开箱即用支持四种 LLM 提供者：

| 提供者 | Crate | 辅助函数 | 需要 |
|--------|-------|----------|------|
| **OpenAI** | `async-openai` | `OpenAIProvider::from_env()` | `OPENAI_API_KEY` |
| **Anthropic** | 自定义 | `AnthropicProvider::from_env()` | `ANTHROPIC_API_KEY` |
| **Google Gemini** | 自定义 | `GeminiProvider::from_env()` | `GOOGLE_API_KEY` |
| **Ollama** | 自定义 | `OllamaProvider::default()` | Ollama 本地运行 |

所有提供者都实现了 `mofa-kernel` 中的 `LLMProvider` trait：

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    fn default_model(&self) -> &str;
    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse>;
}
```

> **架构说明：** `LLMProvider` trait 定义在 `mofa-kernel`（契约），而 `OpenAIProvider`、`OllamaProvider` 等位于 `mofa-foundation`（实现）。这就是微内核模式的工作方式——你可以通过实现这个 trait 创建自己的提供者。

## LLMAgentBuilder

MoFA 提供了 `LLMAgentBuilder`——一个流式构建器，不需要手动实现 `MoFAAgent`（如第 3 章），只需几行代码即可创建功能完整的 LLM 智能体：

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;

let agent = LLMAgentBuilder::new()
    .with_id("my-agent")
    .with_name("我的助手")
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_system_prompt("你是一个乐于助人的 AI 助手。")
    .with_temperature(0.7)
    .with_max_tokens(2048)
    .build();
```

构建器支持多种选项：

| 方法 | 用途 |
|------|------|
| `.with_id(id)` | 设置智能体 ID |
| `.with_name(name)` | 设置显示名称 |
| `.with_provider(provider)` | 设置 LLM 提供者（必需） |
| `.with_system_prompt(prompt)` | 设置系统提示词 |
| `.with_temperature(t)` | 设置采样温度（0.0-2.0） |
| `.with_max_tokens(n)` | 设置最大响应 token 数 |
| `.with_model(model)` | 覆盖默认模型名称 |
| `.with_session_id(id)` | 设置初始会话 ID |
| `.with_sliding_window(n)` | 限制对话上下文窗口 |
| `.from_env()` | 从环境变量自动检测提供者 |

> **Rust 提示：`Arc<dyn Trait>`**
> `Arc::new(OpenAIProvider::from_env())` 将提供者包装在 `Arc`（原子引用计数指针）中。这是因为智能体及其内部组件需要共享同一个提供者。`dyn LLMProvider` 表示"任何实现了 `LLMProvider` 的类型"——这是 Rust 的动态分发，类似于 C++ 中的虚方法调用或 Java 中的接口引用。

## 构建：流式聊天机器人

让我们构建一个支持流式响应和会话上下文的聊天机器人。

创建新项目：

```bash
cargo new llm_chatbot
cd llm_chatbot
```

编辑 `Cargo.toml`：

```toml
[package]
name = "llm_chatbot"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
```

编写 `src/main.rs`：

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 第 1 步：创建提供者 ---
    let provider = Arc::new(OpenAIProvider::from_env());

    // --- 第 2 步：构建智能体 ---
    let agent = LLMAgentBuilder::new()
        .with_id("chatbot-001")
        .with_name("教程聊天机器人")
        .with_provider(provider)
        .with_system_prompt(
            "你是一个友好的 AI 导师，帮助学生了解 MoFA 智能体框架。回答要简洁。"
        )
        .with_temperature(0.7)
        .build();

    // --- 第 3 步：简单问答（非流式） ---
    println!("=== 简单问答 ===");
    let response = agent.ask("什么是微内核架构？").await?;
    println!("回答: {}\n", response);

    // --- 第 4 步：流式响应 ---
    println!("=== 流式响应 ===");
    let mut stream = agent.ask_stream("用 3 句话解释 Rust 中的 trait。").await?;
    print!("回答: ");
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(text) => print!("{}", text),
            Err(e) => eprintln!("\n流式错误: {}", e),
        }
    }
    println!("\n");

    // --- 第 5 步：多轮对话 ---
    println!("=== 多轮对话 ===");
    let r1 = agent.chat("我叫 Alice，我在学习 Rust。").await?;
    println!("回答: {}\n", r1);

    let r2 = agent.chat("我叫什么名字？我在学什么？").await?;
    println!("回答: {}\n", r2);
    // 智能体记住了上一条消息的上下文！

    Ok(())
}
```

运行它：

```bash
cargo run
```

## 使用 Ollama 替代

要使用本地 Ollama 模型，只需替换提供者：

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OllamaProvider};

let provider = Arc::new(OllamaProvider::default());
// Ollama 默认使用 http://localhost:11434

let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_model("llama3.2")  // 指定使用哪个 Ollama 模型
    .with_system_prompt("你是一个乐于助人的助手。")
    .build();
```

或使用 `from_env()` 便捷方法自动检测提供者：

```rust
// 检查 OPENAI_API_KEY、ANTHROPIC_API_KEY、GOOGLE_API_KEY，
// 如果都未设置则回退到 Ollama
let builder = LLMAgentBuilder::from_env()?;
let agent = builder
    .with_system_prompt("你是一个乐于助人的助手。")
    .build();
```

## 刚才发生了什么？

让我们追踪调用 `agent.ask("问题")` 时发生了什么：

1. `LLMAgent` 将你的问题包装为角色为 `"user"` 的 `ChatMessage`
2. 在前面添加角色为 `"system"` 的系统提示词 `ChatMessage`
3. 构建包含温度、最大 token 数等参数的 `ChatCompletionRequest`
4. 调用 `provider.chat(request)` 将请求发送到 LLM API
5. 解包响应 `ChatCompletionResponse` 并返回文本内容

对于 `agent.chat()`（多轮对话），智能体还会：
- 将用户消息存储到当前 `ChatSession`
- 存储助手的响应
- 在下一次请求中包含所有之前的消息（对话上下文）

对于 `agent.ask_stream()` 和 `agent.chat_stream()`：
- 提供者返回 `TextStream`（字符串块的流）
- 你在循环中用 `StreamExt::next()` 消费它
- 每个块包含响应生成过程中的一部分

> **架构说明：** `LLMAgent` 结构体位于 `mofa-foundation`（`crates/mofa-foundation/src/llm/agent.rs`）。它在内部实现了 `MoFAAgent` trait，所以具有相同的生命周期（initialize → execute → shutdown）。构建器模式是一种便捷方式——底层它构造了 `LLMAgentConfig` 并传递给 `LLMAgent::new()`。

## 会话管理

每个 `LLMAgent` 管理多个聊天会话。这对服务多个用户或维护独立的对话线程很有用：

```rust
// 创建新会话（返回会话 ID）
let session_id = agent.create_session().await;

// 在特定会话中聊天
let r1 = agent.chat_with_session(&session_id, "你好！").await?;

// 切换活跃会话
agent.switch_session(&session_id).await?;

// 列出所有会话
let sessions = agent.list_sessions().await;

// 获取或创建指定 ID 的会话
let sid = agent.get_or_create_session("user-123-session").await;
```

## 从配置文件加载

在生产环境中，你可以用 YAML 定义智能体配置：

```yaml
# agent.yml
agent:
  id: "my-agent-001"
  name: "我的 LLM 智能体"
  description: "一个乐于助人的助手"

llm:
  provider: openai
  model: gpt-4o
  api_key: ${OPENAI_API_KEY}
  temperature: 0.7
  max_tokens: 4096
  system_prompt: |
    你是一个乐于助人的 AI 助手。
```

在代码中加载：

```rust
use mofa_sdk::llm::agent_from_config;

let agent = agent_from_config("agent.yml")?;
let response = agent.ask("你好！").await?;
```

## 关键要点

- `LLMAgentBuilder` 是创建 LLM 驱动智能体的推荐方式
- 支持四种提供者：OpenAI、Anthropic、Gemini、Ollama
- `agent.ask()` 用于一次性问题，`agent.chat()` 用于多轮对话
- `agent.ask_stream()` / `agent.chat_stream()` 用于流式响应
- 会话管理支持多用户和多线程对话
- `from_env()` 从环境变量自动检测提供者
- 配置文件（`agent.yml`）适用于生产部署

---

**下一章：** [第 5 章：工具与函数调用](05-tools.md) — 赋予你的智能体调用函数的能力。

[← 返回目录](README.md)

---

[English](../../tutorial/04-llm-agent.md) | **简体中文**
