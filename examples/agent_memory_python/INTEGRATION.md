# Memory Module Integration Guide

This guide shows how to integrate the persistent memory module into your MoFA agent workflows.

## Quick Start: Add Memory to Your Agent

### 1. Basic Integration

```python
from memory_module import MemoryManager, AgentMemoryWorkflow

# Initialize memory with a simple responder
memory = MemoryManager()

def my_responder(prompt: str) -> str:
    # Your agent logic here
    return "response"

workflow = AgentMemoryWorkflow(memory=memory, responder=my_responder)

# Use the workflow
result = workflow.handle_query("What can you help with?", top_k=5)
print(result.response)
```

### 2. With OpenAI LLM

```python
import os
from openai import OpenAI
from memory_module import AgentMemoryWorkflow, MemoryManager

def llm_responder(prompt: str) -> str:
    client = OpenAI(api_key=os.getenv("OPENAI_API_KEY"))
    response = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[{"role": "user", "content": prompt}],
        max_tokens=500
    )
    return response.choices[0].message.content

memory = MemoryManager()
workflow = AgentMemoryWorkflow(memory=memory, responder=llm_responder)
```

### 3. Multi-Agent Coordination

Each agent maintains separate memory:

```python
# Agent A
memory_a = MemoryManager(collection_name="agent_a_memories")
workflow_a = AgentMemoryWorkflow(memory=memory_a, responder=responder_a)

# Agent B
memory_b = MemoryManager(collection_name="agent_b_memories")
workflow_b = AgentMemoryWorkflow(memory=memory_b, responder=responder_b)

# Agents coordinate through shared context
result_a = workflow_a.handle_query("Analyze this data")
result_b = workflow_b.handle_query(f"Based on: {result_a.response}, decide...")
```

## Core Concepts

### MemoryManager

Manages persistent storage of agent interactions.

```python
memory = MemoryManager(
    persist_directory=".agent_memories",  # Where vectors are stored
    collection_name="my_agent",            # Name of the collection
    embedding_function=custom_embeddings   # Optional: custom embeddings
)

# Store a memory
memory_id = memory.store(
    content="Important information",
    metadata={"topic": "data", "priority": "high"}
)

# Retrieve semantic matches
results = memory.retrieve("What was important?", top_k=5)
for result in results:
    print(f"[{result.score:.2f}] {result.content}")

# Update or delete
memory.update(memory_id, content="Updated information")
memory.delete(memory_id)
```

### AgentMemoryWorkflow

Orchestrates the retrieval-augmented prompt generation and storage.

```python
workflow = AgentMemoryWorkflow(memory=memory, responder=my_responder)

result = workflow.handle_query(
    user_query="What should I do?",
    top_k=5,                             # Number of memories to retrieve
    session_id="user_123"                # For tracking sessions
)

# Result contains:
# - result.prompt: The augmented prompt sent to responder
# - result.response: The agent's response
# - result.memory_id: ID of stored interaction
```

## Architecture Patterns

### Pattern 1: Single Agent with Memory

```python
memory = MemoryManager()
workflow = AgentMemoryWorkflow(memory, responder)

# User query → Retrieve memories → Augment prompt → Generate response → Store
while True:
    user_query = input("Query: ")
    result = workflow.handle_query(user_query)
    print(result.response)
```

### Pattern 2: Multi-Turn Conversation

```python
memory = MemoryManager()
workflow = AgentMemoryWorkflow(memory, responder)

# Each turn builds on previous memories
queries = [
    "Tell me about your experience in Python",
    "What was your biggest project?",
    "How did you approach that challenge?"
]

for query in queries:
    result = workflow.handle_query(query, session_id="interview")
    store_response(result)  # For your records
```

### Pattern 3: Multi-Agent with Shared Context

```python
from memory_module import MemoryManager

# Shared knowledge base
shared_memory = MemoryManager(collection_name="shared_knowledge")

# Individual agent memories
agent_a_memory = MemoryManager(collection_name="agent_a")
agent_b_memory = MemoryManager(collection_name="agent_b")

# Agents can cross-query
shared_context = shared_memory.retrieve("market trends", top_k=3)
result_a = workflow_a.handle_query(
    f"Given trends {shared_context}, analyze: {query}"
)
```

### Pattern 4: Session-Based Memory

```python
workflow = AgentMemoryWorkflow(memory, responder)

# Group interactions by session
session_id = "customer_support_ticket_42"
result = workflow.handle_query(user_query, session_id=session_id)

# Later, retrieve all memories from this session
all_session_memories = memory.retrieve(
    "ticket summary",
    top_k=10
)
session_specific = [m for m in all_session_memories 
                    if m.metadata.get("session_id") == session_id]
```

## Custom Embedding Functions

Use domain-specific embeddings:

```python
from memory_module import MemoryManager

class CustomEmbeddings:
    def __call__(self, texts: list[str]) -> list[list[float]]:
        # Your embedding logic
        return embeddings
    
    @staticmethod
    def name():
        return "custom"

memory = MemoryManager(
    embedding_function=CustomEmbeddings()
)
```

## Best Practices

1. **Namespace Collections**: Use descriptive collection names to avoid conflicts
   ```python
   MemoryManager(collection_name="customer_support_agent")
   ```

2. **Clear Metadata**: Always include contextual metadata
   ```python
   memory.store(content, metadata={"source": "user", "type": "question", "timestamp": now})
   ```

3. **Manage Storage**: Periodically prune old memories to save space
   ```python
   # Get all memories and filter by age
   results = memory.retrieve("*", top_k=1000)
   for result in results:
       if is_old(result.metadata.get("created_at")):
           memory.delete(result.memory_id)
   ```

4. **Monitor Performance**: Use top_k judiciously
   - Small values (3-5) for fast response
   - Larger values (10-20) for thorough research

## Testing

```bash
# Run unit tests
pytest tests/memory/test_memory_manager.py

# Test with custom responder
def test_responder():
    def dummy_responder(prompt):
        return "test response"
    
    memory = MemoryManager()
    workflow = AgentMemoryWorkflow(memory, dummy_responder)
    result = workflow.handle_query("test")
    assert result.response == "test response"
```

## Troubleshooting

**Issue**: "chromadb is required"
```bash
pip install chromadb sentence-transformers
```

**Issue**: Slow retrieval
- Use smaller embedding model
- Reduce top_k value
- Archive old memories

**Issue**: Memory consuming too much disk
- Delete old memories regularly
- Use collection_name to separate concerns
- Delete unused collections
