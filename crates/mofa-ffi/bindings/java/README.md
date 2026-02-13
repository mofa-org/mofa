# MoFA SDK Java

Java bindings for the MoFA (Modular Framework for Agents) SDK - a production-grade AI agent framework built in Rust.

## Installation

### Maven

Add the following to your `pom.xml`:

```xml
<dependencies>
    <dependency>
        <groupId>org.mofa</groupId>
        <artifactId>mofa-sdk</artifactId>
        <version>0.1.0</version>
    </dependency>
</dependencies>
```

### Gradle

Add the following to your `build.gradle`:

```groovy
dependencies {
    implementation 'org.mofa:mofa-sdk:0.1.0'
}
```

For Gradle Kotlin DSL:

```kotlin
dependencies {
    implementation("org.mofa:mofa-sdk:0.1.0")
}
```

### From Source

```bash
# Install UniFFI bindgen for Java
cargo install uniffi-bindgen-java

# Generate bindings
cd crates/mofa-sdk
./generate-bindings.sh java

# Build with Maven
cd bindings/java
mvn clean install
```

## Quick Start

```java
import org.mofa.*;
import org.mofa.providers.*;
import org.mofa.types.*;

public class Example {
    public static void main(String[] args) {
        // Set your API key
        System.getenv("OPENAI_API_KEY");

        // Create an LLM agent
        LLMAgent agent = new LLMAgentBuilder()
            .provider(ProviderType.OPENAI)
            .modelName("gpt-4")
            .apiKey(System.getenv("OPENAI_API_KEY"))
            .build();

        // Simple Q&A (no context)
        String response = agent.ask("What is the capital of France?");
        System.out.println(response);

        // Multi-turn chat (with context)
        String response1 = agent.chat("My name is Alice");
        System.out.println(response1);

        String response2 = agent.chat("What's my name?");
        System.out.println(response2); // Remembers: "Your name is Alice"

        // View conversation history
        List<ChatMessage> history = agent.getHistory();
        for (ChatMessage msg : history) {
            System.out.println(msg.getRole() + ": " + msg.getContent());
        }

        // Clear conversation history
        agent.clearHistory();
    }
}
```

## Advanced Usage

### Using Different Providers

```java
import org.mofa.*;
import org.mofa.providers.*;

// OpenAI
LLMAgent agent = new LLMAgentBuilder()
    .provider(ProviderType.OPENAI)
    .modelName("gpt-4")
    .apiKey("your-key")
    .build();

// Ollama (local)
LLMAgent agent = new LLMAgentBuilder()
    .provider(ProviderType.OLLAMA)
    .modelName("llama2")
    .baseUrl("http://localhost:11434")
    .build();

// Azure OpenAI
LLMAgent agent = new LLMAgentBuilder()
    .provider(ProviderType.AZURE)
    .modelName("gpt-4")
    .apiKey("your-key")
    .endpoint("https://your-resource.openai.azure.com")
    .deployment("your-deployment")
    .build();

// Compatible (e.g., localai, vllm)
LLMAgent agent = new LLMAgentBuilder()
    .provider(ProviderType.COMPATIBLE)
    .modelName("local-model")
    .baseUrl("http://localhost:8080")
    .build();
```

### Custom Configuration

```java
import org.mofa.*;

LLMAgent agent = new LLMAgentBuilder()
    .provider(ProviderType.OPENAI)
    .modelName("gpt-4")
    .apiKey("your-key")
    .temperature(0.7)
    .maxTokens(1000)
    .topP(0.9)
    .timeout(30)
    .build();
```

### Error Handling

```java
import org.mofa.*;
import org.mofa.errors.*;

try {
    LLMAgent agent = new LLMAgentBuilder()
        .provider(ProviderType.OPENAI)
        .modelName("gpt-4")
        .build();
    String response = agent.ask("Hello!");
} catch (MoFaException e) {
    if (e instanceof MoFaException.ConfigurationException) {
        System.err.println("Configuration error: " + e.getMessage());
    } else if (e instanceof MoFaException.ProviderException) {
        System.err.println("Provider error: " + e.getMessage());
    } else if (e instanceof MoFaException.RuntimeException) {
        System.err.println("Runtime error: " + e.getMessage());
    }
}
```

### Working with Chat History

```java
import org.mofa.*;
import org.mofa.types.*;

LLMAgent agent = new LLMAgentBuilder().build();

// Get and inspect history
List<ChatMessage> history = agent.getHistory();
for (ChatMessage msg : history) {
    ChatRole role = msg.getRole();
    String content = msg.getContent();
    System.out.println(role + ": " + content.substring(0, Math.min(50, content.length())) + "...");
}
```

## Utility Functions

```java
import org.mofa.*;

// Get SDK version
String version = MoFa.getVersion();
System.out.println("MoFA SDK version: " + version);

// Check if Dora-rs is available
boolean hasDora = MoFa.isDoraAvailable();
System.out.println("Dora-rs available: " + hasDora);
```

## Requirements

- Java 11 or higher
- Supported platforms: Linux, macOS, Windows

## Native Library

The Java bindings include a native library that is automatically loaded. The library is built from Rust using UniFFI for cross-language bindings.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT license

## Contributing

Contributions are welcome! Please visit [GitHub](https://github.com/mofa-org/mofa) for more information.

## Support

- Documentation: https://docs.mofa.org
- Issues: https://github.com/mofa-org/mofa/issues
- Discussions: https://github.com/mofa-org/mofa/discussions
