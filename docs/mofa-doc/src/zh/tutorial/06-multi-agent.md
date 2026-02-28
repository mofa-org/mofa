# 第 6 章：多智能体协调

> **学习目标：** 理解为什么以及何时使用多个智能体，学习 7 种协调模式，构建链式和并行智能体流水线。

## 为什么需要多个智能体？

单个智能体可以做很多事情，但某些任务受益于**专业化分工**：

- **质量**："研究员"智能体收集事实，"写手"智能体撰写文章，"编辑"智能体润色——各自专注于最擅长的事
- **并行性**：多个智能体同时分析问题的不同方面
- **鲁棒性**：智能体可以辩论或投票，减少个体错误
- **可扩展性**：添加更多智能体而无需修改现有的

## 7 种协调模式

MoFA 支持七种编排多个智能体的模式。`mofa-kernel` 中的 `CoordinationPattern` 枚举定义了它们：

```rust
// crates/mofa-kernel/src/agent/components/coordinator.rs

pub enum CoordinationPattern {
    Sequential,                        // 链式：A → B → C
    Parallel,                          // 扇出：A、B、C 同时运行
    Hierarchical { supervisor_id: String }, // 主管委派给工人
    Consensus { threshold: f32 },      // 智能体投票，需达到阈值
    Debate { max_rounds: usize },      // 智能体辩论，改进答案
    MapReduce,                         // 拆分任务，并行处理，合并
    Voting,                            // 多数获胜
    Custom(String),                    // 你自己的模式
}
```

何时使用每种模式：

| 模式 | 使用场景 | 示例 |
|------|----------|------|
| **Sequential（链式）** | 任务有自然阶段 | 研究 → 写作 → 编辑 |
| **Parallel（并行）** | 子任务相互独立 | 分析代码 + 检查安全 + 审查风格 |
| **Hierarchical（层级）** | 需要监督/委派 | 经理将任务分配给专家 |
| **Consensus（共识）** | 需要达成一致 | 多智能体事实核查 |
| **Debate（辩论）** | 通过分歧提高质量 | 正反分析、同行评审 |
| **MapReduce** | 大量输入，统一处理 | 摘要 100 篇文档 |
| **Voting（投票）** | 简单多数决策 | 多模型分类 |

## Coordinator Trait

`Coordinator` trait 定义了智能体如何协同工作：

```rust
#[async_trait]
pub trait Coordinator: Send + Sync {
    async fn dispatch(
        &self,
        task: Task,
        ctx: &AgentContext,
    ) -> AgentResult<Vec<DispatchResult>>;

    async fn aggregate(
        &self,
        results: Vec<AgentOutput>,
    ) -> AgentResult<AgentOutput>;

    fn pattern(&self) -> CoordinationPattern;
    fn name(&self) -> &str;

    async fn select_agents(
        &self,
        task: &Task,
        ctx: &AgentContext,
    ) -> AgentResult<Vec<String>>;

    fn requires_all(&self) -> bool;
}
```

- **`dispatch`**：将任务发送给适当的智能体
- **`aggregate`**：将多个智能体的结果合并为一个输出
- **`select_agents`**：决定哪些智能体应处理给定任务
- **`pattern`**：返回协调策略

## 构建：链式和并行流水线

让我们使用 `MoFAAgent` 实现构建两个多智能体示例。

创建新项目：

```bash
cargo new multi_agent_demo
cd multi_agent_demo
```

编辑 `Cargo.toml`：

```toml
[package]
name = "multi_agent_demo"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

### 示例 1：顺序链

三个智能体组成流水线——每个转换前一个的输出：

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{
    AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentInput,
    AgentOutput, AgentResult, AgentState, MoFAAgent,
};
use mofa_sdk::runtime::run_agents;

// --- 分析文本的智能体 ---
struct AnalystAgent {
    id: String,
    state: AgentState,
}

impl AnalystAgent {
    fn new() -> Self {
        Self {
            id: "analyst-001".to_string(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for AnalystAgent {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { "分析师" }
    fn capabilities(&self) -> &AgentCapabilities {
        &AgentCapabilitiesBuilder::new().build()
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        let text = input.to_text();
        let analysis = format!(
            "分析结果：文本 '{}' 包含 {} 个单词和 {} 个字符。",
            text,
            text.split_whitespace().count(),
            text.len()
        );
        Ok(AgentOutput::text(analysis))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

// --- 改写文本的智能体 ---
struct WriterAgent {
    id: String,
    state: AgentState,
}

impl WriterAgent {
    fn new() -> Self {
        Self {
            id: "writer-001".to_string(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for WriterAgent {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { "写手" }
    fn capabilities(&self) -> &AgentCapabilities {
        &AgentCapabilitiesBuilder::new().build()
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        let analysis = input.to_text();
        let report = format!("报告：\n{}\n\n结论：文本处理成功完成。", analysis);
        Ok(AgentOutput::text(report))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

// --- 链式执行 ---
async fn run_chain(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    // 阶段 1：分析师
    let analyst = AnalystAgent::new();
    let outputs = run_agents(analyst, vec![AgentInput::text(input)]).await?;
    let analysis = outputs[0].to_text();
    println!("  [分析师] → {}", analysis);

    // 阶段 2：写手（接收分析师的输出）
    let writer = WriterAgent::new();
    let outputs = run_agents(writer, vec![AgentInput::text(&analysis)]).await?;
    let report = outputs[0].to_text();
    println!("  [写手]   → {}", report);

    Ok(report)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 顺序链式：分析师 → 写手 ===\n");
    let result = run_chain("MoFA 是一个用 Rust 构建的模块化智能体框架").await?;
    println!("\n最终输出：\n{}", result);

    Ok(())
}
```

