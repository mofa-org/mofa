# Advanced Patterns

Examples demonstrating advanced agent patterns and specialized use cases.

## Reflection Agent

Self-improving agent with generate → critique → refine loop.

**Location:** `examples/reflection_agent/`

```rust
use mofa_sdk::react::{ReflectionAgent, ReflectionConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let llm_agent = Arc::new(create_llm_agent()?);

    // Create reflection agent
    let agent = ReflectionAgent::builder()
        .with_generator(llm_agent.clone())
        .with_config(ReflectionConfig::default().with_max_rounds(3))
        .with_verbose(true)
        .build()?;

    let task = "Explain the concept of ownership in Rust.";
    let result = agent.run(task).await?;

    println!("Rounds: {}", result.rounds);
    println!("Duration: {}ms", result.duration_ms);
    println!("Final Answer:\n{}", result.final_answer);

    // Review improvement process
    for step in &result.steps {
        println!("[Round {}]", step.round + 1);
        println!("Draft: {}", step.draft);
        println!("Critique: {}", step.critique);
    }

    Ok(())
}
```

### Reflection Process

1. **Generate**: Create initial response
2. **Critique**: Analyze response quality
3. **Refine**: Improve based on critique
4. **Repeat**: Until satisfied or max rounds

### Configuration

```rust
let config = ReflectionConfig::default()
    .with_max_rounds(5)              // Maximum refinement rounds
    .with_quality_threshold(0.8)      // Stop if quality exceeds threshold
    .with_critique_prompt("...")      // Custom critique prompt
    .with_verbose(true);              // Log each step
```

## Human-in-the-Loop Secretary

Secretary agent with human decision points.

**Location:** `examples/hitl_secretary/`

```rust
use mofa_sdk::secretary::{
    SecretaryCore, DefaultSecretaryBuilder, DispatchStrategy,
    DefaultInput, DefaultOutput, SecretaryCommand, QueryType,
};

async fn run_secretary() -> Result<()> {
    // Create communication channels
    let (connection, input_tx, mut output_rx) =
        ChannelConnection::<DefaultInput, DefaultOutput>::new_pair(64);

    // Build secretary behavior
    let behavior = DefaultSecretaryBuilder::new()
        .with_name("Project Secretary")
        .with_llm(llm_provider)
        .with_dispatch_strategy(DispatchStrategy::CapabilityFirst)
        .with_auto_clarify(true)
        .with_auto_dispatch(false)  // Human approval required
        .build();

    // Start secretary engine
    let core = SecretaryCore::new(behavior);
    let (_handle, _join_handle) = core.start(connection).await;

    // Handle outputs in background
    tokio::spawn(async move {
        while let Some(output) = output_rx.recv().await {
            match output {
                DefaultOutput::DecisionRequired { decision } => {
                    // Present decision to human
                    println!("Decision needed: {}", decision.description);
                    for (i, opt) in decision.options.iter().enumerate() {
                        println!("  [{}] {}", i, opt.label);
                    }
                }
                DefaultOutput::TaskCompleted { todo_id, result } => {
                    println!("Task {} completed: {}", todo_id, result.summary);
                }
                // ... other outputs
            }
        }
    });

    // Send inputs
    input_tx.send(DefaultInput::Idea {
        content: "Build a REST API".to_string(),
        priority: Some(TodoPriority::High),
        metadata: None,
    }).await?;

    Ok(())
}
```

### 5-Phase Workflow

1. **Receive Ideas** → Record as TODOs
2. **Clarify Requirements** → Generate project documents
3. **Schedule Dispatch** → Assign to execution agents
4. **Monitor Feedback** → Push key decisions to humans
5. **Acceptance Report** → Update TODO status

### Secretary Commands

| Command | Description |
|---------|-------------|
| `idea:<content>` | Submit new idea |
| `clarify:<todo_id>` | Clarify requirements |
| `dispatch:<todo_id>` | Start task execution |
| `decide:<id>:<option>` | Make pending decision |
| `status` | Show statistics |
| `report` | Generate progress report |

## Agent with Plugins and Rhai

Combine compile-time plugins with runtime Rhai scripts.

**Location:** `examples/agent_with_plugins_and_rhai/`

```rust
use mofa_sdk::plugins::{RhaiPlugin, RustPlugin};

// Compile-time Rust plugin
let rust_plugin = LoggingPlugin::new("info");

// Runtime Rhai script
let rhai_plugin = RhaiPlugin::from_file("./scripts/transform.rhai").await?;

// Build agent with both
let agent = ReActAgent::builder()
    .with_llm(llm_client)
    .with_tools(vec![calculator, weather])
    .with_plugin(rust_plugin)
    .with_plugin(rhai_plugin)
    .build();

// Plugins execute in order:
// 1. Rust plugin (before_execute)
// 2. Agent execution
// 3. Rhai plugin (transform output)
// 4. Rust plugin (after_execute)
```

## CLI Production Smoke Test

Production readiness validation.

**Location:** `examples/cli_production_smoke/`

```rust
// Run comprehensive checks
// - Agent creation and execution
// - LLM connectivity
// - Plugin loading
// - Persistence layer
// - Message bus operation

#[tokio::main]
async fn main() -> Result<()> {
    println!("Running production smoke tests...\n");

    // Test 1: Agent lifecycle
    test_agent_lifecycle().await?;

    // Test 2: LLM connectivity
    test_llm_connection().await?;

    // Test 3: Plugin system
    test_plugin_loading().await?;

    // Test 4: Database persistence
    test_persistence().await?;

    // Test 5: Message bus
    test_message_bus().await?;

    println!("\nAll smoke tests passed!");
    Ok(())
}
```

## Configuration Example

Configuration management patterns.

**Location:** `examples/config/`

```rust
use mofa_sdk::config::{AgentConfig, LLMConfig, PersistenceConfig};

// Load from file
let config = AgentConfig::from_file("agent.toml")?;

// Or build programmatically
let config = AgentConfig::builder()
    .id("my-agent")
    .name("My Agent")
    .llm(LLMConfig {
        provider: "openai".into(),
        model: "gpt-4o-mini".into(),
        temperature: 0.7,
    })
    .persistence(PersistenceConfig {
        backend: "postgres".into(),
        url: env::var("DATABASE_URL")?,
    })
    .build()?;

// Create agent from config
let agent = LLMAgentBuilder::from_config(&config).build_async().await?;
```

## Running Examples

```bash
# Reflection agent
export OPENAI_API_KEY=sk-xxx
cargo run -p reflection_agent

# HITL Secretary
cargo run -p hitl_secretary

# Plugin combination
cargo run -p agent_with_plugins_and_rhai

# Smoke tests
cargo run -p cli_production_smoke

# Configuration
cargo run -p config
```

## Available Examples

| Example | Description |
|---------|-------------|
| `reflection_agent` | Self-improving agent pattern |
| `hitl_secretary` | Human-in-the-loop secretary |
| `agent_with_plugins_and_rhai` | Combined plugins + Rhai |
| `cli_production_smoke` | Production smoke tests |
| `config` | Configuration management |

## See Also

- [Secretary Agent Guide](../guides/secretary-agent.md) — Secretary pattern details
- [Plugins](plugins.md) — Plugin system overview
- [Configuration](../appendix/configuration.md) — Configuration reference
