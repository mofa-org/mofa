# Chapter 9: What's Next

> **Learning objectives:** Know how to contribute to MoFA, explore advanced topics, and find resources for your GSoC journey.

Congratulations! You've built agents from scratch, connected them to LLMs, given them tools, orchestrated multi-agent teams, designed workflows, and written hot-reloadable plugins. You have a solid foundation for working with MoFA.

## Contributing to MoFA

MoFA is open source and welcomes contributions. Here's how to get started:

### 1. Read the Contributing Guide

The [CONTRIBUTING.md](../../CONTRIBUTING.md) covers:
- Branch naming conventions (kebab-case: `feat/my-feature`, `fix/bug-name`)
- Commit message format (Conventional Commits: `feat:`, `fix:`, `docs:`)
- PR guidelines and review process
- Architecture rules (the kernel/foundation separation from Chapter 1)

### 2. Find an Issue

Browse [GitHub Issues](https://github.com/moxin-org/mofa/issues) for:
- `good first issue` — Great for getting started
- `help wanted` — Community contributions welcome
- `gsoc` — Tagged for GSoC candidates

### 3. Development Workflow

```bash
# Create a feature branch
git checkout -b feat/my-feature

# Make changes, then check them
cargo check          # Fast compilation check
cargo fmt            # Format code
cargo clippy         # Lint
cargo test           # Run tests

# Commit (Conventional Commits format)
git commit -m "feat: add my new feature"

# Push and create a PR
git push -u origin feat/my-feature
```

## GSoC Project Ideas

Here are areas where MoFA would benefit from contributions. These make excellent GSoC project proposals:

### New LLM Providers
- **Difficulty**: Medium
- **Impact**: High
- Add providers for new LLM APIs (Mistral, Cohere, local model servers)
- Implement the `LLMProvider` trait (see `crates/mofa-foundation/src/llm/`)
- Reference: `openai.rs`, `anthropic.rs`, `ollama.rs` for patterns

### MCP Server Integrations
- **Difficulty**: Medium-Hard
- **Impact**: High
- Build MCP (Model Context Protocol) server integrations
- MoFA already has MCP client support (`mofa-kernel` traits, `mofa-foundation` client)
- Extend with new tool servers, resource providers, or prompt servers

### New Built-in Tools
- **Difficulty**: Easy-Medium
- **Impact**: Medium
- Create useful tools: database query, API client, code executor, web scraper
- Implement the `Tool` trait (Chapter 5)
- Add to `mofa-plugins` built-in tools collection

### Persistence Backend Improvements
- **Difficulty**: Medium
- **Impact**: Medium
- Improve existing PostgreSQL/MySQL/SQLite backends
- Add new backends (Redis, MongoDB, DynamoDB)
- See `crates/mofa-foundation/src/persistence/`

### Python Bindings Enhancement
- **Difficulty**: Medium-Hard
- **Impact**: High
- Improve PyO3/UniFFI bindings in `mofa-ffi`
- Make the Python API more Pythonic
- Add comprehensive Python examples and documentation

### Monitoring Dashboard
- **Difficulty**: Medium
- **Impact**: Medium
- Enhance the Axum-based web dashboard in `mofa-monitoring`
- Add real-time agent visualization, metrics graphs, trace viewer
- Integrate OpenTelemetry traces

### New Examples
- **Difficulty**: Easy
- **Impact**: Medium
- Create example agents for real-world use cases
- Document them well (README + inline comments)
- Good examples: RAG agent, code review agent, data analysis agent

### Workflow Engine Enhancements
- **Difficulty**: Medium-Hard
- **Impact**: High
- Add parallel node execution, sub-workflows, error recovery
- Improve the YAML DSL with more features
- Visual workflow editor (web-based)

## Advanced Topics to Explore

These are features we didn't cover in the tutorial but are available for you to explore:

### Secretary Agent (Human-in-the-Loop)
The secretary agent pattern manages tasks with human oversight — ideal for workflows where AI suggestions need human approval before execution.

```
Receive ideas → Clarify requirements → Schedule agents →
Monitor feedback → Push decisions to human → Update todos
```

See `examples/secretary_agent/` and `examples/hitl_secretary/`.

### MCP Protocol Integration
MoFA supports the Model Context Protocol for connecting to external tool servers:

```rust
use mofa_sdk::kernel::{McpClient, McpTool, McpToolRegistry};
```

See `crates/mofa-kernel/src/mcp/` for traits and `crates/mofa-foundation/src/mcp/` for the client.

### Persistence (PostgreSQL / SQLite)
Store conversation history, agent state, and session data in a database:

```rust
use mofa_sdk::persistence::{PersistencePlugin, PostgresStore};
```

See `examples/streaming_persistence/` and `examples/streaming_manual_persistence/`.

### FFI Bindings (Python, Java, Swift)
Call MoFA agents from other languages:

```python
# Python example (via PyO3)
from mofa import LLMAgent, OpenAIProvider

agent = LLMAgent(provider=OpenAIProvider.from_env())
response = agent.ask("Hello from Python!")
```

See `crates/mofa-ffi/` and `examples/python_bindings/`.

### Dora Distributed Dataflow
Run agents as nodes in a distributed dataflow graph:

```rust
use mofa_sdk::dora::{DoraRuntime, run_dataflow};

let result = run_dataflow("dataflow.yml").await?;
```

See the `dora` feature flag and `crates/mofa-runtime/src/dora/`.

### TTS (Text-to-Speech)
Give your agents a voice with the Kokoro TTS integration:

```rust
let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_tts_plugin(tts_plugin)
    .build();

agent.chat_with_tts(&session_id, "Tell me a joke").await?;
```

## Resources

- **Repository**: [github.com/moxin-org/mofa](https://github.com/moxin-org/mofa)
- **SDK Documentation**: See `crates/mofa-sdk/README.md`
- **Architecture Guide**: See `docs/architecture.md`
- **Security Guide**: See `docs/security.md`
- **Examples**: 27+ examples in the `examples/` directory

### Rust Learning Resources

If you're new to Rust, these resources complement this tutorial:

- [The Rust Book](https://doc.rust-lang.org/book/) — The official guide
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) — Learn through examples
- [Async Rust](https://rust-lang.github.io/async-book/) — Understanding async/await
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial) — The async runtime MoFA uses

## Thank You

Thank you for working through this tutorial! Whether you're here for GSoC or just exploring, we hope MoFA inspires you to build amazing AI agents. The framework is young and growing — your contributions will shape its future.

If you have questions, open an issue on GitHub or join the community discussions. We look forward to seeing what you build!

---

[← Back to Table of Contents](README.md)
