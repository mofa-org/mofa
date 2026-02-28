# MoFA Architecture

## Overview

MoFA (Model-based Framework for Agents) is a production-grade AI agent framework built with a **microkernel + dual-layer plugin system** architecture. This document describes MoFA's layered architecture, responsibilities, and design principles.

## Microkernel Architecture Principles

MoFA strictly follows these microkernel design principles:

1. **Minimal Core**: The kernel provides only the most basic abstractions and capabilities
2. **Plugin-based Extension**: All non-core functionality is provided through plugin mechanisms
3. **Clear Layers**: Each layer has well-defined responsibility boundaries
4. **Unified Interfaces**: Components of the same type use unified abstract interfaces
5. **Correct Dependency Direction**: Upper layers depend on lower layers, not the reverse

## Layered Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        User Layer (User Code)                            │
│                                                                          │
│  User code: Build agents using high-level APIs directly                 │
│  - Users implement the MoFAAgent trait                                   │
│  - Use AgentBuilder to construct Agents                                  │
│  - Use Runtime to manage Agent lifecycle                                 │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                    SDK Layer (mofa-sdk)                                  │
│  Unified API entry point: Re-exports types from all layers,             │
│  provides cross-language bindings                                        │
│                                                                          │
│  Module organization:                                                    │
│  - kernel: Core abstraction layer (MoFAAgent, AgentContext, etc.)        │
│  - runtime: Runtime layer (AgentBuilder, SimpleRuntime, etc.)            │
│  - foundation: Business layer (llm, secretary, react, etc.)              │
│  - Top-level convenience exports: Direct imports for common types        │
│                                                                          │
│  Features:                                                               │
│  - Modular entry points (use mofa_sdk::kernel::*, runtime::*, etc.)      │
│  - Feature flags to control optional capabilities                        │
│  - Cross-language bindings (UniFFI, PyO3)                                │
│  - Modular namespaces                                                    │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                 Business Layer (mofa-foundation)                         │
│  Business functionality and concrete implementations                     │
│                                                                          │
│  Core modules:                                                           │
│  - llm: LLM integration (OpenAI provider)                                │
│  - secretary: Secretary Agent pattern                                    │
│  - react: ReAct pattern implementation                                   │
│  - workflow: Workflow orchestration                                      │
│  - coordination: Multi-agent coordination                                │
│  - collaboration: Adaptive collaboration protocols                       │
│  - persistence: Persistence layer                                        │
│  - prompt: Prompt engineering                                            │
│                                                                          │
│  Responsibilities:                                                       │
│  - Provide production-ready Agent implementations                        │
│  - Implement business logic and collaboration patterns                   │
│  - Integrate external services (LLM, databases, etc.)                    │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                Runtime Layer (mofa-runtime)                              │
│  Agent lifecycle and execution management                                │
│                                                                          │
│  Core components:                                                        │
│  - AgentBuilder: Builder pattern                                         │
│  - AgentRunner: Executor                                                 │
│  - SimpleRuntime: Multi-agent coordination (non-dora mode)               │
│  - AgentRuntime: Dora-rs integration (optional)                          │
│  - Message bus and event routing                                         │
│                                                                          │
│  Responsibilities:                                                       │
│  - Manage Agent lifecycle (init, start, stop, destroy)                   │
│  - Provide Agent execution environment                                   │
│  - Handle inter-agent communication                                      │
│  - Support plugin system                                                 │
│                                                                          │
│  Dependencies:                                                           │
│  - mofa-kernel: Core abstractions                                        │
│  - mofa-plugins: Plugin system                                           │
│  - (optional) mofa-monitoring: Monitoring capabilities                   │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              Abstraction Layer (mofa-kernel/agent/)                      │
│  Core abstractions and extensions                                        │
│                                                                          │
│  Core Traits:                                                            │
│  - MoFAAgent: Core trait (id, name, capabilities, execute, etc.)         │
│                                                                          │
│  Extension Traits (optional):                                            │
│  - AgentLifecycle: pause, resume, interrupt                              │
│  - AgentMessaging: handle_message, handle_event                          │
│  - AgentPluginSupport: Plugin management                                 │
│                                                                          │
│  Core Types:                                                             │
│  - AgentContext: Execution context                                       │
│  - AgentInput/AgentOutput: Input/Output                                  │
│  - AgentState: Agent state                                               │
│  - AgentCapabilities: Capability description                             │
│  - AgentMetadata: Metadata                                               │
│  - AgentError/AgentResult: Error handling                                │
│                                                                          │
│  Responsibilities:                                                       │
│  - Define unified Agent interface                                        │
│  - Provide core types and abstractions                                   │
│  - Support trait composition for feature extension                       │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              Core Layer (mofa-kernel)                                    │
│  Minimal core infrastructure - No business logic                         │
│                                                                          │
│  Core modules:                                                           │
│  - context: Context management                                           │
│  - plugin: Plugin system interface                                       │
│  - bus: Event bus                                                        │
│  - message: Message types                                                │
│  - core: Core types                                                      │
│  - logging: Logging system                                               │
│                                                                          │
│  Responsibilities:                                                       │
│  - Provide basic data structures                                         │
│  - Implement event bus and message passing                               │
│  - Define plugin interfaces                                              │
│  - No business logic                                                     │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              Plugin System (mofa-plugins)                                │
│  Dual-layer plugin architecture                                          │
│                                                                          │
│  Compile-time plugins:                                                   │
│  - Rust/WASM plugins                                                     │
│  - Zero-cost abstraction                                                 │
│  - Performance-critical paths                                            │
│                                                                          │
│  Runtime plugins:                                                        │
│  - Rhai scripting engine                                                 │
│  - Hot-reload support                                                    │
│  - Business logic extension                                              │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              Monitoring Layer (mofa-monitoring) [Optional]               │
│  Observability and metrics                                               │
│  - Web dashboard                                                         │
│  - Metrics collection                                                    │
│  - Distributed tracing                                                   │
└─────────────────────────────────────────────────────────────────────────┘
```

## Dependency Relationships

```
User Code
    ↓
