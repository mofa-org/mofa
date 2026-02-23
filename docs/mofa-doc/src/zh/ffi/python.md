# Python 绑定

使用具有原生体验 API 的 Python 调用 MoFA。

## 安装

```bash
pip install mofa
```

或使用原生 PyO3 绑定:

```bash
pip install mofa-native
```

## 快速开始

```python
import os
from mofa import LLMClient, AgentInput, AgentRunner

# 配置 LLM
os.environ["OPENAI_API_KEY"] = "sk-..."

# 创建客户端
client = LLMClient.from_env()

# 简单查询
response = client.ask("什么是 Rust?")
print(response)

# 带系统提示
response = client.ask_with_system(
    system="你是一个 Rust 专家。",
    prompt="解释所有权。"
)
print(response)
```

## 智能体实现

```python
from mofa import MoFAAgent, AgentContext, AgentInput, AgentOutput, AgentState

class MyAgent(MoFAAgent):
    def __init__(self, client):
        self.client = client
        self._state = AgentState.CREATED

    @property
    def id(self):
        return "my-agent"

    @property
    def name(self):
        return "My Agent"

    async def initialize(self, ctx: AgentContext):
        self._state = AgentState.READY

    async def execute(self, input: AgentInput, ctx: AgentContext) -> AgentOutput:
        self._state = AgentState.EXECUTING
        response = await self.client.ask(input.to_text())
        self._state = AgentState.READY
        return AgentOutput.text(response)

    async def shutdown(self):
        self._state = AgentState.SHUTDOWN

    @property
    def state(self):
        return self._state
```

## 使用 AgentRunner

```python
import asyncio
from mofa import AgentRunner, AgentInput

async def main():
    agent = MyAgent(LLMClient.from_env())
    runner = await AgentRunner.new(agent)

    output = await runner.execute(AgentInput.text("你好!"))
    print(output.as_text())

    await runner.shutdown()

asyncio.run(main())
```

## 流式传输

```python
async def stream_example():
    client = LLMClient.from_env()

    async for chunk in client.stream("讲个故事"):
        print(chunk, end="", flush=True)
```

## 工具

```python
from mofa import Tool, ToolError
import json

class CalculatorTool(Tool):
    @property
    def name(self):
        return "calculator"

    @property
    def description(self):
        return "执行算术运算"

    @property
    def parameters_schema(self):
        return {
            "type": "object",
            "properties": {
                "operation": {"type": "string"},
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["operation", "a", "b"]
        }

    async def execute(self, params):
        op = params["operation"]
        a, b = params["a"], params["b"]

        if op == "add":
            return {"result": a + b}
        elif op == "multiply":
            return {"result": a * b}
        else:
            raise ToolError(f"未知操作: {op}")
```

## ReAct 智能体

```python
from mofa import ReActAgent, SimpleToolRegistry

# 注册工具
registry = SimpleToolRegistry()
registry.register(CalculatorTool())

# 创建智能体
agent = ReActAgent.builder() \
    .with_llm(LLMClient.from_env()) \
    .with_tools(registry) \
    .build()

# 执行
output = await agent.execute(
    AgentInput.text("25 乘以 4 是多少?"),
    AgentContext.new("exec-001")
)
print(output.as_text())
```

## 异步支持

MoFA Python 绑定完全支持异步:

```python
import asyncio
from mofa import LLMClient

async def process_multiple(client, queries):
    tasks = [client.ask(q) for q in queries]
    return await asyncio.gather(*tasks)

async def main():
    client = LLMClient.from_env()
    results = await process_multiple(client, [
        "什么是 Rust?",
        "什么是 Python?",
        "什么是 Go?"
    ])
    for r in results:
        print(r)

asyncio.run(main())
```

## 错误处理

```python
from mofa import AgentError, LLMError

try:
    response = await client.ask("你好")
except LLMError.RateLimited as e:
    print(f"速率限制。{e.retry_after}秒后重试")
except LLMError.InvalidApiKey:
    print("检查您的 API 密钥")
except AgentError.ExecutionFailed as e:
    print(f"执行失败: {e}")
```

## 另见

- [跨语言概述](README.md) — 所有绑定
- [安装](../getting-started/installation.md) — 设置指南
