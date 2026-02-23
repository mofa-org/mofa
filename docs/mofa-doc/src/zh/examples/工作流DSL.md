# 工作流 DSL

使用基于 YAML 的 DSL 定义工作流示例。

## 客户支持工作流

在 YAML 中定义复杂的智能体工作流。

**位置：** `examples/workflow_dsl/`

```rust
use mofa_sdk::workflow::{
    WorkflowDslParser, WorkflowExecutor, ExecutorConfig, WorkflowValue,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 从 YAML 文件解析工作流
    let definition = WorkflowDslParser::from_file("customer_support.yaml")?;
    println!("加载工作流: {} - {}", definition.metadata.id, definition.metadata.name);

    // 使用智能体注册表构建工作流
    let agent_registry = build_agents(&definition).await?;
    let workflow = WorkflowDslParser::build_with_agents(definition, &agent_registry).await?;
    println!("构建包含 {} 个节点的工作流", workflow.node_count());

    // 执行工作流
    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let input = WorkflowValue::String("我被重复扣费了".to_string());

    let result = executor.execute(&workflow, input).await;
    println!("结果: {:?}", result);

    Ok(())
}
```

## 工作流 YAML 定义

示例 `customer_support.yaml`：

```yaml
metadata:
  id: customer-support-v1
  name: 客户支持工作流
  version: "1.0.0"
  description: 处理客户咨询并路由

agents:
  classifier:
    type: llm
    model: gpt-4o-mini
    system_prompt: |
      将客户咨询分类为以下类别：
      - billing（账单）
      - technical（技术）
      - general（一般）
    temperature: 0.3

  billing_agent:
    type: llm
    model: gpt-4o-mini
    system_prompt: 你是账单专员。帮助解决账单问题。
    temperature: 0.5

  technical_agent:
    type: llm
    model: gpt-4o-mini
    system_prompt: 你是技术支持专员。帮助解决技术问题。
    temperature: 0.5

nodes:
  - id: classify
    agent: classifier
    next: route

  - id: route
    type: switch
    field: category
    cases:
      billing: handle_billing
      technical: handle_technical
      default: handle_general

  - id: handle_billing
    agent: billing_agent
    next: respond

  - id: handle_technical
    agent: technical_agent
    next: respond

  - id: handle_general
    agent: general_agent
    next: respond

  - id: respond
    type: output
```

## 并行智能体工作流

并行执行多个智能体。

```rust
async fn run_parallel_agents() -> Result<()> {
    let definition = WorkflowDslParser::from_file("parallel_agents.yaml")?;
    let workflow = WorkflowDslParser::build_with_agents(definition, &agent_registry).await?;

    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let input = WorkflowValue::String(
        "分析季度报告的风险、机会和情绪。".to_string()
    );

    let result = executor.execute(&workflow, input).await;
    Ok(())
}
```

示例 `parallel_agents.yaml`：

```yaml
metadata:
  id: parallel-analysis
  name: 并行分析工作流

agents:
  risk_analyzer:
    type: llm
    model: gpt-4o-mini
    system_prompt: 分析潜在风险和问题。

  opportunity_analyzer:
    type: llm
    model: gpt-4o-mini
    system_prompt: 识别机会和增长潜力。

  sentiment_analyzer:
    type: llm
    model: gpt-4o-mini
    system_prompt: 分析整体情绪和基调。

nodes:
  - id: fan_out
    type: parallel
    branches:
      - risk_analyzer
      - opportunity_analyzer
      - sentiment_analyzer
    next: aggregate

  - id: aggregate
    type: merge
    strategy: concatenate
    next: summarize

  - id: summarize
    agent: summarizer
    next: output
```

## 从定义构建智能体

```rust
async fn build_agents(
    definition: &WorkflowDefinition,
) -> Result<HashMap<String, Arc<LLMAgent>>> {
    let mut registry = HashMap::new();

    for (agent_id, config) in &definition.agents {
        let provider = Arc::new(openai_from_env()?);

        let agent = LLMAgentBuilder::new()
            .with_id(agent_id)
            .with_provider(provider)
            .with_model(&config.model)
            .with_system_prompt(config.system_prompt.as_deref().unwrap_or(""))
            .with_temperature(config.temperature.unwrap_or(0.7))
            .build_async()
            .await?;

        registry.insert(agent_id.clone(), Arc::new(agent));
    }

    Ok(registry)
}
```

## 运行示例

```bash
# 设置 API 密钥
export OPENAI_API_KEY=sk-xxx

# 运行客户支持工作流
cd examples/workflow_dsl
cargo run

# 或从仓库根目录
cargo run -p workflow_dsl
```

## 可用示例

| 示例 | 描述 |
|------|------|
| `workflow_dsl` | 基于 YAML 的工作流定义 |

## 相关链接

- [工作流概念](../concepts/workflows.md) — 工作流架构
- [工作流编排](多智能体协调.md) — 编程式工作流构建
