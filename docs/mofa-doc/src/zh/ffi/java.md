# Java 绑定

从 Java 应用程序使用 MoFA。

## 安装

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

## 快速开始

```java
import org.mofa.sdk.*;
import java.util.concurrent.CompletableFuture;

public class Main {
    public static void main(String[] args) {
        // 配置
        System.setProperty("OPENAI_API_KEY", "sk-...");

        // 创建客户端
        LLMClient client = LLMClient.fromEnv();

        // 简单查询
        String response = client.ask("什么是 Rust?");
        System.out.println(response);

        // 异步查询
        CompletableFuture<String> future = client.askAsync("你好");
        future.thenAccept(System.out::println);
    }
}
```

## 智能体实现

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

## 另见

- [跨语言概述](README.md) — 所有绑定
