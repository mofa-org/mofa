# 术语表

MoFA 中使用的关键术语和概念。

## A

### Agent（智能体）
处理输入并产生输出的软件组件，通常使用 LLM。智能体实现 `MoFAAgent` trait。

### AgentContext（智能体上下文）
执行期间提供给智能体的执行上下文，包含元数据、会话信息和共享状态。

### AgentCapabilities（智能体能力）
描述智能体能做什么的元数据，包括标签、输入/输出类型和并发限制。

### AgentInput（智能体输入）
发送给智能体的输入数据包装类型。可以包含文本、结构化数据或二进制内容。

### AgentOutput（智能体输出）
智能体产生的输出数据包装类型，包括结果和元数据。

### AgentState（智能体状态）
智能体的当前生命周期状态: Created、Ready、Executing、Paused、Error 或 Shutdown。

## C

### Coordinator（协调器）
使用共识、辩论或并行执行等模式管理多个智能体之间通信的组件。

## F

### Foundation Layer（基础层）
`mofa-foundation` crate，包含内核 trait 的具体实现、业务逻辑和集成。

## K

### Kernel（内核）
`mofa-kernel` crate，提供核心抽象、trait 和基本类型。不包含业务逻辑或实现。

## L

### LLMClient（LLM 客户端）
LLM 提供商的客户端包装器，提供统一的文本生成接口。

### LLMProvider（LLM 提供商）
定义 LLM 提供商（OpenAI、Anthropic 等）接口的 trait。

## M

### Microkernel（微内核）
一种架构模式，核心提供最小功能，所有其他功能作为插件实现。

### MoFAAgent
所有智能体必须实现的核心 trait，定义身份、能力、状态和生命周期方法。

## P

### Plugin（插件）
为 MoFA 添加功能的扩展。可以是编译时（Rust/WASM）或运行时（Rhai 脚本）。

### Persistence（持久化）
保存和恢复智能体状态、会话数据和对话历史的能力。

## R

### ReAct
一种结合推理（Reasoning）和行动（Acting）的模式，智能体在思考和采取行动之间交替。

### Rhai
一种嵌入式脚本语言，用于 MoFA 中的运行时插件。

### Runtime（运行时）
`mofa-runtime` crate，管理智能体生命周期、执行和事件路由。

## S

### Secretary Agent（秘书智能体）
一种特殊的智能体模式，协调任务、管理待办事项，并将关键决策路由给人类。

### SDK
`mofa-sdk` crate，提供统一的公共 API，重新导出所有层的功能。

### StateGraph（状态图）
一种工作流抽象，表示状态（节点）和转换（边）的有向图。

## T

### Tool（工具）
智能体可以用来与外部系统交互或执行操作的可调用函数。

### ToolRegistry（工具注册表）
管理可用工具的注册表，允许注册、发现和执行。

## W

### Workflow（工作流）
智能体执行的有编排的序列，可能包含分支、并行性和状态管理。

### WASM
WebAssembly 模块，可以作为编译时插件加载，实现跨语言兼容性。
