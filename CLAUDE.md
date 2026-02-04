# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MoFA (Modular Framework for Agents) is a production-grade AI agent framework built in Rust, designed for extreme performance, unlimited extensibility, and runtime programmability. It implements a **microkernel + dual-layer plugin system** architecture.

**Key Features:**
- Rust core with UniFFI for multi-language bindings (Python, Java, Swift, Kotlin, Go)
- Dual-layer plugins: compile-time (Rust/WASM) for performance + runtime (Rhai scripts) for flexibility
- Multi-agent coordination patterns (chain, parallel, debate, supervision, MapReduce, routing, aggregation)
- Secretary agent mode for human-in-the-loop workflows
- Distributed dataflow support via Dora-rs (optional)
- Actor-based concurrency using Ractor

## Common Commands

```bash
# Build the entire workspace
cargo build
cargo build --release

# Build specific crate
cargo build -p mofa-sdk
cargo build -p mofa-cli

# Run tests
cargo test
cargo test -p mofa-sdk

# Run specific test
cargo test -p mofa-sdk -- test_name

# Run CLI tool
cargo run -p mofa-cli -- mofa --help

# Run examples
cargo run -p mofa-cli -- mofa new my_agent
cd examples/react_agent && cargo run
cd examples/secretary_agent && cargo run

# Check code (fast compilation check)
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Workspace Structure

```
mofa/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── mofa-kernel/        # Microkernel core (lifecycle, metadata, communication)
│   ├── mofa-foundation/    # Foundation layer (LLM, agents, persistence)
│   ├── mofa-runtime/       # Runtime system (message bus, registry, event loop)
│   ├── mofa-plugins/       # Plugin system (dual-layer architecture)
│   ├── mofa-cli/           # CLI tool (`mofa` command)
│   ├── mofa-sdk/           # Main SDK - unified API surface
│   ├── mofa-macros/        # Procedural macros
│   ├── mofa-monitoring/    # Monitoring and observability
│   └── mofa-extra/         # Additional utilities
├── examples/               # Usage examples (17+ examples)
└── docs/                   # Documentation
```

## Architecture Overview

### Microkernel + Dual-Layer Plugin System

MoFA uses a layered microkernel architecture:

1. **Microkernel (`mofa-kernel`)**: Lightweight core with lifecycle management, metadata system, communication bus, and task scheduling
2. **Compile-time Plugin Layer**: Rust/WASM plugins for performance-critical paths (LLM inference, data processing, native system integration)
3. **Runtime Plugin Layer**: Rhai scripts for dynamic business logic, hot-reloadable rules, workflow orchestration
4. **Business Layer**: User-defined agents, workflows, and rules

### Key Crates

- **`mofa-sdk`**: Main entry point - high-level unified API, multi-language bindings, secretary agent mode
- **`mofa-runtime`**: Message bus, agent registry, event loop, health checks, state management
- **`mofa-foundation`**: LLM integration (OpenAI provider), agent abstractions, persistence layer (PostgreSQL/MySQL/SQLite)
- **`mofa-plugins`**: Dual-layer plugin system with Rhai scripting engine integration
- **`mofa-kernel`**: Core runtime with metadata, lifecycle, and communication primitives

### Multi-Agent Coordination Patterns

The framework supports 7 coordination modes:
- **Chain**: Sequential execution (output of one agent becomes input of next)
- **Parallel**: Simultaneous execution with result aggregation
- **Debate**: Multi-agent alternating discussion for quality improvement
- **Supervision**: Supervisor agent evaluates and filters results
- **MapReduce**: Parallel processing with result reduction
- **Routing**: Dynamic agent selection based on conditions
- **Aggregation**: Collects and combines results from multiple agents

### Secretary Agent Pattern

Human-in-the-loop workflow management with 5 phases:
1. **Receive ideas** → Record todos
2. **Clarify requirements** → Project documents
3. **Schedule dispatch** → Call execution agents
4. **Monitor feedback** → Push key decisions to humans
5. **Acceptance report** → Update todos

## Development Notes

### Feature Flags

The workspace uses feature flags extensively:
- `uniffi`: Cross-language bindings (Python, Java, Swift, Kotlin, Go)
- `openai`: OpenAI provider support
- `dora`: Dora-rs distributed runtime support
- `persistence-*`: Database backend selection (postgres, mysql, sqlite)
- `python`: Native Python bindings via PyO3

### Dependencies

Key dependencies:
- `tokio`: Async runtime
- `ractor`: Actor framework for ReAct agents
- `rhai`: Embedded scripting engine for runtime plugins
- `uniffi`: Multi-language bindings generator
- `sqlx`: Database access
- `opentelemetry`: Distributed tracing
- `serde`/`serde_json`: Serialization

### Plugin Development

- Compile-time plugins use Rust traits for zero-cost abstractions
- Runtime plugins use Rhai scripting with built-in JSON processing
- Both layers can interoperate seamlessly

### Testing

Run tests for specific crates:
- `cargo test -p mofa-sdk`: Test SDK functionality
- `cargo test -p mofa-runtime`: Test runtime systems
- `cargo test -p mofa-plugins`: Test plugin system

### Examples Directory

The `examples/` directory contains 17+ examples demonstrating various features:
- `react_agent/`: Basic ReAct pattern agent
- `secretary_agent/`: Secretary agent with human-in-the-loop
- `multi_agent_coordination/`: Various coordination patterns
- `rhai_scripting/`: Runtime scripting
- `workflow_orchestration/`: Workflow builder
- `wasm_plugin/`: WASM plugin development
- `monitoring_dashboard/`: Observability features

Review these when implementing new features to understand existing patterns.

---

## MoFA Microkernel Architecture Standards

### Architecture Layering

MoFA follows a strict microkernel architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                    mofa-sdk (Unified API)                   │
│  - External unified interface                                 │
│  - Re-exports core types from kernel and foundation          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│              mofa-runtime (Execution Lifecycle)              │
│  - AgentRegistry, EventLoop, PluginManager                  │
│  - Dynamic loading and plugin management                      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│            mofa-foundation (Business Logic)                  │
│  - ✅ Concrete implementations (InMemoryStorage, SimpleToolRegistry) |
│  - ✅ Extended types (RichAgentContext, business-specific data)  |
│  - ❌ FORBIDDEN: Re-defining kernel traits                          │
│  - ✅ ALLOWED: Importing and extending kernel traits                   │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│              mofa-kernel (Microkernel Core)                  │
│  - ✅ Trait definitions (Tool, Memory, Reasoner, etc.)       │
│  - ✅ Core data types (AgentInput, AgentOutput, AgentState)  │
│  - ✅ Base abstractions (MoFAAgent, AgentPlugin)             │
│  - ❌ FORBIDDEN: Concrete implementations (except test code)   │
│  - ❌ FORBIDDEN: Business logic                                │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│            mofa-plugins (Plugin Layer)                       │
│  - Plugin adapters (ToolPluginAdapter)                       │
│  - Concrete plugin implementations                            │
└─────────────────────────────────────────────────────────────┘
```

