# 多任务Agent的上下文感知工具路由实现

## 核心设计

我们已经成功实现了一个上下文感知的工具路由系统，主要包括以下核心组件：

### 1. 工具定义模块 (`tool_definitions.rs`)
实现了四种示例工具：
- 计算器 (`calculator`): 处理数字计算
- 天气查询 (`weather_query`): 查询城市天气
- 新闻查询 (`news_query`): 获取最新新闻
- 股票查询 (`stock_query`): 查询股票行情

每个工具都定义了明确的名称、描述和参数结构，符合OpenAI工具调用规范。

### 2. 路由规则系统 (`route_rules.rs`)
实现了上下文感知的规则引擎：

#### 核心规则
- 如果用户提及"最近"且涉及事件，自动路由到新闻API
- 如果涉及数字计算，自动路由到计算器
- 如果涉及天气查询，自动路由到天气API

#### 规则管理
- 规则按优先级排序
- 支持动态添加、删除和修改规则
- 基于关键词匹配的上下文分析

### 3. 工具路由插件 (`tool_routing_plugin.rs`)
实现了 `AgentPlugin` 接口，作为微内核的插件运行：

#### 核心功能
- 接收用户输入
- 结合上下文进行路由分析
- 返回匹配的工具名称
- 支持动态规则更新

#### 微内核+插件价值
- 工具选择逻辑与核心Agent解耦
- 支持动态扩展工具集和路由策略
- 避免核心代码臃肿

### 4. 工具执行器 (`tool_executor.rs`)
实现了工具的实际执行逻辑，为每个工具提供具体的实现：

## 实现特点

### 1. 上下文感知
系统能够根据用户输入的上下文智能选择工具，而不需要明确指定工具名称。

### 2. 动态扩展性
支持在运行时：
- 新增工具
- 新增或修改路由规则
- 调整规则优先级

### 3. 微内核架构
采用插件化设计，路由逻辑与Agent核心解耦，符合高内聚低耦合原则。

## 使用示例

```rust
// 创建工具路由插件
let mut routing_plugin = ToolRoutingPlugin::new();

// 测试数字计算路由
let input1 = "计算 2 + 3 * 4";
let route_result1 = routing_plugin.route_analysis(input1, "").await;
// 路由结果: Some("calculator")

// 测试最近新闻路由
let input2 = "最近有什么科技事件？";
let route_result2 = routing_plugin.route_analysis(input2, "").await;
// 路由结果: Some("news_query")

// 测试天气查询路由
let input3 = "北京天气怎么样？";
let route_result3 = routing_plugin.route_analysis(input3, "").await;
// 路由结果: Some("weather_query")

// 动态添加股票查询工具和规则
let stock_rule = RouteRule::new("stock_query_rule", "股票 行情", "stock_query", 75);
rule_manager.add_rule(stock_rule);

// 测试股票查询路由
let input4 = "AAPL股票价格是多少？";
let route_result4 = routing_plugin.route_analysis(input4, "").await;
// 路由结果: Some("stock_query")
```

## 构建与运行

由于项目依赖问题，直接构建示例可能会遇到困难。建议将这些模块集成到您的Agent系统中使用。

