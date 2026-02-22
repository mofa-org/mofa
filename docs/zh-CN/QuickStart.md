# MoFA 快速开始

> 10 分钟内从零开始运行一个智能体。

---

## 前置条件

- **Rust** stable 工具链（edition 2024 — 需要 Rust ≥ 1.85）
- **Git**

### 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

#### 验证安装

```bash
rustc --version   # 1.85.0 或更新版本
cargo --version
```

#### Windows

使用 [rustup.rs](https://rustup.rs) 的安装程序。确保 `%USERPROFILE%\.cargo\bin` 在你的 `PATH` 环境变量中。

#### macOS (Homebrew)

```bash
brew install rustup
rustup-init
```

---

## 获取源码

```bash
git clone https://github.com/mofa-org/mofa.git
cd mofa
```

---

## 构建项目

```bash
# 构建整个工作空间
cargo build

# 发布构建（优化版）
cargo build --release

# 构建单个 crate
cargo build -p mofa-sdk
```

### 验证编译和测试

```bash
cargo check          # 快速检查，不生成产物
cargo test           # 完整测试套件
cargo test -p mofa-sdk   # 仅测试 SDK
```

---

## 配置 IDE

**VS Code**（推荐）：

1. 安装 [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) 扩展。
2. 打开工作空间根目录 — `rust-analyzer` 会自动识别 `Cargo.toml`。

**JetBrains RustRover / IntelliJ + Rust 插件**：打开文件夹，让 IDE 索引 Cargo 工作空间。

> 参见 [CONTRIBUTING.md](../../CONTRIBUTING.md) 了解编辑代码前需要知道的架构规则。

---

## 配置 LLM 环境

MoFA 支持 **OpenAI**、**Anthropic**、**Google Gemini** 以及任何 **OpenAI 兼容端点**（Ollama、vLLM、OpenRouter 等）。

在项目根目录创建 `.env` 文件（示例中使用的 `dotenvy` 助手会自动加载）：

### OpenAI

```env
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o           # 可选，默认：gpt-4o
```

### Anthropic

```env
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-sonnet-4-5-latest   # 可选
```

### OpenAI 兼容端点（Ollama、vLLM、OpenRouter 等）

```env
OPENAI_API_KEY=ollama          # 或你的密钥
OPENAI_BASE_URL=http://localhost:11434/v1
OPENAI_MODEL=llama3.2
```

### Google Gemini（通过 OpenRouter）

```env
OPENAI_API_KEY=<你的_openrouter_密钥>
OPENAI_BASE_URL=https://openrouter.ai/api/v1
OPENAI_MODEL=google/gemini-2.0-flash-001
```

---

## 你的第一个智能体 — 分步指南

在 `Cargo.toml` 中添加 `mofa-sdk` 和 `tokio`：

```toml
[dependencies]
mofa-sdk = { path = "../mofa/crates/mofa-sdk" }   # 开发时的本地路径
tokio    = { version = "1", features = ["full"] }
dotenvy  = "0.15"
```

然后编写你的智能体：

```rust
//! 最简单的 MoFA 智能体示例：使用 LLM 回答问题。

use std::sync::Arc;
use dotenvy::dotenv;
use mofa_sdk::kernel::agent::prelude::*;
use mofa_sdk::llm::{LLMClient, openai_from_env};

struct LLMAgent {
    id: String,
    name: String,
    capabilities: AgentCapabilities,
    state: AgentState,
    client: LLMClient,
}

impl LLMAgent {
    fn new(client: LLMClient) -> Self {
        Self {
            id: "llm-agent-1".to_string(),
            name: "LLM Agent".to_string(),
            capabilities: AgentCapabilities::builder()
                .tag("llm").tag("qa")
                .input_type(InputType::Text)
                .output_type(OutputType::Text)
                .build(),
            state: AgentState::Created,
            client,
        }
    }
}

#[async_trait]
impl MoFAAgent for LLMAgent {
    fn id(&self)           -> &str               { &self.id }
    fn name(&self)         -> &str               { &self.name }
    fn capabilities(&self) -> &AgentCapabilities { &self.capabilities }
    fn state(&self)        -> AgentState         { self.state.clone() }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        self.state = AgentState::Executing;
        let answer = self.client
            .ask_with_system("你是一个乐于助人的 Rust 专家。", &input.to_text())
            .await
            .map_err(|e| AgentError::ExecutionFailed(e.to_string()))?;
        self.state = AgentState::Ready;
        Ok(AgentOutput::text(answer))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();   // 重要：加载 .env 文件

    let provider = openai_from_env()?;
    let client   = LLMClient::new(Arc::new(provider));

    let mut agent = LLMAgent::new(client);
    let ctx       = AgentContext::new("exec-001");

    agent.initialize(&ctx).await?;

    let output = agent.execute(
        AgentInput::text("Rust 中的借用检查器是什么？"),
        &ctx,
    ).await?;

    println!("{}", output.as_text().unwrap_or("(无回答)"));
    agent.shutdown().await?;
    Ok(())
}
```

运行它：

```bash
cargo run
```

---

## 运行示例

`examples/` 目录包含 27+ 个可直接运行的演示。

```bash
# Echo / 无 LLM 基础示例
cargo run -p chat_stream

# ReAct 智能体（推理 + 工具调用）
cargo run -p react_agent

# Secretary 智能体（人机协作）
cargo run -p secretary_agent

# 多智能体协作模式
cargo run -p multi_agent_coordination

# Rhai 热重载脚本
cargo run -p rhai_hot_reload

# 自适应协作
cargo run -p adaptive_collaboration_agent
```

> 所有示例都从环境变量或本地 `.env` 文件读取凭证。

完整列表参见 [examples/README.md](../../examples/README.md)。

---

## 下一步

| 目标 | 参考文档 |
|---|---|
| 架构深入了解 | [CLAUDE.md](../../CLAUDE.md) |
| API 参考 | [架构文档](architecture.md) |
| 添加自定义 LLM 提供商 | 实现 `mofa_sdk::llm` 中的 `LLMProvider` |
| 编写 Rhai 运行时插件 | `examples/rhai_scripting/` |
| 构建 WASM 插件 | `examples/wasm_plugin/` |
| 贡献修复或功能 | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 提问 | [GitHub Discussions](https://github.com/mofa-org/mofa/discussions) · [Discord](https://discord.com/invite/hKJZzDMMm9) |

---

[English](../QuickStart.md) | **简体中文**