SDK Layer (mofa-sdk)
    ↓
├──→ Business Layer (mofa-foundation)
│        ↓
│   ├──→ Runtime Layer (mofa-runtime)
│   │        ↓
│   │    └──→ Abstraction Layer (mofa-kernel/agent/)
│   │             ↓
│   │          └──→ Core Layer (mofa-kernel)
│   │
│   └──→ Abstraction Layer (mofa-kernel/agent/)
│          ↓
│       Core Layer (mofa-kernel)
│
└──→ Runtime Layer (mofa-runtime)
         ↓
      ├──→ Abstraction Layer (mofa-kernel/agent/)
      │        ↓
      │     Core Layer (mofa-kernel)
      │
      └──→ Plugin System (mofa-plugins)
               ↓
            Core Layer (mofa-kernel)
```

**Key Rule**: Upper layers depend on lower layers, lower layers do not depend on upper layers.

## Layer Responsibilities

### User Layer
- Implement Agent business logic
- Use APIs provided by the SDK

### SDK Layer
- Unified API entry point
- Re-export functionality from all layers
- Provide cross-language bindings
- Modular namespaces

### Business Layer
- LLM integration
- Agent pattern implementations (ReAct, Secretary, etc.)
- Workflow orchestration
- Collaboration protocols
- Persistence

### Runtime Layer
- Agent lifecycle management
- Execution environment
- Event routing
- Plugin support

### Abstraction Layer
- MoFAAgent core interface
- Extension traits
- Core type definitions

### Core Layer
- Basic data structures
- Event bus
- Message passing
- Plugin interfaces

### Plugin System
- Compile-time plugins (Rust/WASM)
- Runtime plugins (Rhai scripts)

### Monitoring Layer
- Observability
- Metrics collection
- Distributed tracing

## Progressive Disclosure Skills Mechanism

MoFA supports a skill system based on `SKILL.md` files with progressive disclosure strategy to control context length and cost.

- Layer 1: Inject only skill metadata summary (name, description, availability)
- Layer 2: On-demand loading of complete skill content (when task requires)
- Supports always skills and multi-directory search (workspace > builtin > system)

```rust
use mofa_sdk::skills::SkillsManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Scan skills directory
    let skills = SkillsManager::new("./skills")?;

    // Inject summary only (metadata)
    let summary = skills.build_skills_summary().await;

    // On-demand load skill content (SKILL.md)
    let requested = vec!["pdf_processing".to_string()];
    let content = skills.load_skills_for_context(&requested).await;

    let system_prompt = format!(
        "You are a helpful assistant.\n\n# Skills Summary\n{}\n\n# Requested Skills\n{}",
        summary, content
    );
    println!("{}", system_prompt);
    Ok(())
}
```

## Usage Examples

### Custom Agent (with Skills and Runtime)

```rust
use mofa_sdk::kernel::{
    AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentError, AgentInput, AgentOutput,
    AgentResult, AgentState, MoFAAgent,
};
use mofa_sdk::runtime::AgentRunner;
use mofa_sdk::llm::{LLMClient, openai_from_env};
use mofa_sdk::skills::SkillsManager;
use async_trait::async_trait;
use std::sync::Arc;

