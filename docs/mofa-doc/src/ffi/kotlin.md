# Kotlin Bindings

Use MoFA from Kotlin applications (Android/JVM).

## Installation

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

## Quick Start

```kotlin
import org.mofa.sdk.*

suspend fun main() {
    // Configure
    System.setProperty("OPENAI_API_KEY", "sk-...")

    // Create client
    val client = LLMClient.fromEnv()

    // Simple query
    val response = client.ask("What is Kotlin?")
    println(response)

    // With system prompt
    val response = client.askWithSystem(
        system = "You are a Kotlin expert.",
        prompt = "Explain coroutines."
    )
    println(response)
}
```

## Agent Implementation

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

## Using AgentRunner

```kotlin
val agent = MyAgent(client)
val runner = AgentRunner(agent)

val output = runner.execute(AgentInput.text("Hello!"))
println(output.asText())

runner.shutdown()
```

## Coroutines Support

MoFA Kotlin bindings are fully coroutine-friendly:

```kotlin
import kotlinx.coroutines.*

suspend fun processMultiple(client: LLMClient, queries: List<String>): List<String> {
    return coroutineScope {
        queries.map { query ->
            async { client.ask(query) }
        }.awaitAll()
    }
}

// Usage
val results = processMultiple(client, listOf(
    "What is Kotlin?",
    "What is Rust?",
    "What is Go?"
))
results.forEach { println(it) }
```

## Android Integration

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
                _response.value = "Error: ${e.message}"
            }
        }
    }
}
```

## Error Handling

```kotlin
try {
    val response = client.ask("Hello")
    println(response)
} catch (e: LLMError.RateLimited) {
    println("Rate limited. Retry after ${e.retryAfter}s")
} catch (e: LLMError.InvalidApiKey) {
    println("Check your API key")
} catch (e: AgentError.ExecutionFailed) {
    println("Execution failed: ${e.message}")
}
```

## See Also

- [Cross-Language Overview](README.md) — All bindings
- [Java Bindings](java.md) — Java guide
