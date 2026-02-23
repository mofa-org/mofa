# Swift 绑定

从 Swift 应用程序 (iOS/macOS) 使用 MoFA。

## 安装

### Swift Package Manager

添加到 `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/mofa-org/mofa-swift.git", from: "0.1.0")
]
```

或在 Xcode 中:
1. File → Add Packages
2. 输入: `https://github.com/mofa-org/mofa-swift`
3. 选择版本 `0.1.0`

## 快速开始

```swift
import MoFA

// 配置
ProcessInfo.processInfo.environment["OPENAI_API_KEY"] = "sk-..."

// 创建客户端
let client = LLMClient.fromEnv()

// 简单查询
let response = try await client.ask("什么是 Rust?")
print(response)

// 带系统提示
let response = try await client.askWithSystem(
    system: "你是一个 Swift 专家。",
    prompt: "解释可选类型。"
)
print(response)
```

## 智能体实现

```swift
import MoFA

class MyAgent: MoFAAgent {
    var id: String { "my-agent" }
    var name: String { "My Agent" }

    private var state: AgentState = .created
    private let llm: LLMClient

    init(llm: LLMClient) {
        self.llm = llm
    }

    func initialize(ctx: AgentContext) async throws {
        state = .ready
    }

    func execute(input: AgentInput, ctx: AgentContext) async throws -> AgentOutput {
        state = .executing
        let response = try await llm.ask(input.toText())
        state = .ready
        return .text(response)
    }

    func shutdown() async throws {
        state = .shutdown
    }
}
```

## 使用 AgentRunner

```swift
let agent = MyAgent(llm: client)
let runner = try await AgentRunner(agent: agent)

let output = try await runner.execute(input: .text("你好!"))
print(output.asText() ?? "(无输出)")

try await runner.shutdown()
```

## 流式传输

```swift
let stream = try await client.stream()
    .system("你很有帮助。")
    .user("讲个故事")
    .start()

for try await chunk in stream {
    print(chunk, terminator: "")
}
```

## 与 SwiftUI 集成

```swift
import SwiftUI
import MoFA

class AgentViewModel: ObservableObject {
    @Published var response: String = ""
    private let client = LLMClient.fromEnv()

    func ask(_ question: String) async {
        do {
            response = try await client.ask(question)
        } catch {
            response = "错误: \(error.localizedDescription)"
        }
    }
}

struct ContentView: View {
    @StateObject var viewModel = AgentViewModel()

    var body: some View {
        VStack {
            Text(viewModel.response)
            Button("提问") {
                Task {
                    await viewModel.ask("什么是 Swift?")
                }
            }
        }
    }
}
```

## 另见

- [跨语言概述](README.md) — 所有绑定
- [Python 绑定](python.md) — Python 指南
