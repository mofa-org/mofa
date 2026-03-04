# Chapter 7: Workflows with StateGraph

> **Learning objectives:** Understand graph-based workflows, implement nodes with `NodeFunc`, define edges and conditional routing, use reducers for state management, and build a customer support workflow.

## Why Workflows?

Multi-agent coordination (Chapter 6) handles task delegation. But what about complex processes with **branching logic**, **loops**, and **shared state**? That's where workflows come in.

MoFA's workflow system is inspired by [LangGraph](https://github.com/langchain-ai/langgraph). It models processes as **directed graphs** where:

- **Nodes** are processing steps (functions that transform state)
- **Edges** define the flow between nodes (including conditional branches)
- **State** flows through the graph and accumulates results

```
           ┌──────────┐
START ───▶ │ Classify  │
           └────┬──────┘
                │
        ┌───────┼───────┐
        ▼       ▼       ▼
    ┌───────┐ ┌───────┐ ┌──────┐
    │Billing│ │ Tech  │ │General│
    └───┬───┘ └───┬───┘ └──┬───┘
        │         │        │
        └─────────┼────────┘
                  ▼
           ┌──────────┐
           │ Respond   │
           └────┬──────┘
                ▼
               END
```

## Core Concepts

### GraphState

Every workflow operates on a **state** object. The `GraphState` trait defines how state is created, merged, and serialized:

```rust
// crates/mofa-kernel/src/workflow/graph.rs

pub trait GraphState: Clone + Send + Sync + 'static {
    fn new() -> Self;
    fn merge(&mut self, other: &Self);
    fn to_value(&self) -> serde_json::Value;
    fn from_value(value: serde_json::Value) -> AgentResult<Self>;
}
```

MoFA provides `JsonState` as a ready-to-use implementation:

```rust
use mofa_sdk::workflow::JsonState;

let mut state = JsonState::new();
state.set("customer_query", json!("I can't log in to my account"));
state.set("category", json!("unknown"));
```

### NodeFunc

Each node in the graph is a function that processes state:

```rust
#[async_trait]
pub trait NodeFunc<S: GraphState>: Send + Sync {
    async fn call(&self, state: &mut S, ctx: &RuntimeContext) -> AgentResult<Command>;
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str> { None }
}
```

A node receives mutable state, does its work, and returns a `Command` that controls flow.

### Command

The `Command` enum tells the graph what to do after a node runs:

```rust
pub enum Command {
    // Continue to the next node (follow the default edge)
    Continue(StateUpdate),

    // Jump to a specific node by name
    Goto(String, StateUpdate),

    // Stop the workflow and return current state
    Return(StateUpdate),
}
```

`StateUpdate` carries the changes this node wants to make to the state.

### Reducers

When multiple nodes update the same state key, **reducers** define how to merge the values:

| Reducer | Behavior | Example |
|---------|----------|---------|
| `AppendReducer` | Adds to a list | Messages accumulate |
| `OverwriteReducer` | Replaces the value | Status field updates |
| `MergeReducer` | Deep-merges JSON objects | Config accumulates |

## Build: Customer Support Workflow

Let's build a workflow that:
1. **Classifies** a customer query (billing, technical, general)
2. **Routes** to a specialized handler
3. **Responds** with a formatted answer

Create a new project:

```bash
cargo new support_workflow
cd support_workflow
```

Edit `Cargo.toml`:

