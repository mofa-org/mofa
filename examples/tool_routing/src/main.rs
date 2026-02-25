mod route_rules;
mod tool_definitions;
mod tool_routing_plugin;
mod tool_executor;

use mofa_sdk::llm::{simple_llm_agent, MockLLMProvider};
use mofa_sdk::kernel::AgentPlugin;
use route_rules::RouteRule;
use std::sync::Arc;
use tool_definitions::{create_calculator_tool, create_news_tool, create_stock_tool, create_weather_tool};
use tool_executor::ExampleToolExecutor;
use tool_routing_plugin::ToolRoutingPlugin;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🔧 多任务Agent的上下文感知工具路由示例\n");
    // 🔧 Example of context-aware tool routing for Multi-task Agents

    // 1. 初始化工具列表
    // 1. Initialize the tool list
    let mut tools = vec![
        create_calculator_tool(),
        create_weather_tool(),
        create_news_tool(),
    ];

    // 2. 创建Mock LLM Provider（真实项目中替换为OpenAI等真实Provider）
    // 2. Create Mock LLM Provider (replace with real Providers like OpenAI in production)
    let mock_provider = Arc::new(MockLLMProvider::new("mock-llm"));

    // 3. 创建Agent并配置工具
    // 3. Create Agent and configure tools
    let agent = Arc::new(simple_llm_agent(
        "multi-task-agent",
        mock_provider.clone(),
        "You are a helpful assistant with access to various tools."
    ));

    // 4. 创建工具执行器
    // 4. Create tool executor
    let tool_executor = ExampleToolExecutor::new();

    // 5. 创建并添加工具路由插件
    // 5. Create and add tool routing plugin
    let mut routing_plugin = ToolRoutingPlugin::new();
    let rule_manager = routing_plugin.rule_manager();

    println!("✅ 系统初始化完成");
    // ✅ System initialization complete
    println!("当前已加载的工具: calculator, weather_query, news_query");
    // Currently loaded tools: calculator, weather_query, news_query
    println!();

    // 6. 测试示例1: 数字计算（路由到计算器）
    // 6. Test Example 1: Numerical calculation (Route to calculator)
    println!("--- 测试1: 数字计算 ---");
    // --- Test 1: Numerical Calculation ---
    let input1 = "计算 2 + 3 * 4";
    println!("用户输入: {}", input1);
    // User Input: {}

    let route_result1 = routing_plugin.route_analysis(input1, "").await;
    println!("路由结果: {:?}", route_result1);
    // Routing Result: {:?}
    println!();

    // 7. 测试示例2: 最近新闻（路由到新闻API）
    // 7. Test Example 2: Recent news (Route to news API)
    println!("--- 测试2: 最近新闻 ---");
    // --- Test 2: Recent News ---
    let input2 = "最近有什么科技事件？";
    println!("用户输入: {}", input2);
    // User Input: {}

    let route_result2 = routing_plugin.route_analysis(input2, "").await;
    println!("路由结果: {:?}", route_result2);
    // Routing Result: {:?}
    println!();

    // 8. 测试示例3: 天气查询（路由到天气API）
    // 8. Test Example 3: Weather query (Route to weather API)
    println!("--- 测试3: 天气查询 ---");
    // --- Test 3: Weather Query ---
    let input3 = "北京天气怎么样？";
    println!("用户输入: {}", input3);
    // User Input: {}

    let route_result3 = routing_plugin.route_analysis(input3, "").await;
    println!("路由结果: {:?}", route_result3);
    // Routing Result: {:?}
    println!();

    // 9. 动态更新规则：新增股票查询工具和路由规则
    // 9. Dynamically update rules: add stock query tool and routing rules
    println!("--- 动态更新规则: 添加股票查询工具 ---");
    // --- Dynamic Rule Update: Adding Stock Query Tool ---
    let stock_tool = create_stock_tool();
    tools.push(stock_tool);

    // 添加股票查询路由规则
    // Add stock query routing rule
    let stock_rule = RouteRule::new(
        "stock_query_rule",
        "股票 行情 价格",
        "stock_query",
        75
    );
    rule_manager.add_rule(stock_rule);

    println!("✅ 已添加股票查询工具和路由规则");
    // ✅ Stock query tool and routing rule added
    println!();

    // 10. 测试示例4: 股票查询（路由到股票API）
    // 10. Test Example 4: Stock query (Route to stock API)
    println!("--- 测试4: 股票查询 ---");
    // --- Test 4: Stock Query ---
    let input4 = "AAPL股票价格是多少？";
    println!("用户输入: {}", input4);
    // User Input: {}

    let route_result4 = routing_plugin.route_analysis(input4, "").await;
    println!("路由结果: {:?}", route_result4);
    // Routing Result: {:?}
    println!();

    // 11. 展示插件统计信息
    // 11. Display plugin statistics
    println!("--- 系统状态 ---");
    // --- System Status ---
    let stats = routing_plugin.stats();
    println!("插件统计: {:?}", stats);
    // Plugin Stats: {:?}
    println!();

    // 12. 动态更新规则演示 - 修改规则优先级
    // 12. Dynamic rule update demonstration - modifying rule priority
    println!("--- 动态更新规则: 修改规则优先级 ---");
    // --- Dynamic Rule Update: Modifying Rule Priority ---
    println!("当前所有规则:");
    // Current rules:
    for rule in rule_manager.get_all_rules() {
        println!("  - {} (优先级: {}): {} -> {}", rule.name, rule.priority, rule.context_pattern, rule.target_tool);
    }
    println!();

    // 修改天气查询规则的优先级
    // Modify the priority of the weather query rule
    let updated_weather_rule = RouteRule::new(
        "weather_query_rule",
        "天气 温度",
        "weather_query",
        95  // 提高到最高优先级
        // 95  // Increased to the highest priority
    );
    rule_manager.update_rule(updated_weather_rule);

    println!("✅ 已将天气查询规则优先级提高到95");
    // ✅ Weather query rule priority increased to 95
    println!();

    println!("修改后的规则:");
    // Modified rules:
    for rule in rule_manager.get_all_rules() {
        println!("  - {} (优先级: {}): {} -> {}", rule.name, rule.priority, rule.context_pattern, rule.target_tool);
    }
    println!();

    println!("🎉 上下文感知工具路由示例演示完成！");
    // 🎉 Context-aware tool routing example demonstration complete!
    Ok(())
}
