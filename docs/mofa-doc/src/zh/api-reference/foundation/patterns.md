# 智能体模式

用于常见用例的内置智能体模式。

## 概述

MoFA 提供多种智能体模式:

| 模式 | 用例 |
|---------|----------|
| ReAct | 带工具的推理 + 行动 |
| Secretary | 人在回路协调 |
| Chain-of-Thought | 逐步推理 |
| Router | 路由到专业智能体 |

## ReAct 模式

迭代使用工具的推理和行动智能体。

```rust
use mofa_sdk::react::ReActAgent;

let agent = ReActAgent::builder()
    .with_llm(client)
    .with_tools(vec![
        Arc::new(SearchTool),
        Arc::new(CalculatorTool),
    ])
    .with_max_iterations(5)
    .build();

let output = agent.execute(input, &ctx).await?;
```

### 配置

```rust
pub struct ReActConfig {
    max_iterations: usize,
    tool_timeout: Duration,
    reasoning_template: String,
}
```

## Secretary 模式

人在回路协调智能体。

```rust
use mofa_sdk::secretary::SecretaryAgent;

let agent = SecretaryAgent::builder()
    .with_llm(client)
    .with_human_feedback(true)
    .with_delegation_targets(vec!["researcher", "writer"])
    .build();
```

### 阶段

1. 接收想法 → 记录待办事项
2. 明确需求 → 生成文档
3. 调度派发 → 调用智能体
4. 监控反馈 → 将决策推送给人类
5. 验收报告 → 更新状态

## Chain-of-Thought

不带工具的逐步推理。

```rust
use mofa_sdk::patterns::ChainOfThought;

let agent = ChainOfThought::builder()
    .with_llm(client)
    .with_steps(5)
    .build();
```

## Router 模式

将请求路由到专业智能体。

```rust
use mofa_sdk::patterns::Router;

let router = Router::builder()
    .with_classifier(classifier_agent)
    .with_route("technical", tech_agent)
    .with_route("billing", billing_agent)
    .with_default(general_agent)
    .build();

let output = router.execute(input, &ctx).await?;
```

## 自定义模式

实现您自己的模式:

```rust
use mofa_sdk::kernel::prelude::*;

struct MyPattern {
    agents: Vec<Box<dyn MoFAAgent>>,
}

#[async_trait]
impl MoFAAgent for MyPattern {
    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
        // 您的模式逻辑
    }
}
```

## 另见

- [秘书智能体指南](../../guides/secretary-agent.md) — 秘书详情
- [工作流](../../concepts/workflows.md) — 工作流编排
