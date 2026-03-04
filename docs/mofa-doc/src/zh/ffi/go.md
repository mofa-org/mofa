# Go 绑定

从 Go 应用程序使用 MoFA。

## 安装

```bash
go get github.com/mofa-org/mofa-go
```

## 快速开始

```go
package main

import (
    "fmt"
    "os"

    "github.com/mofa-org/mofa-go/mofa"
)

func main() {
    // 配置
    os.Setenv("OPENAI_API_KEY", "sk-...")

    // 创建客户端
    client := mofa.NewLLMClient()

    // 简单查询
    response := client.Ask("什么是 Rust?")
    fmt.Println(response)

    // 异步查询
    ch := client.AskAsync("你好")
    result := <-ch
    fmt.Println(result)
}
```

## 智能体实现

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

## 另见

- [跨语言概述](README.md) — 所有绑定
