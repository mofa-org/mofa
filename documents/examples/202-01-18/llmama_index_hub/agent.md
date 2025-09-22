
---


### **1. 核心类关系图**

Openai-Agent-Class-Mermaid
````mermaid
graph TD
    A[OpenAIAgent] --> B[AgentRunner]
    B --> C[BaseAgentRunner]
    C --> D[BaseAgent]
    D --> G[BaseChatEngine]
    D --> F[BaseQueryEngine]
    F --> H[DispatcherSpanMixin]
    F --> I[ChainableMixin]
    F --> J[PromptMixin]
    G --> H
````
---
Tools-Class-Mermaid
````mermaid
graph TD
    A[ArxivToolSpec] --> B[BaseToolSpec]

````
---

### **2. 类的功能和依赖说明**


### 类层次结构与功能总结


#### 类层次结构

1. **OpenAIAgent**:
   - 负责协调代理的工具交互和LLM（大型语言模型）交互。
   - 管理工具、LLM交互、记忆和会话前缀。
   - 支持详细模式，并限制函数调用次数。

2. **AgentRunner**:
   - 继承自BaseAgentRunner，负责任务的生命周期管理和步骤管理。
   - 根据基类提供相关的功能,例如memory/prompt等

3. **BaseAgentRunner**:
   - 提供核心的任务和步骤管理功能。
   - 供子类继承和扩展。

4. **BaseAgent**:
   - 继承自BaseChatEngine和BaseQueryEngine，结合聊天和查询功能，用于代理的实现。
   - 处理聊天和查询操作。

5. **BaseChatEngine**:
   - 管理聊天交互和响应。
   - 与查询引擎集成。

6. **BaseQueryEngine**:
   - 管理查询处理和响应。
   - 包含ChainableMixin、PromptMixin和DispatcherSpanMixin混入类。

#### 混入类与附加组件

- **DispatcherSpanMixin**:
  - 提供性能监控和日志记录功能。
  - 跟踪方法调用和步骤，以便监控和日志记录。

- **ChainableMixin**:
  - 支持查询组件的链式处理。
  - 允许灵活和动态的查询管道。

- **PromptMixin**:
  - 管理提示模板和模块。
  - 确保代理系统中提示的无缝集成。

#### 交互与工作流程

- **用户交互**:
  - 用户与OpenAIAgent交互，它协调整体流程。
  - 使用AgentRunner管理任务和步骤。

- **任务与步骤管理**:
  - AgentRunner负责任务的创建、执行和完成。
  - BaseAgentRunner提供基础的任务管理功能。

- **聊天与查询处理**:
  - BaseAgent结合聊天和查询功能。
  - BaseChatEngine和BaseQueryEngine处理特定的交互和查询。

#### 设计优点

- **模块化**:
  - 易于扩展和定制以满足不同需求。
  - 组件可以替换或修改而不影响整个系统。

- **可扩展性**:
  - 支持添加新的工具、规划功能和查询处理方法。

- **性能监控**:
  - DispatcherSpanMixin允许高效地跟踪和记录方法调用。

#### 示例用例

- **场景**:
  - 用户要求代理执行涉及多个步骤和工具的复杂任务。

- **流程**:
  - OpenAIAgent协调任务，使用AgentRunner管理各个步骤。
  - BasePlanningAgentRunner优化任务计划，确保高效执行。
  - BaseAgent结合聊天和查询功能与用户交互并执行任务。


### **3. 详细类说明**

---

## **类功能说明与主要属性**

### **1. 核心类**

### OpenAIAgent 参数与功能说明

| 参数名称 | 类型 | 默认值 | 说明 |
|----------|------|--------|------|
| `tools` | List[BaseTool] | 无 | 代理可使用的工具列表，每个工具必须继承BaseTool |
| `llm` | OpenAI | 无 | 必须是一个OpenAI实例，用于处理语言模型相关操作 |
| `memory` | BaseMemory | 无 | 负责存储和管理对话历史 |
| `prefix_messages` | List[ChatMessage] | 无 | 在对话开始前预置的消息列表 |
| `verbose` | bool | False | 是否启用详细输出模式 |
| `max_function_calls` | int | DEFAULT_MAX_FUNCTION_CALLS | 限制代理在单次对话中调用工具的最大次数 |
| `default_tool_choice` | str | "auto" | 控制代理如何选择工具："auto"表示自动选择 |
| `callback_manager` | Optional[CallbackManager] | None | 用于管理回调函数 |
| `tool_retriever` | Optional[ObjectRetriever[BaseTool]] | None | 用于动态检索工具 |
| `tool_call_parser` | Optional[Callable[[OpenAIToolCall], Dict]] | None | 自定义工具调用解析器 |

