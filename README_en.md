# MoFA Agent Framework

[English](README_en.md) | [简体中文](README.md)

<p align="center">
    <img src="docs/images/mofa-logo.png" width="30%"/>
</p>


<div align="center">
  <a href="https://crates.io/crates/mofa-sdk">
    <img src="https://img.shields.io/crates/v/mofa.svg" alt="crates.io"/>
  </a>
  <a href="https://pypi.org/project/mofa-core/">
    <img src="https://img.shields.io/pypi/v/mofa-core.svg" alt="PyPI"/>
  </a>
  <a href="https://github.com/mofa-org/mofa/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/mofa-org/mofa" alt="License"/>
  </a>
  <a href="https://docs.rs/mofa-sdk">
    <img src="https://img.shields.io/badge/built_with-Rust-dca282.svg?logo=rust" alt="docs"/>
  </a>
  <a href="https://github.com/mofa-org/mofa/stargazers">
    <img src="https://img.shields.io/github/stars/mofa-org/mofa" alt="GitHub Stars"/>
  </a>
</div>

<h2 align="center">
  <a href="https://mofa.ai/">Website</a>
  |
  <a href="https://mofa.ai/docs/0overview/">Quick Start</a>
  |
  <a href="https://github.com/mofa-org/mofa">GitHub</a>
  |
  <a href="https://hackathon.mofa.ai/">Hackathon</a>
  |
  <a href="https://discord.com/invite/hKJZzDMMm9">Community</a>
</h2>

<p align="center">
 <img src="https://img.shields.io/badge/Performance-Extreme-red?style=for-the-badge" />
 <img src="https://img.shields.io/badge/Extensibility-Unlimited-orange?style=for-the-badge" />
 <img src="https://img.shields.io/badge/Languages-Multi_platform-yellow?style=for-the-badge" />
 <img src="https://img.shields.io/badge/Runtime-Programmable-green?style=for-the-badge" />
</p>

## Overview
MoFA (Modular Framework for Agents) is not just another agent framework.
It is the first production-grade agent framework achieving **"write once, share across languages"**, focusing on **extreme performance, unlimited extensibility, and runtime programmability**.
Through revolutionary architectural design, it creates an innovative **dual-layer plugin system** (compile-time plugins + runtime plugins), achieving a rare perfect balance of "performance and flexibility" in the industry.

MoFA's Breakthroughs:</br>
✅ Rust Core + UniFFI: Extreme performance + native multi-language calls</br>
✅ Dual-layer plugins: Compile-time high performance + runtime zero-deployment modification</br>
✅ Microkernel architecture: Modular, easy to extend</br>
✅ Cloud-native: Native support for distributed and edge computing</br>

## Why Choose MoFA?
### **Performance Advantages**

- Built on Rust with zero-cost abstractions
- Memory safety guarantees
- Significant performance improvements over Python ecosystem frameworks

### **Multi-Language Support**

- Generate Python, Java, Go, Kotlin, Swift bindings via UniFFI
- Support calling Rust core logic from multiple languages
- Cross-language call performance superior to traditional FFI solutions

### **Runtime Programmability**

- Integrated Rhai scripting engine
- Support hot-reload business logic
- Support runtime configuration and rule adjustments
- User-defined extensions

### **Dual-Layer Plugin Architecture**

- **Compile-time plugins**: Extreme performance, native integration
- **Runtime plugins**: Dynamic loading, instant effect
- Support plugin hot loading and version management

### **Distributed Dataflow (Dora)**

- Support Dora-rs distributed runtime
- Cross-process/cross-machine agent communication
- Suitable for edge computing scenarios

### **Actor Concurrency Model (Ractor)**

- Good isolation between agents
- Message-driven architecture
- Support high-concurrency scenarios

## Core Architecture

### Microkernel + Dual-Layer Plugin System

MoFA adopts a **layered microkernel architecture**, achieving extreme extensibility through a **dual-layer plugin system**:

```
┌─────────────────────────────────────────────────────────┐
│                    Business Layer                        │
│  (User-defined Agents, Workflows, Rules)                 │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│          Runtime Plugin Layer (Rhai Scripts)              │
│  • Dynamic tool registration  • Rule engine  • Scripts   │
│  • Hot-load logic    • Expression evaluation             │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│            Compile-time Plugin Layer (Rust/WASM)         │
│  • LLM plugins  • Tool plugins  • Storage  • Protocol    │
│  • High-performance modules  • Native system integration  │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                  Microkernel (mofa-kernel)               │
│  • Lifecycle management  • Metadata  • Communication     │
│  • Task scheduling       • Memory management             │
└─────────────────────────────────────────────────────────┘
```

#### Advantages of Dual-Layer Plugin System

**Compile-time Plugins (Rust/WASM)**

- Extreme performance, zero runtime overhead
- Type safety, compile-time error checking
- Support complex system calls and native integration
- WASM sandbox provides secure isolation