```toml
[package]
name = "support_workflow"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

Write `src/main.rs`:

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{AgentResult, AgentContext};
use mofa_sdk::workflow::{
    JsonState, StateGraphImpl, Command, ControlFlow,
    RuntimeContext, NodeFunc, START, END,
};
use serde_json::json;

// --- Node 1: Classify the query ---

struct ClassifyNode;

#[async_trait]
impl NodeFunc<JsonState> for ClassifyNode {
    fn name(&self) -> &str { "classify" }

    fn description(&self) -> Option<&str> {
        Some("Classifies customer query into billing, technical, or general")
    }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("").to_lowercase();

        // Simple keyword-based classification
        // (In production, use an LLM for this)
        let category = if query.contains("bill") || query.contains("charge")
            || query.contains("payment") || query.contains("invoice")
        {
            "billing"
        } else if query.contains("error") || query.contains("bug")
            || query.contains("crash") || query.contains("login")
        {
            "technical"
        } else {
            "general"
        };

        state.set("category", json!(category));
        println!("  [Classify] Query classified as: {}", category);

        // Use Goto to route to the appropriate handler
        Ok(Command::Goto(
            category.to_string(),
            Default::default(),
        ))
    }
}

// --- Node 2a: Billing handler ---

struct BillingNode;

#[async_trait]
impl NodeFunc<JsonState> for BillingNode {
    fn name(&self) -> &str { "billing" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("");
        let response = format!(
            "Billing Support: I understand you have a billing concern about '{}'. \
             I've pulled up your account. Let me review the recent charges.",
            query
        );
        state.set("response", json!(response));
        state.set("department", json!("billing"));
        println!("  [Billing] Handled");
        Ok(Command::Continue(Default::default()))
    }
}

// --- Node 2b: Technical handler ---

struct TechnicalNode;

#[async_trait]
impl NodeFunc<JsonState> for TechnicalNode {
    fn name(&self) -> &str { "technical" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("");
        let response = format!(
            "Technical Support: I see you're experiencing a technical issue: '{}'. \
             Let me check the system status and recent logs.",
            query
        );
        state.set("response", json!(response));
        state.set("department", json!("technical"));
        println!("  [Technical] Handled");
        Ok(Command::Continue(Default::default()))
    }
}

// --- Node 2c: General handler ---

struct GeneralNode;

#[async_trait]
impl NodeFunc<JsonState> for GeneralNode {
    fn name(&self) -> &str { "general" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("");
        let response = format!(
            "General Support: Thank you for reaching out about '{}'. \
             I'm happy to help with any questions you have.",
            query
        );
        state.set("response", json!(response));
        state.set("department", json!("general"));
        println!("  [General] Handled");
        Ok(Command::Continue(Default::default()))
    }
}

// --- Node 3: Format final response ---

struct RespondNode;

#[async_trait]
impl NodeFunc<JsonState> for RespondNode {
    fn name(&self) -> &str { "respond" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let response = state.get_str("response").unwrap_or("No response generated");
        let department = state.get_str("department").unwrap_or("unknown");

        let final_response = format!(
            "--- Customer Support Response ---\n\
             Department: {}\n\
             {}\n\
             --- End ---",
            department, response
        );

        state.set("final_response", json!(final_response));
        println!("  [Respond] Final response formatted");
        Ok(Command::Return(Default::default()))
    }
}

// --- Build and run the workflow ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the state graph
    let mut graph = StateGraphImpl::<JsonState>::new("customer_support");

    // Add nodes
    graph.add_node(Box::new(ClassifyNode));
    graph.add_node(Box::new(BillingNode));
    graph.add_node(Box::new(TechnicalNode));
    graph.add_node(Box::new(GeneralNode));
    graph.add_node(Box::new(RespondNode));

    // Define edges
    // START → classify
    graph.add_edge(START, "classify");

    // classify → (dynamic routing via Goto in the node)
    // But we still need edges from handlers to respond:
    graph.add_edge("billing", "respond");
    graph.add_edge("technical", "respond");
    graph.add_edge("general", "respond");

    // respond → END (handled by Command::Return)

    // Compile the graph
    let compiled = graph.compile()?;

    // Test with different queries
    let test_queries = vec![
        "I was charged twice for my subscription",
        "I can't login to my account, getting error 500",
        "What are your business hours?",
    ];

    for query in test_queries {
        println!("\n=== Query: '{}' ===", query);
        let mut state = JsonState::new();
        state.set("query", json!(query));

        let result = compiled.run(state).await?;
        println!("{}", result.get_str("final_response").unwrap_or("No response"));
    }

    Ok(())
}
```

Run it:

```bash
cargo run
```

Expected output:

```
=== Query: 'I was charged twice for my subscription' ===
  [Classify] Query classified as: billing
  [Billing] Handled
  [Respond] Final response formatted
--- Customer Support Response ---
Department: billing
Billing Support: I understand you have a billing concern about '...'
--- End ---

=== Query: 'I can't login to my account, getting error 500' ===
  [Classify] Query classified as: technical
  ...
```

## What Just Happened?

1. **Graph construction**: We created nodes and connected them with edges
2. **Compilation**: `graph.compile()` validates the graph (checks for missing edges, unreachable nodes)
3. **Execution**: For each query:
   - State starts at `START`, flows to `classify`
   - `ClassifyNode` uses `Command::Goto(category)` to route to the right handler
   - The handler processes the query and uses `Command::Continue` to flow to `respond`
   - `RespondNode` formats the output and uses `Command::Return` to stop

> **Architecture note:** The `StateGraph` trait is defined in `mofa-kernel` (`crates/mofa-kernel/src/workflow/graph.rs`), while `StateGraphImpl` lives in `mofa-foundation` (`crates/mofa-foundation/src/workflow/state_graph.rs`). Reducers are in `crates/mofa-foundation/src/workflow/reducers.rs`. The workflow DSL parser (`WorkflowDslParser`) supports defining workflows in YAML — see `examples/workflow_dsl/src/main.rs` for a complete example.

## Workflow DSL (YAML)

For complex workflows, you can define them in YAML instead of code:

```yaml
# customer_support.yaml
workflow:
  name: "customer_support"
  nodes:
    - name: classify
      type: llm
      prompt: "Classify this customer query: {{query}}"
    - name: billing
      type: llm
      prompt: "Handle this billing question: {{query}}"
    - name: technical
      type: llm
      prompt: "Handle this technical issue: {{query}}"
    - name: respond
      type: llm
      prompt: "Format a final response for the customer"
  edges:
    - from: START
      to: classify
    - from: classify
      to: [billing, technical]
      condition: "category"
    - from: billing
      to: respond
    - from: technical
      to: respond
```

Load and run it:

```rust
use mofa_sdk::workflow::{WorkflowDslParser, WorkflowExecutor, ExecutorConfig};

let definition = WorkflowDslParser::from_file("customer_support.yaml")?;
let workflow = WorkflowDslParser::build(definition).await?;

let executor = WorkflowExecutor::new(ExecutorConfig::default());
let result = executor.execute(&workflow, input).await?;
```

## Key Takeaways

- Workflows model processes as directed graphs with nodes, edges, and shared state
- `NodeFunc` defines what each node does — receives state, returns a `Command`
- `Command::Continue` follows the default edge, `Goto` jumps to a named node, `Return` stops
- Conditional routing lets nodes decide the next step dynamically
- Reducers (`Append`, `Overwrite`, `Merge`) handle concurrent state updates
- `StateGraphImpl` is the concrete implementation, `JsonState` is the default state type
- YAML DSL is available for defining workflows declaratively

---

**Next:** [Chapter 8: Plugins and Scripting](08-plugins-and-scripting.md) — Write a hot-reloadable Rhai plugin.

[← Back to Table of Contents](README.md)

---

**English** | [简体中文](../zh/tutorial/07-workflows.md)
