
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

2. **AgentRunner**:
   - 继承自BaseAgentRunner，一个协调任务执行、管理任务状态和步骤的类。它可以创建任务、运行任务以及管理任务，所有的Agent应该都继承它（作为基类）

3. **BaseAgentRunner**:
   - 提供核心的任务和步骤管理功能。（不实现，只提供需要实现的功能）
   - 供子类继承和扩展。

4. **BaseAgent**:
   - 继承自BaseChatEngine和BaseQueryEngine，只实现一些内部的私有函数(例如_query,prompt等)。

5. **BaseChatEngine**:
   - 实现关于Agent-chat的管理(例如stream-chat/chat_history)等.用于管理agent对话中的一些功能
  
6. **BaseQueryEngine**:
   - 用于管理处理query相关的功能
   - 包含ChainableMixin、PromptMixin和DispatcherSpanMixin混入类。

#### 混入类与附加组件

- **DispatcherSpanMixin**:
  - 是一个用于性能监控和追踪的混合类，通过装饰器模式为子类的方法自动添加追踪功能


- **ChainableMixin**:
  - 是一个用于支持链式调用的混合类（Mixin），它通过提供链式操作的能力，使得多个方法可以连续调用，从而简化代码并提高可读性。以下是其核心逻辑和作用的详细分析：

- **PromptMixin**:
  - PromptMixin 是一个用于管理提示词（Prompts）的混合类（Mixin），它提供了一套机制来获取、更新和验证提示词，使得提示词的管理更加灵活和模块化
  - 提示词的相关功能

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



### **如何基于 `BaseToolSpec` 创建自定义 Tool**

`ArxivToolSpec` 是一个典型的基于 `BaseToolSpec` 的工具类实现。通过分析它的实现，我们可以总结出一个通用的模板，用于创建自定义的工具类。以下是创建自定义 Tool 的步骤和关键点：

---

### **1. 核心步骤**

#### **(1) 继承 `BaseToolSpec`**
- 自定义工具类必须继承 `BaseToolSpec`。
- 示例：
  ```python
  from llama_index.core.tools.tool_spec.base import BaseToolSpec

  class CustomToolSpec(BaseToolSpec):
      pass
  ```

#### **(2) 定义工具函数列表**
- 在类中定义 `spec_functions`，列出所有需要暴露的工具函数名称。
- 示例：
  ```python
  class CustomToolSpec(BaseToolSpec):
      spec_functions = ["custom_function_1", "custom_function_2"]
  ```

#### **(3) 实现工具函数**
- 实现 `spec_functions` 中列出的工具函数。
- 每个工具函数需要：
  - 定义清晰的输入参数和返回值。
  - 提供详细的文档字符串（docstring），说明函数的功能、参数和返回值。
- 示例：
  ```python
  def custom_function_1(self, param1: str, param2: int) -> List[Document]:
      """
      A tool to perform custom_function_1.

      Args:
          param1 (str): Description of param1.
          param2 (int): Description of param2.

      Returns:
          List[Document]: A list of Document objects.
      """
      # Function logic here
      pass
  ```

### **2. 关键设计原则**

#### **(1) 函数命名清晰**
- 工具函数的名称应清晰表达其功能。
- 示例：`arxiv_query` 明确表示这是一个用于查询 arXiv 的函数。

#### **(2) 参数设计合理**
- 工具函数的参数应尽量简单，避免过于复杂的输入。
- 示例：`arxiv_query` 只需要 `query` 和 `sort_by` 两个参数。

#### **(3) 返回值标准化**
- 工具函数的返回值应尽量标准化，通常返回 `Document` 对象或字典。
- 示例：`arxiv_query` 返回 `List[Document]`，每个 `Document` 包含论文的 PDF 链接、标题和摘要。

#### **(4) 文档字符串完整**
- 每个工具函数应提供完整的文档字符串，说明函数的功能、参数和返回值。
- 示例：
  ```python
  def custom_function_1(self, param1: str, param2: int) -> List[Document]:
      """
      A tool to perform custom_function_1.

      Args:
          param1 (str): Description of param1.
          param2 (int): Description of param2.

      Returns:
          List[Document]: A list of Document objects.
      """
  ```

---

### **5. 总结**

基于 `BaseToolSpec` 创建自定义 Tool 的核心步骤包括：
1. **继承 `BaseToolSpec`**：定义工具类。
2. **定义工具函数列表**：在 `spec_functions` 中列出所有工具函数。
3. **实现工具函数**：编写具体的工具函数逻辑。
4. **初始化参数**：在 `__init__` 中定义工具类的初始化参数。