### Core Rules

#### Rule 1: Trait Definition Location
- ✅ **Kernel Layer**: Define ALL core trait interfaces
- ❌ **Foundation Layer**: NEVER re-define the same trait from kernel
- ✅ **Foundation Layer**: CAN import traits from kernel and add extension methods

#### Rule 2: Implementation Location
- ✅ **Foundation Layer**: Provide ALL concrete implementations
- ❌ **Kernel Layer**: NO concrete implementations (test code excepted)
- ✅ **Plugins Layer**: Provide optional advanced implementations

#### Rule 3: Type Exports
- ✅ **Kernel**: Export only types it defines
- ✅ **Foundation**: Export only types it implements, NOT re-export kernel traits
- ✅ **SDK**: Unified re-export of user-facing APIs

#### Rule 4: Data Types
- ✅ **Kernel Layer**: Base data types (AgentInput, AgentOutput, AgentState, ToolInput, ToolResult)
- ✅ **Foundation Layer**: Business-specific data types (Session, PromptContext, ComponentOutput)
- ⚠️ **Boundary**: If a type is part of a trait definition, put it in kernel; if business-specific, put it in foundation

#### Rule 5: Dependency Direction
```
Foundation → Kernel (ALLOWED)
Plugins → Kernel (ALLOWED)
Plugins → Foundation (ALLOWED)
Kernel → Foundation (FORBIDDEN! Creates circular dependency)
```

