# MoFA SDK Python

Python bindings for the MoFA (Modular Framework for Agents) SDK - a production-grade AI agent framework built in Rust.

## Installation

### From PyPI (Recommended)

```bash
pip install mofa-sdk
```

### From Source

```bash
# Install maturin
pip install maturin

# Build and install
cd crates/mofa-sdk/bindings/python
maturin develop --release
```

## Quick Start

```python
import os
from mofa import (
    LLMAgentBuilder,
    ProviderType,
    ChatRole,
    ChatMessage,
)

# Set your API key
os.environ["OPENAI_API_KEY"] = "your-api-key-here"

# Create an LLM agent
agent = (
    LLMAgentBuilder()
    .provider(ProviderType.OPENAI)
    .model_name("gpt-4")
    .api_key(os.getenv("OPENAI_API_KEY"))
    .build()
)

# Simple Q&A (no context)
response = agent.ask("What is the capital of France?")
print(response)

# Multi-turn chat (with context)
response1 = agent.chat("My name is Alice")
print(response1)

response2 = agent.chat("What's my name?")
print(response2)  # Remembers: "Your name is Alice"

# View conversation history
history = agent.get_history()
for msg in history:
    print(f"{msg['role']}: {msg['content']}")

# Clear conversation history
agent.clear_history()
```

## Advanced Usage

### Using Different Providers

```python
from mofa import LLMAgentBuilder, ProviderType

# OpenAI
agent = (
    LLMAgentBuilder()
    .provider(ProviderType.OPENAI)
    .model_name("gpt-4")
    .api_key("your-key")
    .build()
)

# Ollama (local)
agent = (
    LLMAgentBuilder()
    .provider(ProviderType.OLLAMA)
    .model_name("llama2")
    .base_url("http://localhost:11434")
    .build()
)

# Azure OpenAI
agent = (
    LLMAgentBuilder()
    .provider(ProviderType.AZURE)
    .model_name("gpt-4")
    .api_key("your-key")
    .endpoint("https://your-resource.openai.azure.com")
    .deployment("your-deployment")
    .build()
)

# Compatible (e.g., localai, vllm)
agent = (
    LLMAgentBuilder()
    .provider(ProviderType.COMPATIBLE)
    .model_name("local-model")
    .base_url("http://localhost:8080")
    .build()
)
```

### Custom Configuration

```python
from mofa import LLMAgentBuilder, ProviderType

agent = (
    LLMAgentBuilder()
    .provider(ProviderType.OPENAI)
    .model_name("gpt-4")
    .api_key("your-key")
    .temperature(0.7)
    .max_tokens(1000)
    .top_p(0.9)
    .timeout(30)
    .build()
)
```

### Error Handling

```python
from mofa import LLMAgentBuilder, ProviderType, MoFaError

try:
    agent = (
        LLMAgentBuilder()
        .provider(ProviderType.OPENAI)
        .model_name("gpt-4")
        .build()
    )
    response = agent.ask("Hello!")
except MoFaError.ConfigurationError as e:
    print(f"Configuration error: {e}")
except MoFaError.ProviderError as e:
    print(f"Provider error: {e}")
except MoFaError.RuntimeError as e:
    print(f"Runtime error: {e}")
```

### Working with Chat History

```python
from mofa import LLMAgentBuilder, ChatRole

agent = LLMAgentBuilder().build()

# Add system message
history = [
    {"role": ChatRole.SYSTEM, "content": "You are a helpful assistant."},
    {"role": ChatRole.USER, "content": "Hello!"},
]

# You can also work with strings
history = [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello!"},
]

# Get and inspect history
history = agent.get_history()
for msg in history:
    role = msg["role"]
    content = msg["content"]
    print(f"{role}: {content[:50]}...")
```

## Utility Functions

```python
from mofa import get_version, is_dora_available

# Get SDK version
version = get_version()
print(f"MoFA SDK version: {version}")

# Check if Dora-rs is available
has_dora = is_dora_available()
print(f"Dora-rs available: {has_dora}")
```

## Requirements

- Python 3.8 or higher
- Supported platforms: Linux, macOS, Windows

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

## Contributing

Contributions are welcome! Please visit [GitHub](https://github.com/mofa-org/mofa) for more information.

## Support

- Documentation: https://docs.mofa.org
- Issues: https://github.com/mofa-org/mofa/issues
- Discussions: https://github.com/mofa-org/mofa/discussions