### 主要功能说明

1. **工具管理**
   - 支持多个工具集成
   - 提供工具的动态选择机制
   - 支持工具调用解析

2. **对话管理**
   - 内置记忆系统，支持多轮对话
   - 可预置对话消息
   - 支持详细的调试输出

3. **执行控制**
   - 限制工具调用次数
   - 提供工具选择策略
   - 支持回调机制

4. **扩展性**
   - 支持自定义工具检索器
   - 允许自定义工具调用解析
   - 可扩展的记忆系统


我理解您希望简洁地了解`AgentRunner`和`BasePlanningAgentRunner`这两个类的核心功能和属性。以下是它们的核心信息：

### 1. AgentRunner 类

**核心功能**：
- 任务管理：创建、删除、列出和获取任务
- 步骤管理：获取即将执行的步骤和已完成步骤
- 对话处理：支持同步和异步的聊天交互
- 工具选择：支持自定义工具选择策略
- 任务执行：管理任务的执行流程

**主要属性/方法**：
- `create_task()`: 创建新任务
- `delete_task()`: 删除指定任务
- `list_tasks()`: 列出所有任务
- `get_task()`: 获取特定任务
- `get_upcoming_steps()`: 获取即将执行的步骤
- `get_completed_steps()`: 获取已完成步骤
- `chat()/achat()`: 同步/异步聊天接口
- `stream_chat()/astream_chat()`: 同步/异步流式聊天接口

### 2. BasePlanningAgentRunner 类

**核心功能**：
- 继承自AgentRunner
- 添加了任务规划能力
- 支持多任务并行执行
- 提供任务计划优化功能

**主要属性/方法**：
- `create_plan()`: 创建新的任务计划
- `get_next_tasks()`: 获取下一个待执行任务
- `mark_task_complete()`: 标记任务完成
- `arefine_plan()`: 异步优化任务计划
- 继承了AgentRunner的所有基础功能

这两个类共同构成了一个完整的任务执行框架，其中BasePlanningAgentRunner在AgentRunner的基础上增加了更复杂的任务规划能力。




### BaseAgentRunner 类

| 功能类别 | 方法/属性 | 说明 |
|----------|-----------|------|
| **任务管理** | `create_task()` | 创建新任务 |
|           | `delete_task()` | 删除指定任务 |
|           | `list_tasks()` | 列出所有任务 |
|           | `get_task()` | 获取特定任务 |
|           | `get_completed_tasks()` | 获取已完成的任务 |
| **步骤管理** | `get_upcoming_steps()` | 获取即将执行的步骤 |
|           | `get_completed_steps()` | 获取已完成步骤 |
|           | `get_completed_step()` | 获取特定步骤的输出 |
| **任务执行** | `run_step()/arun_step()` | 同步/异步运行任务步骤 |
|           | `stream_step()/astream_step()` | 同步/异步流式运行任务步骤 |
| **响应处理** | `finalize_response()` | 最终化任务响应 |
| **任务回退** | `undo_step()` | 回退上一步操作 |

---

### 详细说明

1. **任务管理**：
   - 提供任务的全生命周期管理，包括创建、删除、列出和获取任务。
   - 支持获取已完成的任务列表。

2. **步骤管理**：
   - 支持获取即将执行的步骤和已完成步骤。
   - 提供获取特定步骤输出的功能。

3. **任务执行**：
   - 支持同步和异步的任务步骤执行。
   - 提供流式执行任务步骤的能力。

4. **响应处理**：
   - 提供最终化任务响应的功能，用于生成最终的输出结果。

5. **任务回退**：
   - 支持回退上一步操作，用于撤销错误的步骤。

---

`BaseAgentRunner` 是一个基础的任务执行框架，提供了任务和步骤管理的核心功能，是 `AgentRunner` 和 `BasePlanningAgentRunner` 的基类。







### 1. **BaseAgent 类**

**功能**：
- 基础代理类，继承自 `BaseChatEngine` 和 `BaseQueryEngine`
- 提供查询引擎接口和聊天引擎接口

