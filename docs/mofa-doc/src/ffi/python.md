# Python Bindings

Use MoFA from Python with native-feeling APIs.

## Installation

```bash
pip install mofa
```

Or for native PyO3 bindings:

```bash
pip install mofa-native
```

## Quick Start

```python
import os
from mofa import LLMClient, AgentInput, AgentRunner

# Configure LLM
os.environ["OPENAI_API_KEY"] = "sk-..."

# Create client
client = LLMClient.from_env()

# Simple query
response = client.ask("What is Rust?")
print(response)

# With system prompt
response = client.ask_with_system(
    system="You are a Rust expert.",
    prompt="Explain ownership."
)
print(response)
```

## Agent Implementation

```python
from mofa import MoFAAgent, AgentContext, AgentInput, AgentOutput, AgentState

class MyAgent(MoFAAgent):
    def __init__(self, client):
        self.client = client
        self._state = AgentState.CREATED

    @property
    def id(self):
        return "my-agent"

    @property
    def name(self):
        return "My Agent"

    async def initialize(self, ctx: AgentContext):
        self._state = AgentState.READY

    async def execute(self, input: AgentInput, ctx: AgentContext) -> AgentOutput:
        self._state = AgentState.EXECUTING
        response = await self.client.ask(input.to_text())
        self._state = AgentState.READY
        return AgentOutput.text(response)

    async def shutdown(self):
        self._state = AgentState.SHUTDOWN

    @property
    def state(self):
        return self._state
```

## Using AgentRunner

```python
import asyncio
from mofa import AgentRunner, AgentInput

async def main():
    agent = MyAgent(LLMClient.from_env())
    runner = await AgentRunner.new(agent)

    output = await runner.execute(AgentInput.text("Hello!"))
    print(output.as_text())

    await runner.shutdown()

asyncio.run(main())
```

## Streaming

```python
async def stream_example():
    client = LLMClient.from_env()

    async for chunk in client.stream("Tell me a story"):
        print(chunk, end="", flush=True)
```

## Tools

```python
from mofa import Tool, ToolError
import json

class CalculatorTool(Tool):
    @property
    def name(self):
        return "calculator"

    @property
    def description(self):
        return "Performs arithmetic operations"

    @property
    def parameters_schema(self):
        return {
            "type": "object",
            "properties": {
                "operation": {"type": "string"},
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["operation", "a", "b"]
        }

    async def execute(self, params):
        op = params["operation"]
        a, b = params["a"], params["b"]

        if op == "add":
            return {"result": a + b}
        elif op == "multiply":
            return {"result": a * b}
        else:
            raise ToolError(f"Unknown operation: {op}")
```

## ReAct Agent

```python
from mofa import ReActAgent, SimpleToolRegistry

# Register tools
registry = SimpleToolRegistry()
registry.register(CalculatorTool())

# Create agent
agent = ReActAgent.builder() \
    .with_llm(LLMClient.from_env()) \
    .with_tools(registry) \
    .build()

# Execute
output = await agent.execute(
    AgentInput.text("What is 25 times 4?"),
    AgentContext.new("exec-001")
)
print(output.as_text())
```

## Async Support

MoFA Python bindings are fully async:

```python
import asyncio
from mofa import LLMClient

async def process_multiple(client, queries):
    tasks = [client.ask(q) for q in queries]
    return await asyncio.gather(*tasks)

async def main():
    client = LLMClient.from_env()
    results = await process_multiple(client, [
        "What is Rust?",
        "What is Python?",
        "What is Go?"
    ])
    for r in results:
        print(r)

asyncio.run(main())
```

## Error Handling

```python
from mofa import AgentError, LLMError

try:
    response = await client.ask("Hello")
except LLMError.RateLimited as e:
    print(f"Rate limited. Retry after {e.retry_after}s")
except LLMError.InvalidApiKey:
    print("Check your API key")
except AgentError.ExecutionFailed as e:
    print(f"Execution failed: {e}")
```

## See Also

- [Cross-Language Overview](README.md) — All bindings
- [Installation](../getting-started/installation.md) — Setup guide
