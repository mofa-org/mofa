# MoFA Python Bindings Examples

This directory contains Python examples demonstrating how to use the MoFA SDK through UniFFI-generated Python bindings.

## Prerequisites

1. Build the MoFA SDK library with UniFFI support:
```bash
cd /path/to/mofa
cargo build --release --features "uniffi,openai" -p mofa-sdk
```

2. Generate Python bindings:
```bash
cd crates/mofa-sdk
./generate-bindings.sh python
```

3. Set your OpenAI API key:
```bash
export OPENAI_API_KEY=your-key-here
```

Optional environment variables:
```bash
export OPENAI_BASE_URL=https://api.openai.com/v1  # Custom base URL
export OPENAI_MODEL=gpt-3.5-turbo                   # Model to use
```

## Running Examples

### Example 1: Basic LLM Agent
```bash
python 01_llm_agent.py
```

Demonstrates:
- Creating an LLM agent using the builder pattern
- Simple Q&A (ask method)
- Multi-turn chat with context retention
- Getting conversation history
- Clearing history

### Example 2: Version Information
```bash
python 02_version_info.py
```

Demonstrates:
- Getting SDK version
- Checking Dora runtime availability

## Code Example

```python
from mofa import LLMAgentBuilder
import os

# Create an agent
builder = LLMAgentBuilder.create()
builder = builder.set_id("my-agent")
builder = builder.set_name("Python Agent")
builder = builder.set_system_prompt("You are a helpful assistant.")
builder = builder.set_openai_provider(
    os.environ["OPENAI_API_KEY"],
    base_url=os.environ.get("OPENAI_BASE_URL"),
    model=os.environ.get("OPENAI_MODEL", "gpt-3.5-turbo")
)

agent = builder.build()

# Use the agent
response = agent.ask("What is Python?")
print(response)

# Multi-turn chat
r1 = agent.chat("My name is Alice.")
r2 = agent.chat("What's my name?")  # Remembers context
```

## Available Functions

- `get_version()` - Get SDK version string
- `is_dora_available()` - Check if Dora runtime is enabled
- `new_llm_agent_builder()` - Create a new LLMAgentBuilder

## LLMAgentBuilder Methods

- `set_id(id)` - Set agent ID
- `set_name(name)` - Set agent name
- `set_system_prompt(prompt)` - Set system prompt
- `set_temperature(temp)` - Set temperature (0.0-1.0)
- `set_max_tokens(tokens)` - Set max tokens
- `set_openai_provider(key, url, model)` - Configure OpenAI provider
- `build()` - Build the agent

## LLMAgent Methods

- `agent_id()` - Get agent ID
- `name()` - Get agent name
- `ask(question)` - Simple Q&A (no context)
- `chat(message)` - Multi-turn chat (with context)
- `clear_history()` - Clear conversation history
- `get_history()` - Get conversation history