**主要方法**：
- `_get_prompts()`: 获取提示词
- `_get_prompt_modules()`: 获取提示模块
- `_update_prompts()`: 更新提示词
- `_query()/_aquery()`: 同步/异步查询接口
- `stream_chat()/astream_chat()`: 同步/异步流式聊天接口（未实现）

---

### 2. **TaskStep 类**

**功能**：
- 表示代理任务中的一个步骤
- 管理步骤的状态和依赖关系

**主要属性**：
- `task_id`: 任务ID
- `step_id`: 步骤ID
- `input`: 用户输入
- `step_state`: 步骤状态
- `next_steps`: 下一步骤
- `prev_steps`: 上一步骤
- `is_ready`: 步骤是否准备好执行

**主要方法**：
- `get_next_step()`: 获取下一个步骤
- `link_step()`: 链接到下一个步骤

---

### 3. **TaskStepOutput 类**

**功能**：
- 表示任务步骤的输出

**主要属性**：
- `output`: 步骤输出
- `task_step`: 任务步骤
- `next_steps`: 下一步骤列表
- `is_last`: 是否是最后一个步骤

**主要方法**：
- `__str__()`: 返回输出的字符串表示

---

### 4. **Task 类**

**功能**：
- 表示代理任务的一个执行实例

**主要属性**：
- `task_id`: 任务ID
- `input`: 用户输入
- `memory`: 对话记忆
- `callback_manager`: 回调管理器
- `extra_state`: 额外状态

---

### 5. **BaseAgentWorker 类**

**功能**：
- 基础代理工作类，继承自 `PromptMixin` 和 `DispatcherSpanMixin`
- 提供任务步骤的执行和管理

**主要方法**：
- `_get_prompts()`: 获取提示词
- `_get_prompt_modules()`: 获取提示模块
- `_update_prompts()`: 更新提示词
- `initialize_step()`: 初始化步骤
- `run_step()/arun_step()`: 同步/异步运行步骤
- `stream_step()/astream_step()`: 同步/异步流式运行步骤
- `finalize_task()`: 最终化任务
- `set_callback_manager()`: 设置回调管理器
- `as_agent()`: 返回一个代理运行器实例

---


我将按照模块和功能分类，整理这两个文件的核心内容：

---

### 1. **Chat Engine 相关类型**

#### (1) **AgentChatResponse**
- **功能**：表示代理的聊天响应
- **主要属性**：
  - `response`: 响应内容
  - `sources`: 工具输出列表
  - `source_nodes`: 带分数的节点列表
  - `is_dummy_stream`: 是否为模拟流式响应
- **主要方法**：
  - `set_source_nodes()`: 设置源节点
  - `response_gen`: 生成模拟流式响应
  - `async_response_gen`: 异步生成模拟流式响应

#### (2) **StreamingAgentChatResponse**
- **功能**：处理流式聊天响应
- **主要属性**：
  - `chat_stream/achat_stream`: 同步/异步聊天流
  - `queue/aqueue`: 同步/异步队列
  - `is_function`: 是否为函数调用
  - `is_done`: 是否完成
- **主要方法**：
  - `write_response_to_history()`: 将响应写入历史记录
  - `awrite_response_to_history()`: 异步写入历史记录
  - `response_gen/async_response_gen`: 生成流式响应
  - `print_response_stream/aprint_response_stream`: 打印流式响应

#### (3) **ChatMode**
- **功能**：定义聊天引擎的模式
- **模式**：
  - `SIMPLE`: 简单聊天模式
  - `CONDENSE_QUESTION`: 问题浓缩模式
  - `CONTEXT`: 上下文模式
  - `CONDENSE_PLUS_CONTEXT`: 浓缩加上下文模式
  - `REACT`: ReAct 代理模式
  - `OPENAI`: OpenAI 函数调用模式
  - `BEST`: 自动选择最佳模式

---

### 2. **Instrumentation 模块**

#### (1) **DispatcherSpanMixin**
- **功能**：自动为子类方法添加 span 追踪
- **主要特性**：
  - 自动为抽象方法实现添加追踪
  - 自动为被覆盖的已追踪方法添加追踪
  - 支持跨类继承的追踪

#### (2) **Dispatcher 系统**
- **核心组件**：
  - `root_dispatcher`: 根调度器
  - `root_manager`: 根管理器
- **主要功能**：
  - `get_dispatcher()`: 获取或创建调度器
  - 支持层级结构的调度器管理