**Runtime Plugins (Rhai Scripts)**

- No recompilation needed, instant effect
- Business logic hot updates
- User-defined extensions
- Secure sandbox execution with configurable resource limits

**Combined Power**

- Use Rust plugins for performance-critical paths (e.g., LLM inference, data processing)
- Use Rhai scripts for business logic (e.g., rule engines, workflow orchestration)
- Seamless interoperability between both, covering 99% of extension scenarios

## Core Features

### 1. Microkernel Architecture
MoFA adopts a **layered microkernel architecture** with `mofa-kernel` at its core. All other features (including plugin system, LLM capabilities, multi-agent collaboration, etc.) are built as modular components on top of the microkernel.

#### Core Design Principles
- **Core Simplicity**: The microkernel contains only the most basic functions: agent lifecycle management, metadata system, and dynamic management
- **High Extensibility**: All advanced features are extended through modular components and plugins, keeping the kernel stable
- **Loose Coupling**: Components communicate through standardized interfaces, easy to replace and upgrade

#### Integration with Plugin System
- The plugin system is developed based on the `Plugin` interface of the microkernel. All plugins (including LLM plugins, tool plugins, etc.) are integrated through the `AgentPlugin` standard interface
- The microkernel provides plugin registration center and lifecycle management, supporting plugin hot loading and version control
- LLM capabilities are implemented through `LLMPlugin`, encapsulating LLM providers as plugins compliant with microkernel specifications

#### Integration with LLM
- LLM exists as a plugin component of the microkernel, providing unified LLM access capabilities through the `LLMCapability` interface
- All agent collaboration patterns (chain, parallel, debate, etc.) are built on the microkernel's workflow engine and interact with LLMs through standardized LLM plugin interfaces
- Secretary mode is also implemented based on the microkernel's A2A communication protocol and task scheduling system

### 2. Dual-Layer Plugins
- **Compile-time plugins**: Extreme performance, native integration
- **Runtime plugins**: Dynamic loading, instant effect
- Seamless collaboration between both, covering all scenarios

### 3. Agent Coordination
- **Priority Scheduling**: Task scheduling system based on priority levels
- **Communication Bus**: Built-in inter-agent communication bus
- **Workflow Engine**: Visual workflow builder and executor

### 4. LLM and AI Capabilities
- **LLM Abstraction Layer**: Unified LLM integration interface
- **OpenAI Support**: Built-in OpenAI API integration
- **ReAct Pattern**: Agent framework based on reasoning and action
- **Multi-Agent Collaboration**: Team-based agent coordination, supporting multiple collaboration patterns:
  - **Chain Mode**: Multi-agent sequential workflow where output of one agent becomes input of the next, suitable for pipeline processing scenarios
  - **Parallel Mode**: Multiple agents execute simultaneously with automatic result aggregation, significantly improving processing efficiency
  - **Debate Mode**: Multiple agents alternate speaking, optimizing result quality through debate mechanism
  - **Supervision Mode**: A supervisor agent evaluates and filters results
  - **MapReduce Mode**: Parallel processing with result reduction, suitable for large-scale tasks
  - **Routing Mode**: Dynamically select the next agent to execute based on conditions
  - **Aggregation Mode**: Collect and merge results from multiple agents
- **Secretary Mode**: Provides end-to-end task closed-loop management, including 5 core phases: receive ideas → record todos, clarify requirements → convert to project documents, schedule dispatch → call execution agents, monitor feedback → push key decisions to humans, acceptance report → update todos
  </br>**Features**:
    - 🧠 Autonomous task planning and decomposition
    - 🔄 Intelligent agent scheduling and orchestration
    - 👤 Human intervention at key nodes
    - 📊 Full process observability and traceability
    - 🔁 Closed-loop feedback and continuous optimization

### 5. Persistence Layer
- **Multiple Backends**: Support PostgreSQL, MySQL, and SQLite
- **Session Management**: Persistent agent session storage
- **Memory System**: Stateful agent memory management

### 6. Monitoring & Observability
- **Dashboard**: Built-in web dashboard with real-time metrics
- **Metrics System**: Prometheus-compatible metrics system
- **Tracing Framework**: Distributed tracing system

### 7. Rhai Script Engine

