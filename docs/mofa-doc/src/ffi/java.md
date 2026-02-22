# Java Bindings

Use MoFA from Java applications.

## Installation

### Maven

```xml
<dependency>
    <groupId>org.mofa</groupId>
    <artifactId>mofa-java</artifactId>
    <version>0.1.0</version>
</dependency>
```

### Gradle

```groovy
implementation 'org.mofa:mofa-java:0.1.0'
```

## Quick Start

```java
import org.mofa.sdk.*;
import java.util.concurrent.CompletableFuture;

public class Main {
    public static void main(String[] args) {
        // Configure
        System.setProperty("OPENAI_API_KEY", "sk-...");

        // Create client
        LLMClient client = LLMClient.fromEnv();

        // Simple query
        String response = client.ask("What is Rust?");
        System.out.println(response);

        // Async query
        CompletableFuture<String> future = client.askAsync("Hello");
        future.thenAccept(System.out::println);
    }
}
```

## Agent Implementation

```java
public class MyAgent implements MoFAAgent {
    private AgentState state = AgentState.CREATED;
    private LLMClient llm;

    public MyAgent(LLMClient llm) {
        this.llm = llm;
    }

    @Override
    public String getId() {
        return "my-agent";
    }

    @Override
    public String getName() {
        return "My Agent";
    }

    @Override
    public CompletableFuture<Void> initialize(AgentContext ctx) {
        state = AgentState.READY;
        return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<AgentOutput> execute(AgentInput input, AgentContext ctx) {
        state = AgentState.EXECUTING;
        return llm.askAsync(input.toText())
            .thenApply(response -> {
                state = AgentState.READY;
                return AgentOutput.text(response);
            });
    }
}
```

## See Also

- [Cross-Language Overview](README.md) — All bindings