---

### 3. **工具函数**

#### (1) **is_function**
- **功能**：判断消息是否为函数调用
- **实现**：检查消息的 `additional_kwargs` 中是否包含 `tool_calls`

---

### 总结

这两个文件主要定义了聊天引擎的核心数据类型和工具类：

1. **chat_engine/types.py** 定义了聊天响应的数据结构、流式处理机制和聊天模式枚举
2. **instrumentation/__init__.py** 提供了性能监控和追踪的基础设施

这些组件共同构成了聊天引擎的核心功能框架，支持同步/异步处理、流式响应、上下文管理等功能。

---
好的，我来整理并说明这个文件的核心内容。这个文件定义了查询引擎的基础类和组件，以下是详细说明：

---

### 1. **BaseQueryEngine 类**

**功能**：
- 基础查询引擎类，继承自 `ChainableMixin`, `PromptMixin`, `DispatcherSpanMixin`
- 提供同步和异步的查询接口
- 支持回调管理和追踪

**主要方法**：
- `query()`: 同步查询接口
- `aquery()`: 异步查询接口
- `retrieve()`: 检索节点（未实现）
- `synthesize()/asynthesize()`: 合成响应（未实现）
- `_query()/_aquery()`: 抽象方法，子类需实现具体的查询逻辑

**主要属性**：
- `callback_manager`: 回调管理器，用于处理查询过程中的回调事件

**事件追踪**：
- 使用 `dispatcher.span` 装饰器追踪查询过程
- 触发 `QueryStartEvent` 和 `QueryEndEvent` 事件

---

### 2. **QueryEngineComponent 类**

**功能**：
- 将查询引擎封装为查询组件，用于查询管道
- 继承自 `QueryComponent`

**主要方法**：
- `set_callback_manager()`: 设置回调管理器
- `_validate_component_inputs()`: 验证组件输入
- `_run_component()/_arun_component()`: 运行组件（同步/异步）
- `input_keys/output_keys`: 定义组件的输入输出键

**主要属性**：
- `query_engine`: 封装的查询引擎实例

---

### 3. **核心功能**

#### (1) **查询处理**
- 支持字符串或 `QueryBundle` 作为输入
- 自动将字符串转换为 `QueryBundle`
- 提供同步和异步的查询接口

#### (2) **回调管理**
- 使用 `CallbackManager` 管理查询过程中的回调
- 支持追踪查询的开始和结束事件

#### (3) **组件化**
- 通过 `QueryEngineComponent` 将查询引擎封装为可复用的组件
- 支持在查询管道中使用

---

### 4. **未实现功能**
- `retrieve()`: 检索节点
- `synthesize()/asynthesize()`: 合成响应
- 这些功能需要子类根据具体需求实现

---



我将按照模块和功能分类，整理这两个文件的核心内容：

---

### 1. **Query Pipeline 模块**

#### (1) **StringableInput**
- **功能**：定义可转换为字符串的输入类型
- **包含类型**：
  - `CompletionResponse`
  - `ChatResponse`
  - `ChatMessage`
  - `str`
  - `QueryBundle`
  - `Response`
  - `Generator`
  - `NodeWithScore`
  - `TextNode`

#### (2) **InputKeys/OutputKeys**
- **功能**：管理组件的输入输出键
- **主要方法**：
  - `from_keys()`: 从键集合创建实例
  - `validate_keys()`: 验证键集合
  - `all()`: 获取所有键

#### (3) **ChainableMixin**
- **功能**：将模块转换为查询组件
- **主要方法**：
  - `_as_query_component()`: 抽象方法，子类需实现
  - `as_query_component()`: 获取查询组件

#### (4) **QueryComponent**
- **功能**：查询管道中的组件
- **主要方法**：
  - `partial()`: 更新部分参数
  - `run_component()/arun_component()`: 运行组件（同步/异步）
  - `validate_component_inputs()/validate_component_outputs()`: 验证输入输出
- **主要属性**：
  - `partial_dict`: 部分参数
  - `free_req_input_keys`: 自由输入键

#### (5) **CustomQueryComponent**
- **功能**：自定义查询组件
- **主要特性**：
  - 支持回调管理
  - 提供默认的输入输出验证
  - 支持同步/异步运行

#### (6) **Link**
- **功能**：连接两个组件
- **主要属性**：
  - `src/dest`: 源/目标组件
  - `src_key/dest_key`: 源/目标键
  - `condition_fn`: 条件函数
  - `input_fn`: 输入函数