MoFA integrates the [Rhai](https://github.com/rhaiscript/rhai) embedded scripting language, providing **runtime programmability** without recompilation.

#### Script Engine Core
- **Safe Sandbox Execution**: Configurable operation limits, call stack depth, loop control
- **Script Compilation Cache**: Pre-compile scripts for improved repeated execution performance
- **Rich Built-in Functions**: String manipulation, math functions, JSON processing, time utilities
- **Bidirectional JSON Conversion**: Seamless conversion between JSON and Rhai Dynamic types

#### Scripted Workflow Nodes
- **Script Task Nodes**: Execute business logic via scripts
- **Script Condition Nodes**: Dynamic branch decisions
- **Script Transform Nodes**: Data format transformation
- **YAML/JSON Workflow Loading**: Define workflows through configuration files

#### Dynamic Tool System
- **Script-based Tool Definition**: Register tools at runtime
- **Parameter Validation**: Type checking, range validation, enum constraints
- **Auto JSON Schema Generation**: Compatible with LLM Function Calling
- **Hot Loading**: Dynamically load tools from directories

#### Rule Engine
- **Priority Rules**: Critical > High > Normal > Low
- **Multiple Match Modes**: First match, all match, ordered match
- **Composite Actions**: Set variables, trigger events, goto rules
- **Rule Group Management**: Support default fallback actions

#### Typical Application Scenarios
| Scenario | Description |
|----------|-------------|
| **Dynamic Business Rules** | Discount strategies, content moderation rules, no redeployment needed |
| **Configurable Workflows** | User-defined data processing pipelines |
| **LLM Tool Extensions** | Register new tools at runtime for LLM calls |
| **A/B Testing** | Control experiment logic through scripts |
| **Expression Evaluation** | Dynamic condition checking, formula calculation |

## Roadmap

### Short-term Goals
- [ ] Dora-rs runtime support for distributed dataflow
- [ ] Complete distributed tracing implementation
- [ ] Python binding generation
- [ ] More LLM provider integrations

### Long-term Goals
- [ ] Visual workflow designer UI
- [ ] Cloud-native deployment support
- [ ] Advanced agent coordination algorithms
- [ ] Agent platform
- [ ] Cross-process/cross-machine distributed agent collaboration
- [ ] Multi-agent collaboration standard protocol
- [ ] Cross-platform mobile support
- [ ] Evolve into agent operating system

## Quick Start

### Installation

Add MoFA to your Cargo.toml:

```toml
[dependencies]
mofa-sdk = "0.1.0"
```

The runtime mode is most suitable for scenarios that require building complete agent workflows, specifically including:

  ---
1. Multi-agent collaboration scenarios

The runtime provides a message bus (SimpleMessageBus/DoraChannel) and agent registration system, supporting communication between agents:
- Point-to-point communication (send_to_agent)
- Broadcast messages (broadcast)
- Topic pub/sub (publish_to_topic/subscribe_topic)
- Role management (get_agents_by_role)

When you need multiple agents to collaborate on complex tasks (such as master-slave architecture, division of labor), the runtime's communication mechanism can significantly simplify development.

  ---
2. Event-driven agent applications

The runtime has a built-in event loop (run_with_receiver/run_event_loop) and interrupt handling system, automatically managing:
- Event reception and dispatch
- Agent state lifecycle
- Timeout and interrupt handling

Suitable for building applications that need to respond to external events or timers (such as real-time dialogue systems, event response robots).

  ---
3. Distributed agent systems

When the dora feature is enabled, the runtime provides Dora adapters (DoraAgentNode/DoraDataflow), supporting:
- Distributed node deployment
- Cross-node agent communication
- Data flow management

Suitable for production scenarios requiring large-scale deployment and low-latency communication.

  ---
4. Structured agent building

The runtime provides AgentBuilder fluent API, simplifying agent:
- Configuration management
- Plugin integration
- Capability declaration
- Port configuration

Suitable for scenarios where you need to quickly build standardized agents, especially when you need to uniformly manage multiple agent configurations.

  ---
5. Production-grade applications

The runtime provides comprehensive:
- Health checks and state management
- Logging and monitoring integration
- Error handling mechanisms

Suitable for building production applications that need stable operation, rather than simple plugin testing or prototype development.

## Documentation

- [API Documentation](https://docs.rs/mofa)
- [GitHub Repository](https://github.com/mofa-org/mofa)
- [Examples](examples/)

## Contributing

We welcome contributions! Please check out our [contributing guide](CONTRIBUTING.md) for more details.

## Community

- GitHub Issues: [https://github.com/mofa-org/mofa/discussions](https://github.com/mofa-org/mofa/discussions)
- Discord: [https://discord.com/invite/hKJZzDMMm9](https://discord.com/invite/hKJZzDMMm9)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=mofa-org/mofa&type=Date)](https://www.star-history.com/#mofa-org/mofa&Date)

## 🙏 Acknowledgments

MoFA stands on the shoulders of giants:

- [Rust](https://www.rust-lang.org/) - Perfect combination of performance and safety
- [UniFFI](https://mozilla.github.io/uniffi-rs/) - Mozilla's multi-language binding magic
- [Rhai](https://rhai.rs/) - Powerful embedded scripting engine
- [Tokio](https://tokio.rs/) - Async runtime cornerstone
- [Ractor](https://github.com/slawlor/ractor) - Actor model concurrency framework
- [Dora](https://github.com/dora-rs/dora) - Distributed dataflow runtime
- [Wasmtime](https://wasmtime.dev/) - WebAssembly runtime

## Support

源起之道支持｜Supported by Upstream Labs

## License

[Apache License 2.0](./LICENSE)
