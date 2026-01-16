# MoFA 架构文档

## 概述

MoFA (Model-based Framework for Agents) 是一个生产级 AI 智能体框架，采用**微内核 + 双层插件系统**架构设计。本文档描述 MoFA 的层次架构、职责划分和设计原则。

## 微内核架构原则

MoFA 严格遵循以下微内核架构设计原则：

1. **核心最小化**：内核只提供最基本的抽象和能力
2. **插件化扩展**：所有非核心功能通过插件机制提供
3. **清晰的层次**：每一层有明确的职责边界
4. **统一接口**：同类组件使用统一的抽象接口
5. **正确的依赖方向**：上层依赖下层，下层不依赖上层

## 层次架构

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        用户层 (User Code)                                │
│                                                                          │
│  用户代码：直接使用高级 API 构建 Agent                                   │
│  - 用户实现 MoFAAgent trait                                            │
│  - 使用 AgentBuilder 构建 Agent                                         │
│  - 使用 Runtime 管理 Agent 生命周期                                     │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                    SDK层 (mofa-sdk)                                      │
│  统一API入口：重新导出各层类型，提供跨语言绑定                            │
│                                                                          │
│  模块组织：                                                              │
│  - kernel: 核心抽象层 (MoFAAgent, AgentContext, etc.)                   │
│  - runtime: 运行时层 (AgentBuilder, SimpleRuntime, etc.)                │
│  - foundation: 业务层 (llm, secretary, react, etc.)                    │
│  - 顶层便捷导出：常用类型直接导入                                         │
│                                                                          │
│  特性：                                                                  │
│  - 单一导入点 (use mofa_sdk::*)                                        │
│  - Feature flags 控制可选能力                                           │
│  - 跨语言绑定 (UniFFI, PyO3)                                            │
│  - 向后兼容层                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                 业务层 (mofa-foundation)                                 │
│  业务功能和具体实现                                                      │
│                                                                          │
│  核心模块：                                                              │
│  - llm: LLM 集成 (OpenAI provider)                                      │
│  - secretary: 秘书 Agent 模式                                           │
│  - react: ReAct 模式实现                                                │
│  - workflow: 工作流编排                                                 │
│  - coordination: 多 Agent 协调                                          │
│  - collaboration: 自适应协作协议                                         │
│  - persistence: 持久化层                                                │
│  - prompt: 提示词工程                                                   │
│                                                                          │
│  职责：                                                                  │
│  - 提供生产就绪的 Agent 实现                                            │
│  - 实现业务逻辑和协作模式                                                │
│  - 集成外部服务 (LLM, 数据库等)                                         │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                运行时层 (mofa-runtime)                                    │
│  Agent 生命周期和执行管理                                                 │
│                                                                          │
│  核心组件：                                                              │
│  - AgentBuilder: 构建器模式                                             │
│  - AgentRunner: 执行器                                                  │
│  - SimpleRuntime: 多 Agent 协调 (非 dora 模式)                           │
│  - AgentRuntime: Dora-rs 集成 (可选)                                    │
│  - 消息总线和事件路由                                                   │
│                                                                          │
│  职责：                                                                  │
│  - 管理 Agent 生命周期 (初始化、启动、停止、销毁)                        │
│  - 提供 Agent 执行环境                                                  │
│  - 处理 Agent 间通信                                                    │
│  - 支持插件系统                                                         │
│                                                                          │
│  依赖：                                                                  │
│  - mofa-kernel: 核心抽象                                                │
│  - mofa-plugins: 插件系统                                               │
│  - (可选) mofa-monitoring: 监控功能                                     │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              抽象层 (mofa-kernel/agent/)                                  │
│  核心抽象和扩展                                                           │
│                                                                          │
│  核心 Trait：                                                            │
│  - MoFAAgent: 核心 trait (id, name, capabilities, execute, etc.)        │
│                                                                          │
│  扩展 Trait (可选)：                                                     │
│  - AgentLifecycle: pause, resume, interrupt                            │
│  - AgentMessaging: handle_message, handle_event                         │
│  - AgentPluginSupport: 插件管理                                         │
│                                                                          │
│  核心类型：                                                              │
│  - AgentContext: 执行上下文                                              │
│  - AgentInput/AgentOutput: 输入输出                                      │
│  - AgentState: Agent 状态                                               │
│  - AgentCapabilities: 能力描述                                          │
│  - AgentMetadata: 元数据                                                │
│  - AgentError/AgentResult: 错误处理                                     │
│                                                                          │
│  职责：                                                                  │
│  - 定义统一的 Agent 接口                                                 │
│  - 提供核心类型和抽象                                                    │
│  - 支持通过 trait 组合扩展功能                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              核心层 (mofa-kernel)                                         │
│  最小化核心基础设施 - 无业务逻辑                                         │
│                                                                          │
│  核心模块：                                                              │
│  - context: 上下文管理                                                  │
│  - plugin: 插件系统接口                                                 │
│  - bus: 事件总线                                                        │
│  - message: 消息类型                                                    │
│  - core: 核心类型                                                       │
│  - logging: 日志系统                                                    │
│                                                                          │
│  职责：                                                                  │
│  - 提供最基础的数据结构                                                 │
│  - 实现事件总线和消息传递                                               │
│  - 定义插件接口                                                         │
│  - 无任何业务逻辑                                                       │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              插件系统 (mofa-plugins)                                      │
│  双层插件架构                                                            │
│                                                                          │
│  编译时插件：                                                            │
│  - Rust/WASM 插件                                                       │
│  - 零成本抽象                                                           │
│  - 性能关键路径                                                         │
│                                                                          │
│  运行时插件：                                                            │
│  - Rhai 脚本引擎                                                        │
│  - 热重载支持                                                           │
│  - 业务逻辑扩展                                                         │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              监控层 (mofa-monitoring) [可选]                              │
│  可观测性和指标                                                          │
│  - Web 仪表板                                                           │
│  - 指标收集                                                             │
│  - 分布式追踪                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