#### (7) **ComponentIntermediates**
- **功能**：存储组件的中间输入输出
- **主要属性**：
  - `inputs`: 输入
  - `outputs`: 输出

---

### 2. **Prompt Mixin 模块**

#### (1) **PromptMixin**
- **功能**：管理提示词
- **主要方法**：
  - `get_prompts()`: 获取提示词
  - `update_prompts()`: 更新提示词
  - `_validate_prompts()`: 验证提示词
- **主要属性**：
  - `prompts_dict`: 提示词字典
  - `module_dict`: 模块字典

#### (2) **抽象方法**
- `_get_prompts()`: 获取提示词
- `_get_prompt_modules()`: 获取提示模块
- `_update_prompts()`: 更新提示词

---

### 3. **核心功能**

#### (1) **查询管道**
- 支持组件化设计
- 支持同步/异步运行
- 支持输入输出验证
- 支持组件连接

#### (2) **提示词管理**
- 支持分层管理
- 支持子模块提示词
- 支持提示词更新

---

### 4. **未实现功能**
- `_run_component()/_arun_component()`: 组件运行逻辑
- `_get_prompts()/_get_prompt_modules()/_update_prompts()`: 提示词管理逻辑

---

### 总结
这两个文件定义了查询管道和提示词管理的基础框架：
1. **query.py** 提供了查询管道的核心组件和连接机制
2. **mixin.py** 提供了提示词管理的通用接口




------








### 优化后的类层次结构与功能说明

#### 1. 核心类及其功能

##### 1.1 OpenAIAgent

- **功能**: 协调工具和大型语言模型（LLM）的交互，管理对话历史和工具选择。
- **参数**:
  - `tools`: 可用工具列表，每个工具需继承自`BaseTool`。
  - `llm`: 处理语言模型操作的OpenAI实例。
  - `memory`: 管理对话历史的记忆系统。
  - `prefix_messages`: 对话开始前预置的消息列表。
  - `verbose`: 是否启用详细输出模式。
  - `max_function_calls`: 单次对话中工具调用的最大次数。
  - `default_tool_choice`: 工具选择策略，"auto"表示自动选择。
  - `callback_manager`: 管理回调函数。
  - `tool_retriever`: 动态检索工具。
  - `tool_call_parser`: 自定义工具调用解析器。

##### 1.2 AgentRunner

- **功能**: 管理任务生命周期和步骤，提供聊天接口。
- **方法**:
  - `create_task()`, `delete_task()`, `list_tasks()`, `get_task()`: 任务管理。
  - `get_upcoming_steps()`, `get_completed_steps()`: 步骤管理。
  - `chat()/achat()`, `stream_chat()/astream_chat()`: 聊天接口。

##### 1.3 BasePlanningAgentRunner

- **功能**: 扩展`AgentRunner`，增加任务规划和优化功能。
- **方法**:
  - `create_plan()`, `get_next_tasks()`, `mark_task_complete()`, `arefine_plan()`: 任务规划和优化。

##### 1.4 BaseAgentRunner

- **功能**: 提供任务和步骤管理的核心功能。
- **方法**:
  - `create_task()`, `delete_task()`, `list_tasks()`, `get_task()`: 任务管理。
  - `get_upcoming_steps()`, `get_completed_steps()`, `get_completed_step()`: 步骤管理。
  - `run_step()/arun_step()`, `stream_step()/astream_step()`: 任务执行。
  - `finalize_response()`: 响应处理。
  - `undo_step()`: 任务回退。

##### 1.5 BaseAgent

- **功能**: 结合`BaseChatEngine`和`BaseQueryEngine`，处理聊天和查询操作。
- **方法**:
  - `_get_prompts()`, `_get_prompt_modules()`, `_update_prompts()`: 提示管理。
  - `_query()/_aquery()`: 查询接口。
  - `stream_chat()/astream_chat()`: 聊天接口（未实现）。

#### 2. 相关类及其功能

##### 2.1 TaskStep

- **功能**: 管理任务步骤的状态和依赖关系。
- **属性**:
  - `task_id`, `step_id`, `input`, `step_state`, `next_steps`, `prev_steps`, `is_ready`。
- **方法**:
  - `get_next_step()`, `link_step()`: 步骤管理。

##### 2.2 TaskStepOutput