struct MyAgent {
    caps: AgentCapabilities,
    state: AgentState,
    llm: LLMClient,
    skills: SkillsManager,
}

impl MyAgent {
    fn new(llm: LLMClient, skills: SkillsManager) -> Self {
        Self {
            caps: AgentCapabilitiesBuilder::new().tag("llm").tag("skills").build(),
            state: AgentState::Created,
            llm,
            skills,
        }
    }
}

#[async_trait]
impl MoFAAgent for MyAgent {
    fn id(&self) -> &str { "my-agent" }
    fn name(&self) -> &str { "My Agent" }
    fn capabilities(&self) -> &AgentCapabilities { &self.caps }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
        let user_input = input.to_text();
        let requested: Option<Vec<String>> = ctx.get("skill_names").await;

        let summary = self.skills.build_skills_summary().await;
        let mut system_prompt = format!("You are a helpful assistant.\n\n{}", summary);

        if let Some(names) = requested.as_ref() {
            let details = self.skills.load_skills_for_context(names).await;
            if !details.is_empty() {
                system_prompt = format!("{}\n\n# Requested Skills\n\n{}", system_prompt, details);
            }
        }

        let response = self.llm
            .chat()
            .system(system_prompt)
            .user(user_input)
            .send()
            .await
            .map_err(|e| AgentError::ExecutionFailed(e.to_string()))?;

        Ok(AgentOutput::text(response.content().unwrap_or_default()))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = openai_from_env()?;
    let llm = LLMClient::new(Arc::new(provider));
    let skills = SkillsManager::new("./skills")?;
    let agent = MyAgent::new(llm, skills);

    let ctx = AgentContext::with_session("exec-001", "session-001");
    ctx.set("skill_names", vec!["pdf_processing".to_string()]).await;

    let mut runner = AgentRunner::with_context(agent, ctx).await?;
    let output = runner.execute(AgentInput::text("Extract key fields from this PDF")).await?;
    runner.shutdown().await?;
    println!("{}", output.to_text());
    Ok(())
}
```

### Batch Execution

```rust
use mofa_sdk::kernel::{AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentInput, AgentOutput, AgentResult, AgentState, MoFAAgent};
use mofa_sdk::runtime::run_agents;
use async_trait::async_trait;

struct EchoAgent {
    caps: AgentCapabilities,
    state: AgentState,
}

