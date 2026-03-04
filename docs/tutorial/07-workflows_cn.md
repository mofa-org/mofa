# 第 7 章：StateGraph 工作流

> **学习目标：** 理解基于图的工作流，使用 `NodeFunc` 实现节点，定义边和条件路由，使用 reducer 进行状态管理，并构建一个客户支持工作流。

## 为什么需要工作流？

多智能体协调（第 6 章）处理任务委派。但对于具有**分支逻辑**、**循环**和**共享状态**的复杂流程呢？这就是工作流的用武之地。

MoFA 的工作流系统受到 [LangGraph](https://github.com/langchain-ai/langgraph) 的启发。它将流程建模为**有向图**：

- **节点**是处理步骤（转换状态的函数）
- **边**定义节点之间的流向（包括条件分支）
- **状态**在图中流动并积累结果

```
           ┌──────────┐
START ───▶ │  分 类    │
           └────┬──────┘
                │
        ┌───────┼───────┐
        ▼       ▼       ▼
    ┌───────┐ ┌───────┐ ┌──────┐
    │ 账单  │ │ 技术  │ │ 通用  │
    └───┬───┘ └───┬───┘ └──┬───┘
        │         │        │
        └─────────┼────────┘
                  ▼
           ┌──────────┐
           │  响 应    │
           └────┬──────┘
                ▼
               END
```

## 核心概念

### GraphState

每个工作流都操作一个**状态**对象。`GraphState` trait 定义了状态的创建、合并和序列化方式：

```rust
// crates/mofa-kernel/src/workflow/graph.rs

pub trait GraphState: Clone + Send + Sync + 'static {
    fn new() -> Self;
    fn merge(&mut self, other: &Self);
    fn to_value(&self) -> serde_json::Value;
    fn from_value(value: serde_json::Value) -> AgentResult<Self>;
}
```

MoFA 提供了 `JsonState` 作为开箱即用的实现：

```rust
use mofa_sdk::workflow::JsonState;

let mut state = JsonState::new();
state.set("customer_query", json!("我无法登录我的账户"));
state.set("category", json!("unknown"));
```

### NodeFunc

图中的每个节点都是一个处理状态的函数：

```rust
#[async_trait]
pub trait NodeFunc<S: GraphState>: Send + Sync {
    async fn call(&self, state: &mut S, ctx: &RuntimeContext) -> AgentResult<Command>;
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str> { None }
}
```

节点接收可变状态，完成工作后返回一个控制流程的 `Command`。

### Command

`Command` 枚举告诉图在节点运行后做什么：

```rust
pub enum Command {
    // 继续到下一个节点（跟随默认边）
    Continue(StateUpdate),

    // 跳转到指定名称的节点
    Goto(String, StateUpdate),

    // 停止工作流并返回当前状态
    Return(StateUpdate),
}
```

`StateUpdate` 携带此节点希望对状态进行的更改。

### Reducer

当多个节点更新同一个状态键时，**reducer** 定义如何合并值：

| Reducer | 行为 | 示例 |
|---------|------|------|
| `AppendReducer` | 添加到列表 | 消息不断累积 |
| `OverwriteReducer` | 替换值 | 状态字段更新 |
| `MergeReducer` | 深度合并 JSON 对象 | 配置逐步累积 |

## 构建：客户支持工作流

让我们构建一个工作流：
1. **分类**客户查询（账单、技术、通用）
2. **路由**到专门的处理程序
3. **响应**格式化答案

创建新项目：

```bash
cargo new support_workflow
cd support_workflow
```

编辑 `Cargo.toml`：

```toml
[package]
name = "support_workflow"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

编写 `src/main.rs`：

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{AgentResult, AgentContext};
use mofa_sdk::workflow::{
    JsonState, StateGraphImpl, Command, ControlFlow,
    RuntimeContext, NodeFunc, START, END,
};
use serde_json::json;

// --- 节点 1：分类查询 ---

struct ClassifyNode;

#[async_trait]
impl NodeFunc<JsonState> for ClassifyNode {
    fn name(&self) -> &str { "classify" }

    fn description(&self) -> Option<&str> {
        Some("将客户查询分类为账单、技术或通用")
    }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("").to_lowercase();

        // 简单的基于关键词的分类
        //（在生产环境中，使用 LLM 进行分类）
        let category = if query.contains("账单") || query.contains("收费")
            || query.contains("付款") || query.contains("发票")
            || query.contains("bill") || query.contains("charge")
        {
            "billing"
        } else if query.contains("错误") || query.contains("bug")
            || query.contains("崩溃") || query.contains("登录")
            || query.contains("error") || query.contains("login")
        {
            "technical"
        } else {
            "general"
        };

        state.set("category", json!(category));
        println!("  [分类] 查询被分类为: {}", category);

        // 使用 Goto 路由到适当的处理程序
        Ok(Command::Goto(
            category.to_string(),
            Default::default(),
        ))
    }
}

// --- 节点 2a：账单处理 ---

struct BillingNode;

#[async_trait]
impl NodeFunc<JsonState> for BillingNode {
    fn name(&self) -> &str { "billing" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("");
        let response = format!(
            "账单支持：我了解您对 '{}' 有账单方面的疑虑。\
             我已调出您的账户，让我检查最近的收费记录。",
            query
        );
        state.set("response", json!(response));
        state.set("department", json!("账单"));
        println!("  [账单] 已处理");
        Ok(Command::Continue(Default::default()))
    }
}

// --- 节点 2b：技术处理 ---

struct TechnicalNode;

#[async_trait]
impl NodeFunc<JsonState> for TechnicalNode {
    fn name(&self) -> &str { "technical" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("");
        let response = format!(
            "技术支持：我看到您遇到了技术问题：'{}'。\
             让我检查系统状态和最近的日志。",
            query
        );
        state.set("response", json!(response));
        state.set("department", json!("技术"));
        println!("  [技术] 已处理");
        Ok(Command::Continue(Default::default()))
    }
}

// --- 节点 2c：通用处理 ---

struct GeneralNode;

#[async_trait]
impl NodeFunc<JsonState> for GeneralNode {
    fn name(&self) -> &str { "general" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let query = state.get_str("query").unwrap_or("");
        let response = format!(
            "通用支持：感谢您就 '{}' 联系我们。\
             我很乐意回答您的任何问题。",
            query
        );
        state.set("response", json!(response));
        state.set("department", json!("通用"));
        println!("  [通用] 已处理");
        Ok(Command::Continue(Default::default()))
    }
}

// --- 节点 3：格式化最终响应 ---

struct RespondNode;

#[async_trait]
impl NodeFunc<JsonState> for RespondNode {
    fn name(&self) -> &str { "respond" }

    async fn call(
        &self,
        state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> AgentResult<Command> {
        let response = state.get_str("response").unwrap_or("未生成响应");
        let department = state.get_str("department").unwrap_or("未知");

        let final_response = format!(
            "--- 客户支持回复 ---\n\
             部门：{}\n\
             {}\n\
             --- 结束 ---",
            department, response
        );

        state.set("final_response", json!(final_response));
        println!("  [响应] 最终回复已格式化");
        Ok(Command::Return(Default::default()))
    }
}

// --- 构建并运行工作流 ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建状态图
    let mut graph = StateGraphImpl::<JsonState>::new("customer_support");

    // 添加节点
    graph.add_node(Box::new(ClassifyNode));
    graph.add_node(Box::new(BillingNode));
    graph.add_node(Box::new(TechnicalNode));
    graph.add_node(Box::new(GeneralNode));
    graph.add_node(Box::new(RespondNode));

    // 定义边
    graph.add_edge(START, "classify");
    graph.add_edge("billing", "respond");
    graph.add_edge("technical", "respond");
    graph.add_edge("general", "respond");

    // 编译图
    let compiled = graph.compile()?;

    // 用不同的查询测试
    let test_queries = vec![
        "我的订阅被收费了两次",
        "我无法登录账户，出现 500 错误",
        "你们的营业时间是什么？",
    ];

    for query in test_queries {
        println!("\n=== 查询: '{}' ===", query);
        let mut state = JsonState::new();
        state.set("query", json!(query));

        let result = compiled.run(state).await?;
        println!("{}", result.get_str("final_response").unwrap_or("无响应"));
    }

    Ok(())
}
```

运行它：

```bash
cargo run
```

## 刚才发生了什么？

1. **图构建**：我们创建了节点并用边连接它们
2. **编译**：`graph.compile()` 验证图（检查缺失的边、不可达的节点）
3. **执行**：对于每个查询：
   - 状态从 `START` 开始，流向 `classify`
   - `ClassifyNode` 使用 `Command::Goto(category)` 路由到正确的处理程序
   - 处理程序处理查询并使用 `Command::Continue` 流向 `respond`
   - `RespondNode` 格式化输出并使用 `Command::Return` 停止

> **架构说明：** `StateGraph` trait 定义在 `mofa-kernel`（`crates/mofa-kernel/src/workflow/graph.rs`），而 `StateGraphImpl` 位于 `mofa-foundation`（`crates/mofa-foundation/src/workflow/state_graph.rs`）。Reducer 在 `crates/mofa-foundation/src/workflow/reducers.rs`。工作流 DSL 解析器（`WorkflowDslParser`）支持用 YAML 定义工作流——参见 `examples/workflow_dsl/src/main.rs` 获取完整示例。

## 工作流 DSL（YAML）

对于复杂的工作流，你可以用 YAML 定义而不是代码：

```yaml
# customer_support.yaml
workflow:
  name: "customer_support"
  nodes:
    - name: classify
      type: llm
      prompt: "分类这个客户查询: {{query}}"
    - name: billing
      type: llm
      prompt: "处理这个账单问题: {{query}}"
    - name: technical
      type: llm
      prompt: "处理这个技术问题: {{query}}"
    - name: respond
      type: llm
      prompt: "为客户格式化最终回复"
  edges:
    - from: START
      to: classify
    - from: classify
      to: [billing, technical]
      condition: "category"
    - from: billing
      to: respond
    - from: technical
      to: respond
```

加载并运行：

```rust
use mofa_sdk::workflow::{WorkflowDslParser, WorkflowExecutor, ExecutorConfig};

let definition = WorkflowDslParser::from_file("customer_support.yaml")?;
let workflow = WorkflowDslParser::build(definition).await?;

let executor = WorkflowExecutor::new(ExecutorConfig::default());
let result = executor.execute(&workflow, input).await?;
```

## 关键要点

- 工作流将流程建模为带有节点、边和共享状态的有向图
- `NodeFunc` 定义每个节点的功能——接收状态，返回 `Command`
- `Command::Continue` 跟随默认边，`Goto` 跳转到指定节点，`Return` 停止
- 条件路由让节点动态决定下一步
- Reducer（`Append`、`Overwrite`、`Merge`）处理并发状态更新
- `StateGraphImpl` 是具体实现，`JsonState` 是默认状态类型
- YAML DSL 可用于声明式定义工作流

---

**下一章：** [第 8 章：插件与脚本](08-plugins-and-scripting_cn.md) — 编写支持热重载的 Rhai 插件。

[← 返回目录](README_cn.md)

---

**English** | [简体中文](../zh-CN/tutorial/07-workflows_cn.md)
