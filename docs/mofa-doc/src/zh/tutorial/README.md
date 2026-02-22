# MoFA 教程：从零到智能体开发者

> **声明：** 本教程主要由 [Claude Code](https://claude.ai/code) 生成，待 MoFA 架构师 [@lijingrs](https://github.com/lijingrs) 审阅。内容可能会随着审阅进度进行更新。

欢迎来到 MoFA（Modular Framework for Agents）教程！本指南专为 **Google Summer of Code** 学生及所有希望使用 Rust 和 MoFA 微内核架构构建 AI 智能体的开发者设计。

## 你将学到什么

完成本教程后，你将理解 MoFA 的架构，并能够自信地构建、扩展和编排 AI 智能体。

| 章节 | 标题 | 时间 | 你将构建的内容 |
|------|------|------|----------------|
| [01](01-introduction.md) | 介绍 | ~20 分钟 | MoFA 架构的心智模型 |
| [02](02-setup.md) | 环境搭建 | ~15 分钟 | 可用的开发环境 |
| [03](03-first-agent.md) | 你的第一个智能体 | ~45 分钟 | 从零实现 `GreetingAgent` |
| [04](04-llm-agent.md) | LLM 驱动的智能体 | ~45 分钟 | 支持流式输出和记忆的聊天机器人 |
| [05](05-tools.md) | 工具与函数调用 | ~60 分钟 | 带计算器和天气工具的智能体 |
| [06](06-multi-agent.md) | 多智能体协调 | ~45 分钟 | 链式和并行智能体流水线 |
| [07](07-workflows.md) | StateGraph 工作流 | ~60 分钟 | 客户支持工作流 |
| [08](08-plugins-and-scripting.md) | 插件与脚本 | ~45 分钟 | 支持热重载的 Rhai 内容过滤器 |
| [09](09-whats-next.md) | 下一步 | ~15 分钟 | 你的贡献路线图 |

**预计总时间：4-6 小时**

## 前置条件

- **Rust**（1.85+）：通过 [rustup](https://rustup.rs/) 安装
- **LLM 提供者**（任选其一）：
  - OpenAI API 密钥（`OPENAI_API_KEY`），或
  - [Ollama](https://ollama.ai/) 本地运行（免费，无需 API 密钥）
- **Git**：用于克隆仓库
- 基本的终端操作能力

> **Rust 新手？** 别担心。每章都包含"Rust 提示"侧边栏，在涉及到相关概念（traits、async/await、`Arc`）时会进行解释。你不需要是 Rust 专家也能跟上进度。

## 快速链接

- [安装指南](../getting-started/installation.md) - 10 分钟内开始运行
- [架构参考](../concepts/architecture.md) - 深入的架构文档
- [贡献指南](https://github.com/mofa-org/mofa/blob/main/CONTRIBUTING.md) - 如何为 MoFA 做贡献
- [安全指南（English）](../../advanced/security.md) - 安全最佳实践
- [SDK 文档](../../crates/mofa-sdk.md) - SDK API 参考

## 如何使用本教程

1. **按顺序阅读各章节** — 每章都基于前一章的内容
2. **自己输入代码** — 不要只是复制粘贴（这样你会学到更多）
3. **运行每个示例** — 看到输出能建立直觉
4. **阅读"架构说明"标注** — 它们将代码与设计决策联系起来
5. **查看链接的源文件** — 真实代码是最好的文档

准备好了吗？让我们从[第 1 章：介绍](01-introduction.md)开始。

---

[English](../../tutorial/README.md) | **简体中文**
