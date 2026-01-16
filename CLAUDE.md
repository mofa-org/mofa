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
