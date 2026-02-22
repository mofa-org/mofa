# Swift Bindings

Use MoFA from Swift applications (iOS/macOS).

## Installation

### Swift Package Manager

Add to `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/mofa-org/mofa-swift.git", from: "0.1.0")
]
```

Or in Xcode:
1. File → Add Packages
2. Enter: `https://github.com/mofa-org/mofa-swift`
3. Select version `0.1.0`

## Quick Start

```swift
import MoFA

// Configure
ProcessInfo.processInfo.environment["OPENAI_API_KEY"] = "sk-..."

// Create client
let client = LLMClient.fromEnv()

// Simple query
let response = try await client.ask("What is Rust?")
print(response)

// With system prompt
let response = try await client.askWithSystem(
    system: "You are a Swift expert.",
    prompt: "Explain optionals."
)
print(response)
```

## Agent Implementation

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

## Using AgentRunner

```swift
let agent = MyAgent(llm: client)
let runner = try await AgentRunner(agent: agent)

let output = try await runner.execute(input: .text("Hello!"))
print(output.asText() ?? "(no output)")

try await runner.shutdown()
```

## Streaming

```swift
let stream = try await client.stream()
    .system("You are helpful.")
    .user("Tell a story")
    .start()

for try await chunk in stream {
    print(chunk, terminator: "")
}
```

## Integration with SwiftUI

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
            response = "Error: \(error.localizedDescription)"
        }
    }
}

struct ContentView: View {
    @StateObject var viewModel = AgentViewModel()

    var body: some View {
        VStack {
            Text(viewModel.response)
            Button("Ask") {
                Task {
                    await viewModel.ask("What is Swift?")
                }
            }
        }
    }
}
```

## See Also

- [Cross-Language Overview](README.md) — All bindings
- [Python Bindings](python.md) — Python guide