### Code Checklist

#### Kernel Layer Checklist
- [ ] Is this a trait definition? ✅ Otherwise it shouldn't be here
- [ ] Is this a core data type? ✅ AgentInput/Output/State, ToolInput/Result, etc.
- [ ] Is this a base type? ✅ Interfaces, primitives
- [ ] Does it contain concrete implementations? ❌ Move to foundation

#### Foundation Layer Checklist
- [ ] Does it re-define a kernel trait? ❌ Import from kernel instead
- [ ] Is this a concrete implementation? ✅ Correct location
- [ ] Does it depend on kernel types? ✅ Allowed: `use mofa_kernel::...`
- [ ] Is it depended on by kernel? ❌ Creates circular dependency

#### Type Export Checklist
- [ ] Does foundation re-export kernel traits? ❌ Remove duplicate exports
- [ ] Can users clearly tell which layer a type comes from? ✅ Should be clear
- [ ] Are there naming conflicts? ❌ Should be avoided

### Common Anti-Patterns

#### ❌ Anti-Pattern 1: Foundation Re-defining Kernel Trait

```rust
// crates/mofa-foundation/src/agent/components/tool.rs
// ❌ WRONG: Re-defining kernel trait in foundation
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    // ...
}
```

**Correct Approach**:
```rust
// ✅ CORRECT: Import from kernel
pub use mofa_kernel::agent::components::tool::Tool;

// If you need to extend, define a wrapper
pub struct FoundationTool {
    inner: Arc<dyn Tool>,
    extra_field: String,
}
```

#### ❌ Anti-Pattern 2: Kernel Containing Concrete Implementation

```rust
// crates/mofa-kernel/src/agent/components/tool.rs
// ❌ WRONG: Kernel should not contain concrete implementations
pub struct SimpleToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

#[async_trait]
impl ToolRegistry for SimpleToolRegistry {
    // Implementation...
}
```

**Correct Approach**:
```rust
// ✅ CORRECT: Only define trait in kernel
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    fn register(&mut self, tool: Arc<dyn Tool>) -> AgentResult<()>;
    // ...
}

// Concrete implementation goes in foundation
// crates/mofa-foundation/src/agent/components/tool_registry.rs
pub struct SimpleToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

#[async_trait]
impl ToolRegistry for SimpleToolRegistry {
    // Implementation...
}
```

#### ❌ Anti-Pattern 3: Duplicate Exports Causing Type Confusion

```rust
// crates/mofa-foundation/src/agent/mod.rs
// ❌ WRONG: Duplicate exports of kernel types
pub use mofa_kernel::agent::{Tool, ToolRegistry};
pub use components::tool::{Tool, ToolRegistry, SimpleToolRegistry};
```

**Correct Approach**:
```rust
// ✅ CORRECT: Foundation exports only what it implements
pub use components::tool_registry::{SimpleToolRegistry, EchoTool};
pub use components::tool::ToolCategory; // Foundation-specific extension

// Tool and ToolRegistry are exported by kernel, no need to re-export here
```

### Identifying Architecture Violations

When reviewing code, check for these warning signs:

1. **Same trait name in multiple crates**: Indicates duplicate definition
2. **Foundation `pub use` of kernel traits with custom modifications**: Likely should be extension not replacement
3. **Kernel `pub struct` with trait implementations**: Concrete code in wrong layer
4. **Circular dependency warnings in Cargo.toml**: Architecture violation

### Quick Reference

| What | Where | Example |
|-------|-------|---------|
| **Trait definitions** | `mofa-kernel` | `Tool`, `Memory`, `Reasoner`, `Coordinator` |
| **Core data types** | `mofa-kernel` | `AgentInput`, `AgentOutput`, `AgentState` |
| **Base abstractions** | `mofa-kernel` | `MoFAAgent`, `AgentPlugin` |
| **Concrete implementations** | `mofa-foundation` | `SimpleToolRegistry`, `InMemoryStorage` |
| **Business types** | `mofa-foundation` | `Session`, `PromptContext`, `RichAgentContext` |
| **Plugin implementations** | `mofa-plugins` | `ToolPluginAdapter`, `LLMPlugin` |
