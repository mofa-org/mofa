# 第 1 章：介绍

> **学习目标：** 理解 MoFA 是什么，微内核架构如何工作，以及贯穿本教程的核心概念。

## 什么是 MoFA？

MoFA（Modular Framework for Agents）是一个用 Rust 构建的**生产级 AI 智能体框架**。它让你能够构建可以推理、使用工具、与其他智能体协作以及运行复杂工作流的智能代理。

**为什么选择 Rust 来构建 AI 智能体？**

- **性能**：原生速度的智能体编排，实时交互无 GC 停顿
- **安全**：编译器在构建时就捕获整类 bug（数据竞争、空指针）
- **并发**：`async/await` + `tokio` 运行时高效处理数千个并发智能体交互
- **多语言**：通过 UniFFI 绑定，你的 Rust 智能体可以被 Python、Java、Swift、Kotlin 和 Go 调用

## 微内核哲学

MoFA 采用了**微内核**架构，这一理念借鉴自操作系统设计。核心思想简单而强大：

> **内核定义契约（traits）。其他一切都是可插拔的实现。**

这意味着你可以在不触及核心的情况下替换 LLM 提供者、存储后端、工具注册表甚至脚本引擎。以下是 MoFA 10 个 crate 的组织方式：

```
 面向开发者的工具包
┌─────────────────────────────────────────────────────┐
│              mofa-sdk（开发工具包）                    │
│  你日常使用的 API：构建器辅助函数、重新导出、            │
│  便捷函数（openai_from_env 等）                       │
└──────────┬──────────────────────────────┬───────────┘
           │  使用                        │  使用
           ▼                              ▼
 框架核心                           扩展系统
┌────────────────────────┐  ┌─────────────────────────┐
│  mofa-runtime          │  │  mofa-plugins            │
│  AgentRunner、注册表、  │  │  Rhai 脚本、WASM、       │
│  事件循环、生命周期     │  │  热重载、TTS、            │
└──────────┬─────────────┘  │  内置工具                │
           │                └──────────┬──────────────┘
           ▼                           │
┌────────────────────────┐             │
│  mofa-foundation       │             │
│  LLM 提供者、智能体、   │◄────────────┘
│  工具、持久化、         │
│  工作流、秘书智能体     │
└──────────┬─────────────┘
           │
           ▼
┌────────────────────────┐
│  mofa-kernel           │
│  仅包含 Trait 定义      │
│  MoFAAgent、Tool、      │
│  Memory、Reasoner、     │
│  Coordinator、Plugin、  │
│  StateGraph            │
└────────────────────────┘

 外围 crate：
  mofa-cli        CLI 工具（含 TUI，项目脚手架）
  mofa-ffi        UniFFI + PyO3 绑定（Python、Java、Go、Kotlin、Swift）
  mofa-monitoring  仪表板、指标、分布式链路追踪
  mofa-extra      Rhai 引擎、规则引擎
  mofa-macros     过程宏
```

**mofa-sdk** 不是框架的一个层级——它是一个**面向开发者的工具包**，与框架并行存在，为你提供简洁、符合人体工学的 API。可以把它想象成一个工具箱：它从框架 crate（kernel、foundation、runtime、plugins）中取出你需要的东西交给你，这样你很少需要直接从各个 crate 中导入。

### 黄金法则

```
✅  Foundation → Kernel   （导入 trait，提供实现）
❌  Kernel → Foundation   （禁止！——会产生循环依赖）
```

内核对具体的 LLM 提供者、数据库或脚本引擎一无所知。它只定义实现必须填充的形状（traits）。这就是 MoFA 真正模块化的原因。

> **Rust 提示：什么是 trait？**
> Rust 中的 trait 类似于 Java 中的接口或 Swift 中的协议。它定义了一组类型必须实现的方法。例如，`MoFAAgent` trait 表示"任何自称为智能体的东西都必须有 `execute()`、`initialize()` 和 `shutdown()` 方法"。内核定义这些 trait；foundation 提供实现它们的具体结构体。

## 核心概念

以下是贯穿本教程的核心概念：

| 概念 | 定义 | 所在位置 |
|------|------|----------|
| **Agent（智能体）** | 接收输入、处理并产生输出的自治单元 | Trait 在 `mofa-kernel`，实现在 `mofa-foundation` |
| **Tool（工具）** | 智能体可以调用的函数（如网络搜索、计算器） | Trait 在 `mofa-kernel`，适配器在 `mofa-foundation` |
| **Memory（记忆）** | 智能体的键值存储 + 对话历史 | Trait 在 `mofa-kernel` |
| **Reasoner（推理器）** | 结构化推理（思考 → 决定 → 行动） | Trait 在 `mofa-kernel` |
| **Coordinator（协调器）** | 编排多个智能体协同工作 | Trait 在 `mofa-kernel`，`AgentTeam` 在 `mofa-foundation` |
| **Plugin（插件）** | 具有生命周期管理的可加载扩展 | Trait 在 `mofa-kernel`，Rhai/WASM 在 `mofa-plugins` |
| **Workflow（工作流）** | 处理状态的节点图（LangGraph 风格） | Trait 在 `mofa-kernel`，实现在 `mofa-foundation` |
| **LLM Provider（LLM 提供者）** | LLM API 的适配器（OpenAI、Ollama 等） | Trait 在 `mofa-kernel`，提供者在 `mofa-foundation` |

## 双层插件系统

MoFA 拥有独特的两层可扩展方案：

1. **编译时插件**（Rust / WASM）：最高性能，类型安全，适用于 LLM 推理适配器、数据处理流水线和原生集成。使用 Rust 编写（或编译为 WASM）。

2. **运行时插件**（Rhai 脚本）：最大灵活性，无需重新编译即可热重载，适用于业务规则、内容过滤器和工作流逻辑。使用 [Rhai](https://rhai.rs/)（一种轻量级嵌入式脚本语言）编写。

两层都实现相同的 `AgentPlugin` trait，因此系统对它们进行统一处理。你将在第 3 章构建编译时智能体，在第 8 章构建运行时 Rhai 插件。

## 你将构建的内容

以下是每章产出的内容地图：

```
第 3 章：GreetingAgent ─────── 理解 MoFAAgent trait
         │
第 4 章：LLM 聊天机器人 ────── 连接 OpenAI/Ollama，流式响应
         │
第 5 章：工具使用智能体 ────── 计算器 + 天气工具，ReAct 模式
         │
第 6 章：智能体团队 ────────── 链式和并行协调
         │
第 7 章：支持工作流 ────────── 带条件路由的 StateGraph
         │
第 8 章：Rhai 内容过滤器 ───── 支持热重载的脚本插件
```

每章都基于前一章的内容，但代码示例是独立的——如果你已经理解了前置条件，可以跳到任何章节。

## 关键要点

- MoFA 采用**微内核架构**：内核 = trait，foundation = 实现
- 依赖方向严格为 **foundation → kernel**，绝不反向
- **mofa-sdk** 是面向开发者的工具包（不是框架层级）；框架核心为 Runtime → Foundation → Kernel，Plugins 作为扩展系统
- **双层插件系统**同时提供性能（Rust/WASM）和灵活性（Rhai）
- 你将在第 3-8 章中逐步构建功能越来越强大的智能体

---

**下一章：** [第 2 章：环境搭建](02-setup_cn.md) — 准备好你的开发环境。

[← 返回目录](README_cn.md)

---

**English** | [简体中文](../zh-CN/tutorial/01-introduction_cn.md)
