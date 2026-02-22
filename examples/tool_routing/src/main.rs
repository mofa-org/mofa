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

    // 1. 初始化工具列表
    let mut tools = vec![
        create_calculator_tool(),
        create_weather_tool(),
        create_news_tool(),
    ];

    // 2. 创建Mock LLM Provider（真实项目中替换为OpenAI等真实Provider）
    let mock_provider = Arc::new(MockLLMProvider::new("mock-llm"));

    // 3. 创建Agent并配置工具
    let agent = Arc::new(simple_llm_agent(
        "multi-task-agent",
        mock_provider.clone(),
        "You are a helpful assistant with access to various tools."
    ));

    // 4. 创建工具执行器
    let tool_executor = ExampleToolExecutor::new();

    // 5. 创建并添加工具路由插件
    let mut routing_plugin = ToolRoutingPlugin::new();
    let rule_manager = routing_plugin.rule_manager();

    println!("✅ 系统初始化完成");
    println!("当前已加载的工具: calculator, weather_query, news_query");
    println!();

    // 6. 测试示例1: 数字计算（路由到计算器）
    println!("--- 测试1: 数字计算 ---");
    let input1 = "计算 2 + 3 * 4";
    println!("用户输入: {}", input1);

    let route_result1 = routing_plugin.route_analysis(input1, "").await;
    println!("路由结果: {:?}", route_result1);
    println!();

    // 7. 测试示例2: 最近新闻（路由到新闻API）
    println!("--- 测试2: 最近新闻 ---");
    let input2 = "最近有什么科技事件？";
    println!("用户输入: {}", input2);

    let route_result2 = routing_plugin.route_analysis(input2, "").await;
    println!("路由结果: {:?}", route_result2);
    println!();

    // 8. 测试示例3: 天气查询（路由到天气API）
    println!("--- 测试3: 天气查询 ---");
    let input3 = "北京天气怎么样？";
    println!("用户输入: {}", input3);

    let route_result3 = routing_plugin.route_analysis(input3, "").await;
    println!("路由结果: {:?}", route_result3);
    println!();

    // 9. 动态更新规则：新增股票查询工具和路由规则
    println!("--- 动态更新规则: 添加股票查询工具 ---");
    let stock_tool = create_stock_tool();
    tools.push(stock_tool);

    // 添加股票查询路由规则
    let stock_rule = RouteRule::new(
        "stock_query_rule",
        "股票 行情 价格",
        "stock_query",
        75
    );
    rule_manager.add_rule(stock_rule);

    println!("✅ 已添加股票查询工具和路由规则");
    println!();

    // 10. 测试示例4: 股票查询（路由到股票API）
    println!("--- 测试4: 股票查询 ---");
    let input4 = "AAPL股票价格是多少？";
    println!("用户输入: {}", input4);

    let route_result4 = routing_plugin.route_analysis(input4, "").await;
    println!("路由结果: {:?}", route_result4);
    println!();

    // 11. 展示插件统计信息
    println!("--- 系统状态 ---");
    let stats = routing_plugin.stats();
    println!("插件统计: {:?}", stats);
    println!();

    // 12. 动态更新规则演示 - 修改规则优先级
    println!("--- 动态更新规则: 修改规则优先级 ---");
    println!("当前所有规则:");
    for rule in rule_manager.get_all_rules() {
        println!("  - {} (优先级: {}): {} -> {}", rule.name, rule.priority, rule.context_pattern, rule.target_tool);
    }
    println!();

    // 修改天气查询规则的优先级
    let updated_weather_rule = RouteRule::new(
        "weather_query_rule",
        "天气 温度",
        "weather_query",
        95  // 提高到最高优先级
    );
    rule_manager.update_rule(updated_weather_rule);

    println!("✅ 已将天气查询规则优先级提高到95");
    println!();

    println!("修改后的规则:");
    for rule in rule_manager.get_all_rules() {
        println!("  - {} (优先级: {}): {} -> {}", rule.name, rule.priority, rule.context_pattern, rule.target_tool);
    }
    println!();

    println!("🎉 上下文感知工具路由示例演示完成！");
    Ok(())
}
