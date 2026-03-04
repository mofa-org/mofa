# Architecture Overview

MoFA (Model-based Framework for Agents) is a production-grade AI agent framework built with a **microkernel + dual-layer plugin system** architecture.

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
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              Abstraction Layer (mofa-kernel/agent/)                      │
│  Core abstractions and extensions                                        │
│                                                                          │
│  Core Traits:                                                            │
│  - MoFAAgent: Core trait (id, name, capabilities, execute, etc.)         │
│  - AgentLifecycle: pause, resume, interrupt                              │
│  - AgentMessaging: handle_message, handle_event                          │
│  - AgentPluginSupport: Plugin management                                 │
│                                                                          │
│  Core Types:                                                             │
│  - AgentContext, AgentInput, AgentOutput, AgentState, etc.               │
└─────────────────────────────────────────────────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────────────┐
│              Plugin System (mofa-plugins)                                │
│  Dual-layer plugin architecture                                          │
│                                                                          │
│  Compile-time plugins: Rust/WASM (zero-cost abstraction)                 │
│  Runtime plugins: Rhai scripting (hot-reload support)                    │
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
│   │    └──→ Kernel Layer (mofa-kernel)
│   │
│   └──→ Kernel Layer (mofa-kernel)
│
└──→ Runtime Layer (mofa-runtime)
         ↓
      ├──→ Kernel Layer (mofa-kernel)
      │
      └──→ Plugin System (mofa-plugins)
               ↓
            Core Layer (mofa-kernel)
```

**Key Rule**: Upper layers depend on lower layers, lower layers do not depend on upper layers.

## Layer Responsibilities

| Layer | Responsibility | Examples |
|-------|---------------|----------|
| **User** | Implement business logic | Custom agents, workflows |
| **SDK** | Unified API entry point | Re-exports, bindings |
| **Foundation** | Business capabilities | LLM, persistence, patterns |
| **Runtime** | Execution environment | Lifecycle, events, plugins |
| **Kernel** | Core abstractions | Traits, types, interfaces |
| **Plugins** | Extension mechanisms | Rust/WASM, Rhai scripts |

## Design Decisions

### Why Microkernel Architecture?

1. **Extensibility**: Easily extend functionality through plugin system
2. **Flexibility**: Users can depend only on the layers they need
3. **Maintainability**: Clear layer boundaries make code easy to maintain
4. **Testability**: Each layer can be tested independently

### Why Doesn't SDK Only Depend on Foundation?

SDK as a unified API entry point needs to:

1. Expose Runtime's runtime management functionality
2. Expose Kernel's core abstractions
3. Expose Foundation's business functionality

Therefore, SDK acts as a **facade**, re-exporting functionality from all layers.

### Why Are Foundation and Runtime Peer Relationships?

- **Foundation** provides **business capabilities** (LLM, persistence, patterns, etc.)
- **Runtime** provides **execution environment** (lifecycle management, event routing, etc.)

Both have different responsibilities, don't depend on each other, and both depend on the core abstractions provided by Kernel.

## Architecture Rules

These rules ensure the architecture stays clean and maintainable:

### Rule 1: Trait Definition Location

- **Kernel Layer**: Define ALL core trait interfaces
- **Foundation Layer**: NEVER re-define the same trait from kernel
- **Foundation Layer**: CAN import traits from kernel and add extension methods

### Rule 2: Implementation Location

- **Foundation Layer**: Provide ALL concrete implementations
- **Kernel Layer**: NO concrete implementations (test code excepted)
- **Plugins Layer**: Provide optional advanced implementations

### Rule 3: Dependency Direction

```
Foundation → Kernel (ALLOWED)
Plugins → Kernel (ALLOWED)
Plugins → Foundation (ALLOWED)
Kernel → Foundation (FORBIDDEN!)
```

## See Also

- [Microkernel Design](microkernel.md) — Deep dive into the microkernel pattern
- [Agents](agents.md) — Understanding the MoFAAgent trait
- [Plugins](plugins.md) — The dual-layer plugin system
- [Workspace Structure](../appendix/configuration.md) — Project organization
