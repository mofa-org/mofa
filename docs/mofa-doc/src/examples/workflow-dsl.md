# Workflow DSL

Examples demonstrating workflow definition using YAML-based DSL.

## Customer Support Workflow

Define complex agent workflows in YAML.

**Location:** `examples/workflow_dsl/`

```rust
use mofa_sdk::workflow::{
    WorkflowDslParser, WorkflowExecutor, ExecutorConfig, WorkflowValue,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse workflow from YAML file
    let definition = WorkflowDslParser::from_file("customer_support.yaml")?;
    println!("Loaded workflow: {} - {}", definition.metadata.id, definition.metadata.name);

    // Build workflow with agent registry
    let agent_registry = build_agents(&definition).await?;
    let workflow = WorkflowDslParser::build_with_agents(definition, &agent_registry).await?;
    println!("Built workflow with {} nodes", workflow.node_count());

    // Execute workflow
    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let input = WorkflowValue::String("I was charged twice for my subscription".to_string());

    let result = executor.execute(&workflow, input).await;
    println!("Result: {:?}", result);

    Ok(())
}
```

## Workflow YAML Definition

Example `customer_support.yaml`:

```yaml
metadata:
  id: customer-support-v1
  name: Customer Support Workflow
  version: "1.0.0"
  description: Handles customer inquiries with routing

agents:
  classifier:
    type: llm
    model: gpt-4o-mini
    system_prompt: |
      Classify the customer inquiry into categories:
      - billing
      - technical
      - general
    temperature: 0.3

  billing_agent:
    type: llm
    model: gpt-4o-mini
    system_prompt: You are a billing specialist. Help resolve billing issues.
    temperature: 0.5

  technical_agent:
    type: llm
    model: gpt-4o-mini
    system_prompt: You are a technical support agent. Help with technical issues.
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

## Parallel Agents Workflow

Execute multiple agents in parallel.

```rust
async fn run_parallel_agents() -> Result<()> {
    let definition = WorkflowDslParser::from_file("parallel_agents.yaml")?;
    let workflow = WorkflowDslParser::build_with_agents(definition, &agent_registry).await?;

    let executor = WorkflowExecutor::new(ExecutorConfig::default());
    let input = WorkflowValue::String(
        "Analyze the quarterly report for risks, opportunities, and sentiment.".to_string()
    );

    let result = executor.execute(&workflow, input).await;
    Ok(())
}
```

Example `parallel_agents.yaml`:

```yaml
metadata:
  id: parallel-analysis
  name: Parallel Analysis Workflow

agents:
  risk_analyzer:
    type: llm
    model: gpt-4o-mini
    system_prompt: Analyze for potential risks and concerns.

  opportunity_analyzer:
    type: llm
    model: gpt-4o-mini
    system_prompt: Identify opportunities and growth potential.

  sentiment_analyzer:
    type: llm
    model: gpt-4o-mini
    system_prompt: Analyze overall sentiment and tone.

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

## Building Agents from Definition

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

## Running Examples

```bash
# Set API key
export OPENAI_API_KEY=sk-xxx

# Run customer support workflow
cd examples/workflow_dsl
cargo run

# Or from repo root
cargo run -p workflow_dsl
```

## Available Examples

| Example | Description |
|---------|-------------|
| `workflow_dsl` | YAML-based workflow definitions |

## See Also

- [Workflows Concept](../concepts/workflows.md) — Workflow architecture
- [Workflow Orchestration](multi-agent-coordination.md) — Programmatic workflow building