impl EchoAgent {
    fn new() -> Self {
        Self {
            caps: AgentCapabilitiesBuilder::new().tag("echo").build(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for EchoAgent {
    fn id(&self) -> &str { "echo-agent" }
    fn name(&self) -> &str { "Echo Agent" }
    fn capabilities(&self) -> &AgentCapabilities { &self.caps }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        Ok(AgentOutput::text(format!("Echo: {}", input.to_text())))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inputs = vec![
        AgentInput::text("task-1"),
        AgentInput::text("task-2"),
    ];
    let outputs = run_agents(EchoAgent::new(), inputs).await?;
    for output in outputs {
        println!("{}", output.to_text());
    }
    Ok(())
}
```

### LLMAgentBuilder (Core Builder)

`LLMAgentBuilder` is located in the foundation layer and is responsible for assembling LLM provider, prompts, sessions, plugins, and persistence capabilities into an `LLMAgent`. `LLMAgent` implements `MoFAAgent`, so it can be run directly by the runtime execution engine or `AgentRunner`.

#### End-to-End: From Build to Run (Best Practice)

```rust
use mofa_sdk::kernel::AgentContext;
use mofa_sdk::runtime::AgentRunner;
use mofa_sdk::llm::{LLMAgentBuilder, HotReloadableRhaiPromptPlugin};
use mofa_sdk::persistence::{PersistencePlugin, PostgresStore};
use mofa_sdk::kernel::AgentInput;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Persistence plugin (optional, but recommended for production)
    let store = Arc::new(PostgresStore::connect("postgres://localhost/mofa").await?);
    let user_id = Uuid::now_v7();
    let tenant_id = Uuid::now_v7();
    let agent_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    let persistence = PersistencePlugin::new(
        "persistence-plugin",
        store,
        user_id,
        tenant_id,
        agent_id,
        session_id,
    );

    // 2) Prompt template (hot-reloadable)
    let prompt = HotReloadableRhaiPromptPlugin::new("./prompts/template.rhai").await;

    // 3) Build LLM Agent (config + session + plugins)
    let mut agent = LLMAgentBuilder::from_env()?
        .with_id("support-agent")
        .with_name("Support Agent")
        .with_system_prompt("You are a helpful assistant.")
        .with_sliding_window(10)
        .with_session_id(session_id.to_string())
        .with_hot_reload_prompt_plugin(prompt)
        .with_persistence_plugin(persistence)
        .build_async()
        .await;

    // 4) Session management (can create/switch before running)
    let session_id = agent.create_session().await;
    agent.switch_session(&session_id).await?;

    // 5) Runtime context (execution metadata)
    let ctx = AgentContext::with_session("exec-001", session_id.clone());
    ctx.set("user_id", user_id.to_string()).await;

    // 6) Run via AgentRunner (MoFAAgent lifecycle)
    let mut runner = AgentRunner::with_context(agent, ctx).await?;
    let output = runner.execute(AgentInput::text("Hello")).await?;
    println!("{}", output.to_text());
    Ok(())
}
```

### Using LLM

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = openai_from_env()?;
    let client = LLMClient::new(std::sync::Arc::new(provider));
    let response = client.ask("What is Rust?").await?;
    println!("{}", response);
    Ok(())
}
```

### Multi-Agent Coordination

```rust
use mofa_sdk::runtime::{SimpleRuntime, AgentBuilder};
use mofa_sdk::kernel::MoFAAgent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = SimpleRuntime::new();

    // Register multiple agents
    let agent1 = MyAgent1::new();
    let agent2 = MyAgent2::new();

    runtime.register_agent(agent1.metadata(), agent1.config(), "worker").await?;
    runtime.register_agent(agent2.metadata(), agent2.config(), "worker").await?;

    // Start runtime
    runtime.start().await?;

    Ok(())
}
```

## Design Decisions

### Why Microkernel Architecture?

1. **Extensibility**: Easily extend functionality through plugin system
2. **Flexibility**: Users can depend only on the layers they need
3. **Maintainability**: Clear layer boundaries make code easy to maintain
4. **Testability**: Each layer can be tested independently

### Why Doesn't SDK Only Depend on Foundation?

While microkernel architecture emphasizes layering, SDK as a unified API entry point needs to:

1. Expose Runtime's runtime management functionality
2. Expose Kernel's core abstractions
3. Expose Foundation's business functionality

Therefore, SDK acts as a **facade**, re-exporting functionality from all layers rather than depending layer by layer.

### Why Are Foundation and Runtime Peer Relationships?

- Foundation provides **business capabilities** (LLM, persistence, patterns, etc.)
- Runtime provides **execution environment** (lifecycle management, event routing, etc.)

Both have different responsibilities, don't depend on each other, and both depend on the core abstractions provided by Kernel.

## Future Improvements

1. **Stricter Dependency Checking**: Use tools like `cargo deny` to prevent incorrect dependency directions
2. **Finer-grained Feature Flags**: Reduce compilation time
3. **More Complete Documentation**: Detailed documentation and examples for each module
4. **Performance Optimization**: Optimize performance of critical paths
5. **Better Error Handling**: Unified error handling mechanism

## References

- [Agent Refactoring Proposal](./specs/agent_refactoring_proposal.md)
- [Secretary Agent Usage Guide](./specs/secretary_agent_usage.md)
- [Adaptive Collaboration Protocol](./specs/adaptive_collaboration.md)

---

**English** | [简体中文](zh-CN/architecture.md)
