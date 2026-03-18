# Persistent Memory Module for MoFA Agents

This example provides a reusable Python memory layer for agent workflows.
It stores interactions in a persistent vector database and retrieves relevant memories by semantic similarity.

## Architecture

- Memory manager (`memory_module/memory_manager.py`)
  - Stores content and metadata
  - Retrieves semantically relevant memories using embeddings + Chroma vector index
  - Supports delete and update operations
- Workflow orchestrator (`memory_module/workflow.py`)
  - Retrieves top-k relevant memories for a user query
  - Appends memories to the prompt
  - Calls an agent responder
  - Stores the new interaction as long-term memory
- Demo entry points
  - `demo_agent.py`: two-turn scripted demo
  - `cli_demo.py`: interactive CLI demo

## Core API

- `memory.store(content: str, metadata: dict) -> str`
- `memory.retrieve(query: str, top_k: int = 5) -> list[MemoryRecord]`
- `memory.delete(memory_id: str) -> None`
- `memory.update(memory_id: str, content: str | None = None, metadata: dict | None = None) -> None`

## Install

From repository root:

```bash
cd examples/agent_memory_python
python -m pip install -r requirements.txt
```

For LLM-integrated examples, also install OpenAI:

```bash
python -m pip install openai
```

## Run scripted demo

```bash
cd examples/agent_memory_python
python demo_agent.py
```

Expected behavior:
- First interaction stores user preference memory.
- Second interaction retrieves the prior memory and uses it in the prompt.

## Run CLI demo

```bash
cd examples/agent_memory_python
python cli_demo.py
```

CLI commands:
- Regular text: run workflow and store interaction
- `/search <query>`: semantic retrieval
- `/delete <memory_id>`: remove a memory
- `/exit`: quit

## Run LLM-integrated example

Single-agent example that calls OpenAI:

```bash
OPENAI_API_KEY=sk-... python llm_integrated_example.py
```

This shows a memory-augmented agent that uses persistent memory with a real LLM responder.

## Run multi-agent example

Two-agent collaboration example using persistent memory:

```bash
OPENAI_API_KEY=sk-... python multi_agent_example.py
```

This demonstrates how multiple agents can share and build on memories in a collaborative workflow.

## Unit tests

```bash
cd /path/to/repo
pytest tests/memory/test_memory_manager.py
```

The tests validate:
- storing and retrieving memories
- deleting memories
- updating memories