## 依赖关系

```
用户代码
    ↓
SDK层 (mofa-sdk)
    ↓
├──→ 业务层 (mofa-foundation)
│        ↓
│   ├──→ 运行时层 (mofa-runtime)
│   │        ↓
│   │    └──→ 抽象层 (mofa-kernel/agent/)
│   │             ↓
│   │          └──→ 核心层 (mofa-kernel)
│   │
│   └──→ 抽象层 (mofa-kernel/agent/)
│          ↓
│       核心层 (mofa-kernel)
│
└──→ 运行时层 (mofa-runtime)
         ↓
      ├──→ 抽象层 (mofa-kernel/agent/)
      │        ↓
      │     核心层 (mofa-kernel)
      │
      └──→ 插件系统 (mofa-plugins)
               ↓
            核心层 (mofa-kernel)
```

**关键规则**：上层依赖下层，下层不依赖上层。

## 各层职责

### 用户层
- 实现 Agent 业务逻辑
- 使用 SDK 提供的 API

### SDK层
- 统一 API 入口
- 重新导出各层功能
- 提供跨语言绑定
- 维护向后兼容性

### 业务层
- LLM 集成
- Agent 模式实现 (ReAct, Secretary, etc.)
- 工作流编排
- 协作协议
- 持久化

### 运行时层
- Agent 生命周期管理
- 执行环境
- 事件路由
- 插件支持

### 抽象层
- MoFAAgent 核心接口
- 扩展 trait
- 核心类型定义

### 核心层
- 基础数据结构
- 事件总线
- 消息传递
- 插件接口

### 插件系统
- 编译时插件 (Rust/WASM)
- 运行时插件 (Rhai 脚本)

### 监控层
- 可观测性
- 指标收集
- 分布式追踪

## 使用示例

### 基础用法

```rust
use mofa_sdk::{AgentBuilder, MoFAAgent, run_agent};
use mofa_sdk::AgentInput;
use async_trait::async_trait;

struct MyAgent;

#[async_trait]
impl MoFAAgent for MyAgent {
    fn id(&self) -> &str { "my-agent" }
    fn name(&self) -> &str { "My Agent" }
    fn capabilities(&self) -> &AgentCapabilities {
        &self.caps
    }

    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()> {
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
        Ok(AgentOutput::text("Hello!"))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        Ok(())
    }

    fn state(&self) -> AgentState {
        AgentState::Ready
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_agent(MyAgent).await
}
```

### 使用 LLM

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = openai_from_env()?;
    let client = LLMClient::new(std::sync::Arc::new(provider));
    let response = client.ask("What is Rust?").await?;
    println!("{}", response);
    Ok(())
}
```

### 多 Agent 协调

```rust
use mofa_sdk::{SimpleRuntime, AgentBuilder, MoFAAgent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime = SimpleRuntime::new();

    // 注册多个 agent
    let agent1 = MyAgent1::new();
    let agent2 = MyAgent2::new();

    runtime.register_agent(agent1.metadata(), agent1.config(), "worker").await?;
    runtime.register_agent(agent2.metadata(), agent2.config(), "worker").await?;

    // 启动运行时
    runtime.start().await?;

    Ok(())
}
```

## 设计决策

### 为什么采用微内核架构？

1. **可扩展性**：通过插件系统轻松扩展功能
2. **灵活性**：用户可以只依赖需要的层
3. **可维护性**：清晰的层次边界使代码易于维护
4. **可测试性**：每层可以独立测试

### 为什么 SDK 不只依赖 Foundation？

虽然微内核架构强调分层，但 SDK 作为统一的 API 入口，需要：

1. 暴露 Runtime 的运行时管理功能
2. 暴露 Kernel 的核心抽象
3. 暴露 Foundation 的业务功能

因此 SDK 作为 **facade**，重新导出各层的功能，而不是逐层依赖。

### 为什么 Foundation 和 Runtime 是平级关系？

- Foundation 提供**业务能力**（LLM、持久化、模式等）
- Runtime 提供**执行环境**（生命周期管理、事件路由等）

两者职责不同，互不依赖，都依赖 Kernel 提供的核心抽象。

## 未来改进

1. **更严格的依赖检查**：使用 `cargo deny` 等工具防止错误的依赖方向
2. **更细粒度的 feature flags**：减少编译时间
3. **更完整的文档**：每个模块都有详细的文档和示例
4. **性能优化**：优化关键路径的性能
5. **更好的错误处理**：统一的错误处理机制

## 参考资料

- [Agent 重构提案](./agent_refactoring_proposal.md)
- [秘书 Agent 使用指南](./secretary_agent_usage.md)
- [自适应协作协议](./adaptive_collaboration.md)
