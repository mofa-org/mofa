# MoFA Java Bindings Examples

This directory contains Java examples demonstrating how to use the MoFA SDK through UniFFI-generated Java bindings.

## Prerequisites

1. Build the MoFA SDK library with UniFFI support:
```bash
cd /path/to/mofa
cargo build --release --features "uniffi,openai" -p mofa-sdk
```

2. Generate Java bindings (requires uniffi-bindgen-java):
```bash
cd crates/mofa-sdk
cargo install uniffi-bindgen-java
./generate-bindings.sh java
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

### Compile and run with Maven:
```bash
mvn compile exec:java -Dexec.mainClass="com.mofa.examples.LLMAgentExample"
```

### Example: Basic LLM Agent
Demonstrates:
- Creating an LLM agent using the builder pattern
- Simple Q&A (ask method)
- Multi-turn chat with context retention
- Getting conversation history
- Clearing history

## Code Example

```java
import com.mofa.*;

public class Example {
    public static void main(String[] args) throws MoFaError {
        // Create an agent
        LLMAgentBuilder builder = UniFFI.INSTANCE.newLlmAgentBuilder();
        builder = builder.setId("my-agent");
        builder = builder.setName("Java Agent");
        builder = builder.setSystemPrompt("You are a helpful assistant.");
        builder = builder.setOpenaiProvider(
            System.getenv("OPENAI_API_KEY"),
            System.getenv("OPENAI_BASE_URL"),
            System.getenv().getOrDefault("OPENAI_MODEL", "gpt-3.5-turbo")
        );

        LLMAgent agent = builder.build();

        // Use the agent
        String response = agent.ask("What is Java?");
        System.out.println(response);

        // Multi-turn chat
        String r1 = agent.chat("My name is Bob.");
        String r2 = agent.chat("What's my name?");  // Remembers context
    }
}
```

## Available Functions

| Function | Description |
|----------|-------------|
| `UniFFI.INSTANCE.getVersion()` | Get SDK version string |
| `UniFFI.INSTANCE.isDoraAvailable()` | Check if Dora runtime is enabled |
| `UniFFI.INSTANCE.newLlmAgentBuilder()` | Create a new LLMAgentBuilder |

## LLMAgentBuilder Methods

| Method | Description |
|--------|-------------|
| `setId(id)` | Set agent ID |
| `setName(name)` | Set agent name |
| `setSystemPrompt(prompt)` | Set system prompt |
| `setTemperature(temp)` | Set temperature (0.0-1.0) |
| `setMaxTokens(tokens)` | Set max tokens |
| `setOpenaiProvider(key, url, model)` | Configure OpenAI provider |
| `build()` | Build the agent |

## LLMAgent Methods

| Method | Description |
|--------|-------------|
| `agentId()` | Get agent ID |
| `name()` | Get agent name |
| `ask(question)` | Simple Q&A (no context) |
| `chat(message)` | Multi-turn chat (with context) |
| `clearHistory()` | Clear conversation history |
| `getHistory()` | Get conversation history |
