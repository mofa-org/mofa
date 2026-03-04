# Go Bindings

Use MoFA from Go applications.

## Installation

```bash
go get github.com/mofa-org/mofa-go
```

## Quick Start

```go
package main

import (
    "fmt"
    "os"

    "github.com/mofa-org/mofa-go/mofa"
)

func main() {
    // Configure
    os.Setenv("OPENAI_API_KEY", "sk-...")

    // Create client
    client := mofa.NewLLMClient()

    // Simple query
    response := client.Ask("What is Rust?")
    fmt.Println(response)

    // Async query
    ch := client.AskAsync("Hello")
    result := <-ch
    fmt.Println(result)
}
```

## Agent Implementation

```go
package main

import (
    "github.com/mofa-org/mofa-go/mofa"
)

type MyAgent struct {
    id    string
    name  string
    state mofa.AgentState
    llm   *mofa.LLMClient
}

func NewMyAgent(llm *mofa.LLMClient) *MyAgent {
    return &MyAgent{
        id:    "my-agent",
        name:  "My Agent",
        state: mofa.StateCreated,
        llm:   llm,
    }
}

func (a *MyAgent) GetID() string {
    return a.id
}

func (a *MyAgent) GetName() string {
    return a.name
}

func (a *MyAgent) Execute(input mofa.AgentInput) (mofa.AgentOutput, error) {
    a.state = mofa.StateExecuting
    response := a.llm.Ask(input.Text())
    a.state = mofa.StateReady
    return mofa.TextOutput(response), nil
}
```

## See Also

- [Cross-Language Overview](README.md) — All bindings
