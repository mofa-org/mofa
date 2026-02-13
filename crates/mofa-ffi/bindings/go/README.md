# MoFA SDK Go

Go bindings for the MoFA (Modular Framework for Agents) SDK - a production-grade AI agent framework built in Rust.

## Installation

```bash
go get github.com/mofa-org/mofa-go@latest
```

For a specific version:

```bash
go get github.com/mofa-org/mofa-go@v0.1.0
```

### From Source

```bash
# Install UniFFI bindgen for Go
go install github.com/NordSecurity/uniffi-bindgen-go/cmd/uniffi-bindgen-go@latest

# Generate bindings
cd crates/mofa-sdk/bindings/go
./generate-go.sh

# Build and install
go build ./...
go install ./...
```

## Quick Start

```go
package main

import (
    "fmt"
    "os"

    mofa "github.com/mofa-org/mofa-go"
    "github.com/mofa-org/mofa-go/providers"
    "github.com/mofa-org/mofa-go/types"
)

func main() {
    // Set your API key
    apiKey := os.Getenv("OPENAI_API_KEY")

    // Create an LLM agent
    agent, err := providers.NewLLMAgentBuilder().
        Provider(providers.ProviderTypeOpenAI).
        ModelName("gpt-4").
        ApiKey(apiKey).
        Build()
    if err != nil {
        panic(err)
    }

    // Simple Q&A (no context)
    response, err := agent.Ask("What is the capital of France?")
    if err != nil {
        panic(err)
    }
    fmt.Println(response)

    // Multi-turn chat (with context)
    response1, err := agent.Chat("My name is Alice")
    if err != nil {
        panic(err)
    }
    fmt.Println(response1)

    response2, err := agent.Chat("What's my name?")
    if err != nil {
        panic(err)
    }
    fmt.Println(response2) // Remembers: "Your name is Alice"

    // View conversation history
    history, err := agent.GetHistory()
    if err != nil {
        panic(err)
    }
    for _, msg := range history {
        fmt.Printf("%s: %s\n", msg.Role, msg.Content)
    }

    // Clear conversation history
    err = agent.ClearHistory()
    if err != nil {
        panic(err)
    }
}
```

## Advanced Usage

### Using Different Providers

```go
package main

import (
    mofa "github.com/mofa-org/mofa-go"
    "github.com/mofa-org/mofa-go/providers"
)

// OpenAI
agent, err := providers.NewLLMAgentBuilder().
    Provider(providers.ProviderTypeOpenAI).
    ModelName("gpt-4").
    ApiKey("your-key").
    Build()

// Ollama (local)
agent, err := providers.NewLLMAgentBuilder().
    Provider(providers.ProviderTypeOllama).
    ModelName("llama2").
    BaseUrl("http://localhost:11434").
    Build()

// Azure OpenAI
agent, err := providers.NewLLMAgentBuilder().
    Provider(providers.ProviderTypeAzure).
    ModelName("gpt-4").
    ApiKey("your-key").
    Endpoint("https://your-resource.openai.azure.com").
    Deployment("your-deployment").
    Build()

// Compatible (e.g., localai, vllm)
agent, err := providers.NewLLMAgentBuilder().
    Provider(providers.ProviderTypeCompatible).
    ModelName("local-model").
    BaseUrl("http://localhost:8080").
    Build()
```

### Custom Configuration

```go
package main

import (
    mofa "github.com/mofa-org/mofa-go"
    "github.com/mofa-org/mofa-go/providers"
)

agent, err := providers.NewLLMAgentBuilder().
    Provider(providers.ProviderTypeOpenAI).
    ModelName("gpt-4").
    ApiKey("your-key").
    Temperature(0.7).
    MaxTokens(1000).
    TopP(0.9).
    Timeout(30).
    Build()
```

### Error Handling

```go
package main

import (
    "fmt"
    "log"

    mofa "github.com/mofa-org/mofa-go"
    "github.com/mofa-org/mofa-go/providers"
    mofaerrors "github.com/mofa-org/mofa-go/errors"
)

func main() {
    agent, err := providers.NewLLMAgentBuilder().
        Provider(providers.ProviderTypeOpenAI).
        ModelName("gpt-4").
        Build()
    if err != nil {
        if mofaErr, ok := err.(*mofaerrors.MoFaError); ok {
            switch mofaErr.Code() {
            case mofaerrors.ConfigurationError:
                log.Fatalf("Configuration error: %v", err)
            case mofaerrors.ProviderError:
                log.Fatalf("Provider error: %v", err)
            case mofaerrors.RuntimeError:
                log.Fatalf("Runtime error: %v", err)
            default:
                log.Fatalf("Unknown error: %v", err)
            }
        } else {
            log.Fatalf("Error: %v", err)
        }
    }

    response, err := agent.Ask("Hello!")
    if err != nil {
        log.Fatalf("Ask failed: %v", err)
    }
    fmt.Println(response)
}
```

### Working with Chat History

```go
package main

import (
    "fmt"

    mofa "github.com/mofa-org/mofa-go"
    "github.com/mofa-org/mofa-go/providers"
    "github.com/mofa-org/mofa-go/types"
)

func main() {
    agent, err := providers.NewLLMAgentBuilder().Build()
    if err != nil {
        panic(err)
    }

    // Get and inspect history
    history, err := agent.GetHistory()
    if err != nil {
        panic(err)
    }

    for _, msg := range history {
        role := msg.Role
        content := msg.Content
        if len(content) > 50 {
            content = content[:50] + "..."
        }
        fmt.Printf("%s: %s\n", role, content)
    }
}
```

## Utility Functions

```go
package main

import (
    "fmt"

    mofa "github.com/mofa-org/mofa-go"
)

func main() {
    // Get SDK version
    version := mofa.GetVersion()
    fmt.Printf("MoFA SDK version: %s\n", version)

    // Check if Dora-rs is available
    hasDora := mofa.IsDoraAvailable()
    fmt.Printf("Dora-rs available: %t\n", hasDora)
}
```

## Requirements

- Go 1.21 or higher
- Supported platforms: Linux, macOS, Windows

## Native Library

The Go bindings include a native library that is automatically loaded via cgo. The library is built from Rust using UniFFI for cross-language bindings.

### Loading the Native Library

The native library (`libmofa_sdk.so`, `libmofa_sdk.dylib`, or `mofa_sdk.dll`) must be available in your library search path. You can:

1. Install the library system-wide
2. Place it in the same directory as your executable
3. Set `LD_LIBRARY_PATH` (Linux), `DYLD_LIBRARY_PATH` (macOS), or `PATH` (Windows)

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
