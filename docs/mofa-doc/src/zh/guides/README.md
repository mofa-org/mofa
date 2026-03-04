# 指南

常见任务和模式的实用指南。

## 概述

- **LLM 提供商** — 配置不同的 LLM 后端
- **工具开发** — 创建自定义工具
- **持久化** — 保存和恢复智能体状态
- **多智能体系统** — 协调多个智能体
- **秘书智能体** — 人在回路模式
- **技能系统** — 可组合的智能体能力
- **监控与可观测性** — 生产环境监控

## 常见模式

### 构建 ReAct 智能体

```rust
let agent = ReActAgent::builder()
    .with_llm(client)
    .with_tools(vec![tool1, tool2])
    .build();
```

### 多智能体协调

```rust
let coordinator = SequentialCoordinator::new()
    .add_agent(researcher)
    .add_agent(writer);
```

## 下一步

根据您的用例选择指南。