### 示例 2：并行执行

多个智能体并发处理相同的输入，然后聚合结果：

```rust
use tokio::task::JoinSet;

async fn run_parallel(input: &str) -> Result<Vec<String, Box<dyn std::error::Error>>> {
    let mut tasks = JoinSet::new();

    // 并行启动多个智能体
    let input_clone = input.to_string();
    tasks.spawn(async move {
        let agent = AnalystAgent::new();
        let outputs = run_agents(agent, vec![AgentInput::text(&input_clone)]).await?;
        Ok::<_, anyhow::Error>(outputs[0].to_text())
    });

    let input_clone = input.to_string();
    tasks.spawn(async move {
        let agent = WriterAgent::new();
        let outputs = run_agents(agent, vec![AgentInput::text(&input_clone)]).await?;
        Ok::<_, anyhow::Error>(outputs[0].to_text())
    });

    // 收集完成的结果
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result? {
            Ok(text) => results.push(text),
            Err(e) => eprintln!("智能体失败: {}", e),
        }
    }

    Ok(results)
}
```

> **Rust 提示：`JoinSet`**
> `tokio::task::JoinSet` 让你可以生成多个异步任务并在它们完成时收集结果。每个 `spawn` 返回一个 `JoinHandle`。`join_next().await` 返回下一个完成的任务。这就是在异步 Rust 中实现并行执行的方式。

## 使用 AgentTeam（Foundation）

对于更复杂的多智能体协调，MoFA 的 foundation 层提供了 `AgentTeam`：

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use mofa_foundation::llm::multi_agent::{AgentTeam, TeamPattern};

// 创建专业化的 LLM 智能体
let researcher = LLMAgentBuilder::new()
    .with_provider(provider.clone())
    .with_system_prompt("你是一个严谨的研究员。收集事实。")
    .build();

let writer = LLMAgentBuilder::new()
    .with_provider(provider.clone())
    .with_system_prompt("你是一个技艺精湛的写手。创作引人入胜的内容。")
    .build();

// 使用构建器模式创建团队
let team = AgentTeam::new("content-team")
    .with_name("内容团队")
    .add_member("researcher", Arc::new(researcher))
    .add_member("writer", Arc::new(writer))
    .with_pattern(TeamPattern::Chain)   // 顺序流水线
    .build();

let result = team.run("写一篇关于 Rust 的博客文章").await?;
```

可用的 `TeamPattern` 值：

```rust
pub enum TeamPattern {
    Chain,                          // 每个智能体的输出传递给下一个
    Parallel,                       // 所有智能体同时运行
    Debate { max_rounds: usize },   // 智能体在多轮中讨论和改进
    Supervised,                     // 监督者智能体评估结果
    MapReduce,                      // 并行处理后归约
    Custom,                         // 用户自定义模式（默认使用链式）
}
```

> **架构说明：** `AgentTeam` 位于 `mofa-foundation`（`crates/mofa-foundation/src/llm/multi_agent.rs`）。它在内部实现了 `mofa-kernel` 中的 `Coordinator` trait。参见 `examples/multi_agent_coordination/src/main.rs` 和 `examples/adaptive_collaboration_agent/src/main.rs` 获取完整的工作示例。

## 刚才发生了什么？

在链式示例中：
1. `AnalystAgent` 接收原始文本并产生分析结果
2. 分析结果成为 `WriterAgent` 的输入
3. 写手产生最终报告

在并行示例中：
1. 两个智能体同时接收相同的输入
2. 它们独立处理（通过 `tokio::spawn` 使用独立的操作系统线程）
3. 结果在完成时被收集——不保证顺序

`AgentTeam` 抽象为你处理 LLM 智能体的这些管道工作，包括：
- 智能体之间的自动消息格式化
- 错误处理和重试
- 根据选择的模式进行结果聚合

## 关键要点

- 多智能体协调实现了专业化、并行性和鲁棒性
- 7 种模式：Sequential、Parallel、Hierarchical、Consensus、Debate、MapReduce、Voting
- `Coordinator` trait 定义 `dispatch`、`aggregate` 和 `select_agents`
- 手动链式：顺序运行智能体，将输出作为下一个的输入传递
- 手动并行：使用 `tokio::task::JoinSet` 进行并发执行
- `AgentTeam` 为 LLM 智能体提供高级协调
- `TeamPattern` 选择编排策略

---

**下一章：** [第 7 章：StateGraph 工作流](07-workflows.md) — 构建基于图的有状态工作流。

[← 返回目录](README.md)

---

[English](../../tutorial/06-multi-agent.md) | **简体中文**
