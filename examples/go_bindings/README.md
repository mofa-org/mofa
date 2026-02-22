# MoFA Go Bindings Examples

This directory contains Go examples demonstrating how to use the MoFA SDK through UniFFI-generated Go bindings.

## Prerequisites

1. Build the MoFA SDK library with UniFFI support:
```bash
cd /path/to/mofa
cargo build --release --features "uniffi,openai" -p mofa-sdk
```

2. Install uniffi-bindgen-go:
```bash
cargo install uniffi-bindgen-go --git https://github.com/NordSecurity/uniffi-bindgen-go
```

3. Generate Go bindings:
```bash
cd /path/to/mofa/crates/mofa-sdk/bindings/go
./generate-go.sh
```

4. Set your OpenAI API key:
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
go run 01_llm_agent.go
```

Demonstrates:
- Creating an LLM agent using the builder pattern
- Simple Q&A (ask method)
- Multi-turn chat with context retention
- Getting conversation history
- Clearing history

## Code Example

```go
package main

import (
    "fmt"
    "os"
    mofa "mofa-sdk/bindings/go"
)

func main() {
    // Create an agent
    builder := mofa.NewLlmAgentBuilder()
    builder.SetId("my-agent")
    builder.SetName("Go Agent")
    builder.SetSystemPrompt("You are a helpful assistant.")
    builder.SetOpenaiProvider(
        os.Getenv("OPENAI_API_KEY"),
        os.Getenv("OPENAI_BASE_URL"),
        os.Getenv("OPENAI_MODEL"),
    )

    agent, _ := builder.Build()

    // Use the agent
    answer, _ := agent.Ask("What is Go?")
    fmt.Println(answer)

    // Multi-turn chat
    agent.Chat("My name is Charlie.")
    agent.Chat("What's my name?")  // Remembers context
}
```

## Available Functions

| Function | Description |
|----------|-------------|
| `GetVersion()` | Get SDK version string |
| `IsDoraAvailable()` | Check if Dora runtime is enabled |
| `NewLlmAgentBuilder()` | Create a new LLMAgentBuilder |

## LLMAgentBuilder Methods

| Method | Description |
|--------|-------------|
| `SetId(id)` | Set agent ID |
| `SetName(name)` | Set agent name |
| `SetSystemPrompt(prompt)` | Set system prompt |
| `SetTemperature(temp)` | Set temperature (0.0-1.0) |
| `SetMaxTokens(tokens)` | Set max tokens |
| `SetOpenaiProvider(key, url, model)` | Configure OpenAI provider |
| `Build()` | Build the agent |

## LLMAgent Methods

| Method | Description |
|--------|-------------|
| `AgentId()` | Get agent ID |
| `Name()` | Get agent name |
| `Ask(question)` | Simple Q&A (no context) |
| `Chat(message)` | Multi-turn chat (with context) |
| `ClearHistory()` | Clear conversation history |
| `GetHistory()` | Get conversation history |
