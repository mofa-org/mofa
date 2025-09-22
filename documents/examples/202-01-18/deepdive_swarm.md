# 1. Swarm HandOff & Routines 的本质与实现

---
### **1. Routines（任务流程）**

#### **1.1 核心逻辑**


Routines 的核心是**将复杂任务拆解为明确步骤**，让 llm 按步骤执行。这种设计让任务执行更可控、可预测，同时支持动态调整。例如，客户突然不想购买时，llm 可以直接跳到退款流程，无需从头开始。

#### **1.2 案例中的具体体现**

- **Sales Agent 的 Routine**：
  ```python
  sales_agent = Agent(
      name="Sales Agent",
      instructions=(
          "You are a sales agent for ACME Inc."
          "Always answer in a sentence or less."
          "Follow the following routine with the user:"
          "1. Ask them about any problems in their life related to catching roadrunners.\n"
          "2. Casually mention one of ACME's crazy made-up products can help.\n"
          " - Don't mention price.\n"
          "3. Once the user is bought in, drop a ridiculous price.\n"
          "4. Only after everything, and if the user says yes, "
          "tell them a crazy caveat and execute their order.\n"
      ),
      tools=[execute_order, transfer_back_to_triage],
  )
  ```

  - **具体步骤**：
    1. **问问题**：先问客户有没有问题。
    2. **推销产品**：提到 ACME 的产品能帮忙，但先不提价格。
    3. **报价**：等客户感兴趣了，再报一个离谱的价格。
    4. **成交**：如果客户同意，最后再告诉客户一个附加条件，然后完成订单。

- **Issues and Repairs Agent 的 Routine**：
  ```python
  issues_and_repairs_agent = Agent(
      name="Issues and Repairs Agent",
      instructions=(
          "You are a customer support agent for ACME Inc."
          "Always answer in a sentence or less."
          "Follow the following routine with the user:"
          "1. First, ask probing questions and understand the user's problem deeper.\n"
          " - unless the user has already provided a reason.\n"
          "2. Propose a fix (make one up).\n"
          "3. ONLY if not satesfied, offer a refund.\n"
          "4. If accepted, search for the ID and then execute refund."
      ),
      tools=[execute_refund, look_up_item, transfer_back_to_triage],
  )
  ```

  - **具体步骤**：
    1. **问问题**：先问客户具体遇到了什么问题。
    2. **提供解决方案**：给一个解决方案。
    3. **退款**：如果客户不满意，就提供退款。
    4. **执行退款**：如果客户同意退款，就查找商品 ID 并执行退款。

#### **1.3 可视化表示**

`````mermaid
graph TD
    A[任务开始] --> B[问问题]
    B --> C{客户感兴趣吗?}
    C -->|是| D[推销产品]
    C -->|否| E[结束]
    D --> F{客户想买吗?}
    F -->|是| G[报价]
    F -->|否| E
    G --> H{客户同意吗?}
    H -->|是| I[成交]
    H -->|否| E
    I --> J[任务完成]
`````






---
### **2. Handoffs（任务交接）**

#### **2.1 核心逻辑**

Handoffs 的核心在于任务在不同 Agent 之间的流动机制。通过创建 Agent-Tool，LLM 理解任务后调用 `func_call`，间接地调用不同的 Agent 来完成不同的任务。这种设计避免了直接硬编码任务交接逻辑，而是通过 LLM 的智能判断来实现动态的任务分配。

#### **2.2 实现方式**

- **Tool 的使用**：
  - **Tool 的定义**：每个 Tool 都是一个 Python 函数，用于将任务从一个 Agent 转移到另一个 Agent。
  - **Tool 的调用**：LLM 根据当前任务选择合适的 Tool 并调用。

- **代码中的 Handoffs**：
  - **Sales Agent 的 Handoffs**：
    ```python
    tools=[execute_order, transfer_back_to_triage]
    ```

  - **Issues and Repairs Agent 的 Handoffs**：
    ```python
    tools=[execute_refund, look_up_item, transfer_back_to_triage]
    ```

  - **Tool 的实现**：
    ```python
    def transfer_to_sales_agent():
        """User for anything sales or buying related."""
        return sales_agent

    def transfer_to_issues_and_repairs():
        """User for issues, repairs, or refunds."""
        return issues_and_repairs_agent

    def transfer_back_to_triage():
        """Call this if the user brings up a topic outside of your purview,
        including escalating to human."""
        return triage_agent
    ```

#### **2.3 优点**

func.__doc__ 读取函数中的注释说明
整体流程不是特别清楚 需要写一个整体流程的

- **灵活性**：通过 Tool 的使用，任务交接更加灵活，可以根据实时情况动态调整。
- **解耦**：任务交接逻辑与具体 Agent 实现解耦，提高了系统的可维护性和可扩展性。
- **智能判断**：LLM 根据当前任务智能选择合适的 Tool，避免了硬编码的局限性。

#### **2.4 mermaid-graph**

`````mermaid
graph TD
    A[用户输入] --> B{当前Agent}
    B -->|分析输入| C[LLM处理]
    C --> D{是否需要调用Tool?}
    D -->|是| E[选择合适Tool]
    D -->|否| F[直接生成响应]
    E --> G[解析Tool调用]
    G --> H[执行对应函数]
    H --> I{返回结果类型}
    I -->|新Agent| J[切换当前Agent]
    I -->|普通结果| K[将结果加入消息历史]
    J --> L[继续对话]
    K --> L
    F --> L
    L --> M[输出响应]
    M --> N{继续对话?}
    N -->|是| A
    N -->|否| O[结束流程]
`````

---

### **3. 结论**

Handoffs 的设计是为了实现任务的高效交接和状态管理。通过 Tool 的使用，可以避免在调用过程中需要知道相关 Agent 的内容，完全通过 LLM 去做判断和调用。





## **3. Mofa吸取的经验**
1. 在创建Agent时，建议遵循以下最佳实践：
   - 使用`Routines`规范设计prompt模板，确保指令清晰、结构统一
   - 在prompt中明确Agent的职责边界和响应格式要求

2. 为每个Agent创建专用的`Agent-Tool`调用接口：
   - 在函数文档中详细说明Agent的功能定位、输入输出格式


3. 构建智能任务分配Agent：
   - 创建`Scheduler-Agent`作为中央调度器
   - 通过`Handoffs`机制实现任务自动分配
   - 基于任务类型和Agent能力进行智能匹配
   - 支持动态添加新Agent，保持系统可扩展性

