# 第 9 章：下一步

> **学习目标：** 了解如何为 MoFA 做贡献，探索高级主题，找到 GSoC 旅程的资源。

恭喜！你已经从零构建了智能体，将它们连接到 LLM，赋予它们工具，编排了多智能体团队，设计了工作流，并编写了支持热重载的插件。你已经拥有了使用 MoFA 工作的扎实基础。

## 为 MoFA 做贡献

MoFA 是开源项目，欢迎贡献。以下是开始的方式：

### 1. 阅读贡献指南

[CONTRIBUTING.md](https://github.com/mofa-org/mofa/blob/main/CONTRIBUTING.md) 涵盖了：
- 分支命名约定（kebab-case：`feat/my-feature`、`fix/bug-name`）
- 提交消息格式（Conventional Commits：`feat:`、`fix:`、`docs:`）
- PR 指南和审查流程
- 架构规则（第 1 章中的 kernel/foundation 分离）

### 2. 找到 Issue

浏览 [GitHub Issues](https://github.com/moxin-org/mofa/issues) 查找：
- `good first issue` — 适合入门
- `help wanted` — 欢迎社区贡献
- `gsoc` — 为 GSoC 候选人标记

### 3. 开发工作流

```bash
# 创建功能分支
git checkout -b feat/my-feature

# 修改代码，然后检查
cargo check          # 快速编译检查
cargo fmt            # 格式化代码
cargo clippy         # 代码检查
cargo test           # 运行测试

# 提交（Conventional Commits 格式）
git commit -m "feat: add my new feature"

# 推送并创建 PR
git push -u origin feat/my-feature
```

## GSoC 项目想法

以下是 MoFA 可以受益于贡献的领域。这些可以作为优秀的 GSoC 项目提案：

### 新 LLM 提供者
- **难度**：中等
- **影响**：高
- 为新的 LLM API 添加提供者（Mistral、Cohere、本地模型服务器）
- 实现 `LLMProvider` trait（参见 `crates/mofa-foundation/src/llm/`）
- 参考：`openai.rs`、`anthropic.rs`、`ollama.rs` 的模式

### MCP 服务器集成
- **难度**：中高
- **影响**：高
- 构建 MCP（Model Context Protocol）服务器集成
- MoFA 已有 MCP 客户端支持（`mofa-kernel` trait、`mofa-foundation` 客户端）
- 扩展新的工具服务器、资源提供者或提示服务器

### 新内置工具
- **难度**：简单-中等
- **影响**：中等
- 创建有用的工具：数据库查询、API 客户端、代码执行器、网络爬虫
- 实现 `Tool` trait（第 5 章）
- 添加到 `mofa-plugins` 内置工具集合

### 持久化后端改进
- **难度**：中等
- **影响**：中等
- 改进现有的 PostgreSQL/MySQL/SQLite 后端
- 添加新后端（Redis、MongoDB、DynamoDB）
- 参见 `crates/mofa-foundation/src/persistence/`

### Python 绑定增强
- **难度**：中高
- **影响**：高
- 改进 `mofa-ffi` 中的 PyO3/UniFFI 绑定
- 使 Python API 更加 Pythonic
- 添加全面的 Python 示例和文档

### 监控仪表板
- **难度**：中等
- **影响**：中等
- 增强 `mofa-monitoring` 中基于 Axum 的 Web 仪表板
- 添加实时智能体可视化、指标图表、链路查看器
- 集成 OpenTelemetry 链路追踪

### 新示例
- **难度**：简单
- **影响**：中等
- 为真实使用场景创建示例智能体
- 完善文档（README + 内联注释）
- 好的示例：RAG 智能体、代码审查智能体、数据分析智能体

### 工作流引擎增强
- **难度**：中高
- **影响**：高
- 添加并行节点执行、子工作流、错误恢复
- 改进 YAML DSL 的功能
- 可视化工作流编辑器（基于 Web）

## 值得探索的高级主题

以下是我们在教程中未涵盖但可以探索的功能：

### 秘书智能体（人在回路中）
秘书智能体模式通过人工监督管理任务——适用于 AI 建议需要人工批准才能执行的工作流。

```
接收想法 → 澄清需求 → 调度智能体 →
监控反馈 → 推送决策给人类 → 更新待办事项
```

参见 `examples/secretary_agent/` 和 `examples/hitl_secretary/`。

### MCP 协议集成
MoFA 支持 Model Context Protocol，用于连接外部工具服务器：

```rust
use mofa_sdk::kernel::{McpClient, McpTool, McpToolRegistry};
```

参见 `crates/mofa-kernel/src/mcp/` 获取 trait，`crates/mofa-foundation/src/mcp/` 获取客户端。

### 持久化（PostgreSQL / SQLite）
将对话历史、智能体状态和会话数据存储到数据库中：

```rust
use mofa_sdk::persistence::{PersistencePlugin, PostgresStore};
```

参见 `examples/streaming_persistence/` 和 `examples/streaming_manual_persistence/`。

### FFI 绑定（Python、Java、Swift）
从其他语言调用 MoFA 智能体：

```python
# Python 示例（通过 PyO3）
from mofa import LLMAgent, OpenAIProvider

agent = LLMAgent(provider=OpenAIProvider.from_env())
response = agent.ask("来自 Python 的问候！")
```

参见 `crates/mofa-ffi/` 和 `examples/python_bindings/`。

### Dora 分布式数据流
将智能体作为分布式数据流图中的节点运行：

```rust
use mofa_sdk::dora::{DoraRuntime, run_dataflow};

let result = run_dataflow("dataflow.yml").await?;
```

参见 `dora` feature flag 和 `crates/mofa-runtime/src/dora/`。

### TTS（文字转语音）
通过 Kokoro TTS 集成让你的智能体拥有声音：

```rust
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_tts_plugin(tts_plugin)
    .build();

agent.chat_with_tts(&session_id, "讲个笑话").await?;
```

## 资源

- **仓库**：[github.com/moxin-org/mofa](https://github.com/moxin-org/mofa)
- **SDK 文档**：参见 `crates/mofa-sdk/README.md`
- **架构指南**：参见 `docs/architecture.md`
- **安全指南**：参见 `docs/security.md`
- **示例**：`examples/` 目录中有 27+ 个示例

### Rust 学习资源

如果你是 Rust 新手，以下资源可以补充本教程：

- [The Rust Book](https://doc.rust-lang.org/book/) — 官方指南
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) — 通过示例学习
- [Async Rust](https://rust-lang.github.io/async-book/) — 理解 async/await
- [Tokio 教程](https://tokio.rs/tokio/tutorial) — MoFA 使用的异步运行时

## 感谢

感谢你完成了本教程！无论你是为了 GSoC 还是只是在探索，我们希望 MoFA 能激励你构建出色的 AI 智能体。框架还年轻且在不断成长——你的贡献将塑造它的未来。

如果有问题，请在 GitHub 上提 issue 或加入社区讨论。我们期待看到你构建的作品！

---

[← 返回目录](README.md)

---

[English](../../tutorial/09-whats-next.md) | **简体中文**
