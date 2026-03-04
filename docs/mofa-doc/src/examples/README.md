# Examples

Comprehensive examples demonstrating MoFA features.

## Categories

### Core Agents
Basic agent patterns including echo agents, LLM chat, and ReAct agents.

### Multi-Agent Coordination
Coordination patterns like sequential, parallel, debate, and consensus.

### Plugins
Plugin system examples including Rhai scripts, hot reloading, and WASM.

### Cross-Language Bindings
Python, Java, Go, Swift, and Kotlin FFI bindings.

### Domain-Specific
Financial compliance, medical diagnosis, and secretary agents.

### Streaming & Persistence
Database-backed conversations with PostgreSQL.

### Runtime System
Agent lifecycle, message bus, and backpressure handling.

### RAG & Knowledge
Retrieval-augmented generation with vector stores.

### Workflow DSL
YAML-based workflow definitions.

### Monitoring & Observability
Web dashboards and metrics collection.

### Multimodal & TTS
LLM streaming with text-to-speech integration.

### Advanced Patterns
Reflection agents and human-in-the-loop workflows.

## Running Examples

```bash
# From repository root
cargo run -p <example_name>

# Example
cargo run -p react_agent
cargo run -p rag_pipeline
```
