# Kotlin 绑定

从 Kotlin 应用程序 (Android/JVM) 使用 MoFA。

## 安装

### Gradle (Kotlin DSL)

```kotlin
dependencies {
    implementation("org.mofa:mofa-kotlin:0.1.0")
}
```

### Gradle (Groovy DSL)

```groovy
dependencies {
    implementation 'org.mofa:mofa-kotlin:0.1.0'
}
```

### Maven

```xml
<dependency>
    <groupId>org.mofa</groupId>
    <artifactId>mofa-kotlin</artifactId>
    <version>0.1.0</version>
</dependency>
```

## 快速开始

```kotlin
import org.mofa.sdk.*

suspend fun main() {
    // 配置
    System.setProperty("OPENAI_API_KEY", "sk-...")

    // 创建客户端
    val client = LLMClient.fromEnv()

    // 简单查询
    val response = client.ask("什么是 Kotlin?")
    println(response)

    // 带系统提示
    val response = client.askWithSystem(
        system = "你是一个 Kotlin 专家。",
        prompt = "解释协程。"
    )
    println(response)
}
```

## 智能体实现

```kotlin
import org.mofa.sdk.*

class MyAgent(
    private val llm: LLMClient
) : MoFAAgent {

    override val id: String = "my-agent"
    override val name: String = "My Agent"

    private var state: AgentState = AgentState.CREATED

    override suspend fun initialize(ctx: AgentContext) {
        state = AgentState.READY
    }

    override suspend fun execute(input: AgentInput, ctx: AgentContext): AgentOutput {
        state = AgentState.EXECUTING
        val response = llm.ask(input.toText())
        state = AgentState.READY
        return AgentOutput.text(response)
    }

    override suspend fun shutdown() {
        state = AgentState.SHUTDOWN
    }
}
```

## 使用 AgentRunner

```kotlin
val agent = MyAgent(client)
val runner = AgentRunner(agent)

val output = runner.execute(AgentInput.text("你好!"))
println(output.asText())

runner.shutdown()
```

## 协程支持

MoFA Kotlin 绑定完全支持协程:

```kotlin
import kotlinx.coroutines.*

suspend fun processMultiple(client: LLMClient, queries: List<String>): List<String> {
    return coroutineScope {
        queries.map { query ->
            async { client.ask(query) }
        }.awaitAll()
    }
}

// 用法
val results = processMultiple(client, listOf(
    "什么是 Kotlin?",
    "什么是 Rust?",
    "什么是 Go?"
))
results.forEach { println(it) }
```

## Android 集成

```kotlin
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import org.mofa.sdk.*

class AgentViewModel : ViewModel() {
    private val client = LLMClient.fromEnv()

    private val _response = MutableStateFlow("")
    val response: StateFlow<String> = _response

    fun ask(question: String) {
        viewModelScope.launch {
            try {
                _response.value = client.ask(question)
            } catch (e: Exception) {
                _response.value = "错误: ${e.message}"
            }
        }
    }
}
```

## 错误处理

```kotlin
try {
    val response = client.ask("你好")
    println(response)
} catch (e: LLMError.RateLimited) {
    println("速率限制。${e.retryAfter}秒后重试")
} catch (e: LLMError.InvalidApiKey) {
    println("检查您的 API 密钥")
} catch (e: AgentError.ExecutionFailed) {
    println("执行失败: ${e.message}")
}
```

## 另见

- [跨语言概述](README.md) — 所有绑定
- [Java 绑定](java.md) — Java 指南