- **功能**: 表示任务步骤的输出。
- **属性**:
  - `output`, `task_step`, `next_steps`, `is_last`。
- **方法**:
  - `__str__()`: 输出字符串表示。

##### 2.3 Task

- **功能**: 表示代理任务的执行实例。
- **属性**:
  - `task_id`, `input`, `memory`, `callback_manager`, `extra_state`。

##### 2.4 BaseAgentWorker

- **功能**: 提供任务步骤的执行和管理。
- **方法**:
  - `_get_prompts()`, `_get_prompt_modules()`, `_update_prompts()`: 提示管理。
  - `initialize_step()`, `run_step()/arun_step()`, `stream_step()/astream_step()`, `finalize_task()`, `set_callback_manager()`, `as_agent()`: 任务执行和管理。

#### 3. 模块和工具函数

##### 3.1 Chat Engine 相关类型

- **AgentChatResponse**: 表示代理的聊天响应，包含响应内容和工具输出列表。
- **StreamingAgentChatResponse**: 处理流式聊天响应，支持同步和异步响应生成。
- **ChatMode**: 定义聊天引擎的模式，包括简单模式、问题浓缩模式、上下文模式等。

##### 3.2 Instrumentation 模块

- **DispatcherSpanMixin**: 自动为子类方法添加性能监控和追踪。
- **Dispatcher 系统**: 管理调度器和事件处理，支持层级结构的调度器管理。

##### 3.3 工具函数

- **is_function**: 判断消息是否为函数调用，检查`additional_kwargs`中是否包含`tool_calls`。

#### 4. 查询引擎和提示词管理

##### 4.1 Query Pipeline 模块

- **StringableInput**: 定义可转换为字符串的输入类型。
- **InputKeys/OutputKeys**: 管理组件的输入输出键。
- **ChainableMixin**: 将模块转换为查询组件。
- **QueryComponent**: 查询管道中的组件，支持部分参数更新和输入输出验证。
- **CustomQueryComponent**: 自定义查询组件，支持回调管理和组件运行。
- **Link**: 连接两个组件，定义源/目标键和条件函数。
- **ComponentIntermediates**: 存储组件的中间输入输出。

##### 4.2 Prompt Mixin 模块

- **PromptMixin**: 管理提示词，支持提示词的获取、更新和验证。
- **抽象方法**: `_get_prompts()`, `_get_prompt_modules()`, `_update_prompts()`需在子类中实现。

#### 5. 总结

- **chat_engine/types.py**: 定义聊天响应的数据结构和聊天模式枚举。
- **instrumentation/__init__.py**: 提供性能监控和追踪的基础设施。
- **query.py** 和 **mixin.py**: 定义查询管道和提示词管理的基础框架，支持组件化设计和提示词的分层管理。


# Oenai-Agent 运行说明
0. 使用from_tools的时候，会调用Llmama-index中的FunctionTool这个类，这个类里面会自动的变成FunctionTool的class，并且返回
1. 首先由于底层的AgentRunner调用了Mixin的方法,所以他们都被wrapper装饰起来.都变成了一个decorator的函数
2. 调用的都是BaseAgentRunner中的chat->_chat
3. 在_chat这个函数中，主要会调用一个task的函数，根据刚才实例话过后的对象，提取里面的内容，通过pydantic创建，并且使用TaskState的类，将任务放到里面，相当于创建了一个任务队列
4. 这个时候会进入到一个死循环中,直到认为是最后一个任务后，再结束
5. 每个循环都会调用AgentRunner._run_step()这个主要的函数
6. 在run_step这个函数中，主要会调用agent_worker.run_step这个函数.在这个函数中,首先会获取tools,会根据创建好的tool类中的内容，获取tool的参数以及函数名称，然后转换成openai的dict形式的tool(调用to_openai_tool)
7. 调用_get_agent_response，然后这个函数中再调用chat()函数,然后调用wrapped_llm_chat这个装饰器函数。
8. 调用这个函数的过程中,会调用openai包中的OpenAI类(这个类继承了FunctionCallingLLM)，这个时候会存在一个tool_call的变量
9. 调用OpenAiAgentWorker中的_call_function函数,首先把tool的函数变成openai的tool函数.然后调用get_latest_tool_calls这个函数,把tool需要的参数提取出来,然后去调用_call_function函数，并且将这个结果包存agent_chat_response.sources这个属性中
10. 最后结束整个流程

