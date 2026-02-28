//! Rhai 脚本引擎集成示例
//! Rhai script engine integration example
//!
//! 本示例演示了 MoFA 框架中 Rhai 脚本引擎的多种应用场景：
//! This example demonstrates multiple application scenarios of the Rhai script engine in the MoFA framework:
//! 1. 基础脚本执行
//! 1. Basic script execution
//! 2. 脚本化工作流节点
//! 2. Scripted workflow nodes
//! 3. 动态工具定义
//! 3. Dynamic tool definition
//! 4. 规则引擎
//! 4. Rule engine

use mofa_sdk::rhai::{
    // 脚本引擎
    // Script engine
    condition_script, task_script, ParameterType, RhaiScriptEngine, RuleAction, RuleBuilder,
    RuleEngine, RuleGroupDefinition,
    // 工具
    // Tools
    RuleMatchMode, RulePriority, ScriptContext, ScriptEngineConfig, ScriptSecurityConfig,
    // 规则引擎
    // Rule engine
    ScriptToolDefinition, ScriptToolRegistry, ScriptWorkflowDefinition, ScriptWorkflowExecutor, ToolBuilder,
    ToolParameter,
};
use std::collections::HashMap;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== MoFA Rhai Script Engine Integration Example ===\n");
    // === MoFA Rhai Script Engine Integration Example ===

    // 运行所有示例
    // Run all examples
    demo_basic_script_execution().await?;
    demo_script_workflow().await?;
    demo_dynamic_tools().await?;
    demo_rule_engine().await?;
    demo_advanced_features().await?;

    info!("\n=== All examples completed ===");
    // === All examples completed ===
    Ok(())
}

/// 示例 1: 基础脚本执行
/// Example 1: Basic script execution
async fn demo_basic_script_execution() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n--- Example 1: Basic script execution ---\n");
    // --- Example 1: Basic script execution ---

    // 创建脚本引擎
    // Create script engine
    let engine = RhaiScriptEngine::new(ScriptEngineConfig::default())?;

    // 1.1 简单表达式
    // 1.1 Simple expression
    info!("1.1 Simple expression calculation:");
    // 1.1 Simple expression calculation:
    let context = ScriptContext::new();
    let result = engine.execute("(1 + 2) * 3 + 4", &context).await?;
    info!("  表达式: (1 + 2) * 3 + 4 = {}", result.value);
    // Expression: (1 + 2) * 3 + 4 = {}

    // 1.2 使用变量
    // 1.2 Using variables
    info!("\n1.2 Using variables:");
    // 1.2 Using variables:
    let context = ScriptContext::new()
        .with_variable("name", "MoFA")?
        .with_variable("version", 1)?;

    let result = engine.execute(
        r#"
            let greeting = "Hello, " + name + "!";
            let info = "Version: " + version;
            greeting + " " + info
        "#,
        &context,
    ).await?;
    info!("  Result: {}", result.value);
    // Result: {}

    // 1.3 使用函数
    // 1.3 Using functions
    info!("\n1.3 Define and call functions:");
    // 1.3 Define and call functions:
    let result = engine.execute(
        r#"
            fn fibonacci(n) {
                if n <= 1 { return n; }
                fibonacci(n - 1) + fibonacci(n - 2)
            }
            fibonacci(10)
        "#,
        &context,
    ).await?;
    info!("  fibonacci(10) = {}", result.value);

    // 1.4 使用内置函数
    // 1.4 Using built-in functions
    info!("\n1.4 Built-in functions:");
    // 1.4 Built-in functions:
    let result = engine.execute(
        r#"
            let text = "  Hello World  ";
            let trimmed = trim(text);
            let upper_text = upper(trimmed);
            let json = to_json(#{name: "test", value: 42});
            #{
                trimmed: trimmed,
                upper: upper_text,
                json: json,
                timestamp: now()
            }
        "#,
        &context,
    ).await?;
    info!("  Result: {}", serde_json::to_string_pretty(&result.value)?);
    // Result: {}

    // 1.5 编译和缓存脚本
    // 1.5 Compile and cache script
    info!("\n1.5 Compile and cache script:");
    // 1.5 Compile and cache script:
    engine.compile_and_cache(
        "calculator",
        "Calculator",
        r#"
            fn add(a, b) { a + b }
            fn multiply(a, b) { a * b }
            fn calculate(x, y, op) {
                if op == "add" { add(x, y) }
                else if op == "mul" { multiply(x, y) }
                else { 0 }
            }
            calculate(input.x, input.y, input.op)
        "#,
    ).await?;

    let context = ScriptContext::new()
        .with_variable("input", serde_json::json!({
            "x": 10,
            "y": 5,
            "op": "mul"
        }))?;
    let result = engine.execute_compiled("calculator", &context).await?;
    info!("  calculate(10, 5, \"mul\") = {}", result.value);

    Ok(())
}

/// 示例 2: 脚本化工作流
/// Example 2: Scripted workflow
async fn demo_script_workflow() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n--- Example 2: Scripted workflow ---\n");
    // --- Example 2: Scripted workflow ---

    // 2.1 简单线性工作流
    // 2.1 Simple linear workflow
    info!("2.1 Simple linear workflow (data processing pipeline):");
    // 2.1 Simple linear workflow (data processing pipeline):
    let mut workflow = ScriptWorkflowDefinition::new("data_pipeline", "Data Processing Pipeline");
    // ScriptWorkflowDefinition: data_pipeline, data processing pipeline

    workflow
        .add_node(task_script(
            "validate",
            "Data Validation",
            // Data validation
            r#"
                log("Validating input data...");
                if input.value < 0 {
                    throw "值不能为负数";
                }
                input
            "#,
        ))
        .add_node(task_script(
            "transform",
            "Data Transformation",
            // Data transformation
            r#"
                log("Transforming data...");
                #{
                    original: input.value,
                    doubled: input.value * 2,
                    squared: input.value * input.value
                }
            "#,
        ))
        .add_node(task_script(
            "format",
            "Format Output",
            // Format output
            r#"
                log("Formatting output...");
                "Result: original=" + input.original +
                ", doubled=" + input.doubled +
                ", squared=" + input.squared
            "#,
        ))
        .add_edge("validate", "transform")
        .add_edge("transform", "format")
        .set_start("validate")
        .add_end("format");

    let executor = ScriptWorkflowExecutor::new(workflow, ScriptEngineConfig::default()).await?;
    let result = executor.execute(serde_json::json!({"value": 5})).await?;
    info!("  {}", result);

    // 2.2 条件分支工作流
    // 2.2 Conditional branching workflow
    info!("\n2.2 Conditional branching workflow (user rating system):");
    // 2.2 Conditional branching workflow (user rating system):
    let mut workflow = ScriptWorkflowDefinition::new("rating_system", "Rating System");
    // ScriptWorkflowDefinition: rating_system, rating system

    workflow
        .add_node(condition_script(
            "check_score",
            "Check Score",
            // Check score
            r#"
                let score = input.score;

                // 直接修改并返回新对象
                // Modify directly and return new object
                if score >= 90 { #{score: input.score, rating: "excellent"} }
                else if score >= 70 { #{score: input.score, rating: "good"} }
                else if score >= 60 { #{score: input.score, rating: "pass"} }
                else { #{score: input.score, rating: "fail"} }
            "#,
        ))
        .add_node(task_script(
            "excellent",
            "Excellent Handling",
            // Excellent grade processing
            r#"#{rating: "A", message: "Excellent! Score: " + to_string(input.score)}"#,
        ))
        .add_node(task_script(
            "good",
            "Good Handling",
            // Good grade processing
            r#"#{rating: "B", message: "Good! Score: " + to_string(input.score)}"#,
        ))
        .add_node(task_script(
            "pass",
            "Pass Handling",
            // Pass grade processing
            r#"#{rating: "C", message: "Pass. Score: " + to_string(input.score)}"#,
        ))
        .add_node(task_script(
            "fail",
            "Fail Handling",
            // Fail grade processing
            r#"#{rating: "D", message: "Fail. Score: " + to_string(input.score)}"#,
        ))
        .add_node(task_script("end", "结束", "input"))
        // Task script end
        .add_conditional_edge("check_score", "excellent", "rating == \"excellent\"")
        .add_conditional_edge("check_score", "good", "rating == \"good\"")
        .add_conditional_edge("check_score", "pass", "rating == \"pass\"")
        .add_conditional_edge("check_score", "fail", "rating == \"fail\"")
        .add_edge("excellent", "end")
        .add_edge("good", "end")
        .add_edge("pass", "end")
        .add_edge("fail", "end")
        .set_start("check_score")
        .add_end("end");

    let executor = ScriptWorkflowExecutor::new(workflow, ScriptEngineConfig::default()).await?;

    for score in [95, 75, 65, 45] {
        executor.reset().await;
        let result = executor.execute(serde_json::json!({"score": score})).await?;
        info!("  Score {}: {:?}", score, result);
        // Score {}: {:?}
    }

    Ok(())
}

/// 示例 3: 动态工具定义
/// Example 3: Dynamic tool definition
async fn demo_dynamic_tools() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n--- Example 3: Dynamic tool definition ---\n");
    // --- Example 3: Dynamic tool definition ---

    let registry = ScriptToolRegistry::new(ScriptEngineConfig::default())?;

    // 3.1 注册计算器工具
    // 3.1 Register calculator tool
    info!("3.1 Register calculator tool:");
    // 3.1 Register calculator tool:
    let calc_tool = ToolBuilder::new("calculator", "Advanced Calculator")
        // Advanced calculator
        .description("Perform mathematical operations")
        // Perform mathematical operations
        .param(ToolParameter::new("operation", ParameterType::String)
            .required()
            .with_description("Operation type: add, sub, mul, div, pow")
            // Operation type: add, sub, mul, div, pow
            .with_enum(vec![
                serde_json::json!("add"),
                serde_json::json!("sub"),
                serde_json::json!("mul"),
                serde_json::json!("div"),
                serde_json::json!("pow"),
            ]))
        .param(ToolParameter::new("a", ParameterType::Float).required())
        .param(ToolParameter::new("b", ParameterType::Float).required())
        .script(r#"
            let a = params.a;
            let b = params.b;
            let op = params.operation;

            let result = if op == "add" { a + b }
            else if op == "sub" { a - b }
            else if op == "mul" { a * b }
            else if op == "div" {
                if b == 0.0 { throw "除数不能为零"; }
                a / b
            }
            else if op == "pow" {
                let r = 1.0;
                for i in 0..b.to_int() { r *= a; }
                r
            }
            else { throw "未知操作: " + op; };

            #{
                operation: op,
                a: a,
                b: b,
                result: result,
                expression: `${a} ${op} ${b} = ${result}`
            }
        "#)
        .tag("math")
        .build();

    registry.register(calc_tool).await?;

    // 测试计算器
    // Test calculator
    let operations = vec![
        ("add", 10.0, 5.0),
        ("sub", 10.0, 3.0),
        ("mul", 7.0, 6.0),
        ("div", 100.0, 4.0),
        ("pow", 2.0, 10.0),
    ];

    for (op, a, b) in operations {
        let mut input = HashMap::new();
        input.insert("operation".to_string(), serde_json::json!(op));
        input.insert("a".to_string(), serde_json::json!(a));
        input.insert("b".to_string(), serde_json::json!(b));

        let result = registry.execute("calculator", input).await?;
        info!("  {}", result.result["expression"]);
    }

    // 3.2 注册字符串处理工具
    // 3.2 Register string processing tool
    info!("\n3.2 Register string processing tool:");
    // 3.2 Register string processing tool:
    let string_tool = ScriptToolDefinition::new(
        "string_processor",
        "String Processor",
        // String processor
        r#"
            let text = params.text;
            let ops = params.operations;

            for op in ops {
                if op == "trim" { text = trim(text); }
                else if op == "upper" { text = upper(text); }
                else if op == "lower" { text = lower(text); }
                else if op == "reverse" {
                    let chars = text.chars();
                    let reversed = "";
                    for i in range(0, chars.len()) {
                        reversed = chars[chars.len() - 1 - i] + reversed;
                    }
                    text = reversed;
                }
            }

            #{
                original: params.text,
                processed: text,
                operations: ops
            }
        "#,
    )
    .with_description("Perform multiple operations on strings")
    // Perform multiple operations on strings
    .with_parameter(ToolParameter::new("text", ParameterType::String).required())
    .with_parameter(ToolParameter::new("operations", ParameterType::Array).required())
    .with_tag("string");

    registry.register(string_tool).await?;

    let mut input = HashMap::new();
    input.insert("text".to_string(), serde_json::json!("  Hello World  "));
    input.insert("operations".to_string(), serde_json::json!(["trim", "upper"]));

    let result = registry.execute("string_processor", input).await?;
    info!("  Original: \"{}\"", result.result["original"]);
    // Original: \"{}\"
    info!("  Processed: \"{}\"", result.result["processed"]);
    // Processed: \"{}\"

    // 3.3 生成 JSON Schema（用于 LLM function calling）
    // 3.3 Generate JSON Schema (for LLM function calling)
    info!("\n3.3 Tool JSON Schema (for LLM):");
    // 3.3 Tool JSON Schema (for LLM):
    let schemas = registry.generate_tool_schemas().await;
    for schema in &schemas {
        info!("  Tool: {}", schema["name"]);
        // Tool: {}
    }

    Ok(())
}

/// 示例 4: 规则引擎
/// Example 4: Rule engine
async fn demo_rule_engine() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n--- Example 4: Rule engine ---\n");
    // --- Example 4: Rule engine ---

    let engine = RuleEngine::new(ScriptEngineConfig::default())?;

    // 4.1 折扣规则系统
    // 4.1 Discount rule system
    info!("4.1 E-commerce discount rule system:");
    // 4.1 E-commerce discount rule system:

    // VIP 会员折扣
    // VIP member discount
    engine.register_rule(
        RuleBuilder::new("vip_discount", "VIP Member Discount")
            // VIP member discount
            .description("VIP members enjoy a 20% discount")
            // VIP members enjoy a 20% discount
            .priority(RulePriority::High)
            .condition("user.is_vip == true")
            .then_execute(r#"
                let discount = 0.8;
                let final_price = order.total * discount;
                #{
                    rule: "vip_discount",
                    discount_rate: discount,
                    original_price: order.total,
                    final_price: final_price,
                    saved: order.total - final_price
                }
            "#)
            .tag("discount")
            .build()
    ).await?;

    // 大额订单折扣
    // Large order discount
    engine.register_rule(
        RuleBuilder::new("bulk_discount", "Large Order Discount")
            // Large order discount
            .description("Get $100 off for orders over $1000")
            // $100 off for orders over $1000
            .priority(RulePriority::Normal)
            .condition("order.total >= 1000")
            .then_execute(r#"
                let discount_amount = 100;
                let final_price = order.total - discount_amount;
                #{
                    rule: "bulk_discount",
                    discount_amount: discount_amount,
                    original_price: order.total,
                    final_price: final_price,
                    saved: discount_amount
                }
            "#)
            .tag("discount")
            .build()
    ).await?;

    // 新用户折扣
    // New user discount
    engine.register_rule(
        RuleBuilder::new("new_user_discount", "New User Discount")
            // New user discount
            .description("10% off for a new user's first order")
            // 10% off for new user's first order
            .priority(RulePriority::Low)
            .condition("user.is_new == true && user.order_count == 0")
            .then_execute(r#"
                let discount = 0.9;
                let final_price = order.total * discount;
                #{
                    rule: "new_user_discount",
                    discount_rate: discount,
                    original_price: order.total,
                    final_price: final_price,
                    saved: order.total - final_price
                }
            "#)
            .tag("discount")
            .build()
    ).await?;

    // 创建规则组
    // Create rule group
    engine.register_group(
        RuleGroupDefinition::new("discount_rules", "Discount Rule Group")
            // Discount rule group
            .with_match_mode(RuleMatchMode::FirstMatch)  // 只应用第一个匹配的规则
            // Only apply the first matching rule
            .with_rules(vec!["vip_discount", "bulk_discount", "new_user_discount"])
            .with_default_action(RuleAction::ReturnValue {
                value: serde_json::json!({
                    "rule": "no_discount",
                    "message": "No discount available"
                    // No discount available
                })
            })
    ).await?;

    // 测试不同场景
    // Test different scenarios
    let test_cases = vec![
        ("VIP User", serde_json::json!({
            // VIP User
            "user": {"is_vip": true, "is_new": false, "order_count": 10},
            "order": {"total": 500}
        })),
        ("Large Order", serde_json::json!({
            // Large order
            "user": {"is_vip": false, "is_new": false, "order_count": 5},
            "order": {"total": 1500}
        })),
        ("New User", serde_json::json!({
            // New user
            "user": {"is_vip": false, "is_new": true, "order_count": 0},
            "order": {"total": 300}
        })),
        ("Regular User Small Order", serde_json::json!({
            // Regular user small order
            "user": {"is_vip": false, "is_new": false, "order_count": 3},
            "order": {"total": 200}
        })),
    ];

    for (scenario, data) in test_cases {
        let mut context = ScriptContext::new();
        context.set_variable("user", data["user"].clone())?;
        context.set_variable("order", data["order"].clone())?;

        let result = engine.execute_group("discount_rules", &mut context).await?;
        info!("  Scenario: {} -> {:?}", scenario, result.final_result);
        // Scenario: {} -> {:?}
    }

    // 4.2 内容审核规则
    // 4.2 Content review rules
    info!("\n4.2 Content review rules:");
    // 4.2 Content review rules:

    engine.register_rule(
        RuleBuilder::new("spam_check", "Spam Check")
            // Spam check
            .priority(RulePriority::Critical)
            .condition(r#"
                let content = lower(text);
                contains(content, "buy now") ||
                contains(content, "click here") ||
                contains(content, "free money")
            "#)
            .then_return(serde_json::json!({
                "status": "rejected",
                "reason": "Suspected spam"
                // Suspected spam
            }))
            .build()
    ).await?;

    engine.register_rule(
        RuleBuilder::new("length_check", "Length Check")
            // Length check
            .priority(RulePriority::High)
            .condition("text.len() < 10 || text.len() > 1000")
            .then_return(serde_json::json!({
                "status": "rejected",
                "reason": "Content length requirement mismatch (10-1000 characters)"
                // Content length requirement mismatch (10-1000 characters)
            }))
            .build()
    ).await?;

    engine.register_rule(
        RuleBuilder::new("pass_check", "Review Pass")
            // Review pass
            .priority(RulePriority::Lowest)
            .condition("true")
            .then_return(serde_json::json!({
                "status": "approved",
                "message": "Content review passed"
                // Content review passed
            }))
            .build()
    ).await?;

    engine.register_group(
        RuleGroupDefinition::new("content_review", "Content Review")
            // Content review
            .with_match_mode(RuleMatchMode::FirstMatch)
            .with_rules(vec!["spam_check", "length_check", "pass_check"])
    ).await?;

    let contents = vec![
        "This is a normal comment, sharing my experience.",
        // This is a normal comment, sharing my experience.
        "Buy now! Click here for free money!!!",
        "Short",
        // Short
    ];

    for content in contents {
        let mut context = ScriptContext::new()
            .with_variable("text", content)?;
        let result = engine.execute_group("content_review", &mut context).await?;
        info!("  内容: \"{}...\" -> {:?}",
            &content[..content.len().min(30)],
            result.final_result);
        // Content: \"{}...\" -> {:?}
    }

    Ok(())
}

/// 示例 5: 高级功能
/// Example 5: Advanced features
async fn demo_advanced_features() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n--- 示例 5: 高级功能 ---\n");
    // --- Example 5: Advanced features ---

    // 5.1 安全配置
    // 5.1 Security configuration
    info!("5.1 安全配置:");
    // 5.1 Security configuration:
    let security_config = ScriptSecurityConfig {
        max_execution_time_ms: 1000,
        max_call_stack_depth: 32,
        max_operations: 10_000,
        max_array_size: 1000,
        max_string_size: 10_000,
        allow_loops: true,
        allow_file_operations: false,
        allow_network_operations: false,
    };

    let config = ScriptEngineConfig {
        security: security_config,
        debug_mode: true,
        strict_mode: true,
        ..Default::default()
    };

    let engine = RhaiScriptEngine::new(config)?;
    info!("  已配置安全限制的脚本引擎");
    // Script engine with security restrictions configured

    // 5.2 脚本验证
    // 5.2 Script validation
    info!("\n5.2 脚本语法验证:");
    // 5.2 Script syntax validation:
    let valid_script = "let x = 1 + 2; x * 3";
    let invalid_script = "let x = 1 + ; x * 3";

    let errors = engine.validate(valid_script)?;
    info!("  有效脚本: {} (错误数: {})", valid_script, errors.len());
    // Valid script: {} (error count: {})

    let errors = engine.validate(invalid_script)?;
    info!("  无效脚本: {} (错误: {:?})", invalid_script, errors);
    // Invalid script: {} (errors: {:?})

    // 5.3 脚本日志
    // 5.3 Script logs
    info!("\n5.3 脚本日志收集:");
    // 5.3 Script log collection:
    let context = ScriptContext::new();
    let result = engine.execute(
        r#"
            log("开始处理...");
            debug("调试信息: 变量初始化");
            let result = 42;
            log("处理完成，结果: " + result);
            result
        "#,
        &context,
    ).await?;
    info!("  结果: {}", result.value);
    // Result: {}
    info!("  日志: {:?}", result.logs);
    // Logs: {:?}

    // 5.4 复杂数据处理
    // 5.4 Complex data processing
    info!("\n5.4 复杂数据处理:");
    // 5.4 Complex data processing:
    let context = ScriptContext::new()
        .with_variable("data", serde_json::json!({
            "users": [
                {"name": "Alice", "age": 30, "active": true},
                {"name": "Bob", "age": 25, "active": false},
                {"name": "Charlie", "age": 35, "active": true}
            ]
        }))?;

    let result = engine.execute(
        r#"
            // 过滤活跃用户并计算统计信息
            // Filter active users and calculate statistics
            let users = data.users;
            let active_users = [];
            let total_age = 0;

            for user in users {
                if user.active {
                    active_users.push(user);
                    total_age += user.age;
                }
            }

            #{
                total_users: users.len(),
                active_count: active_users.len(),
                average_age: if active_users.len() > 0 {
                    total_age / active_users.len()
                } else { 0 },
                active_names: active_users.map(|u| u.name)
            }
        "#,
        &context,
    ).await?;
    info!("  处理结果: {}", serde_json::to_string_pretty(&result.value)?);
    // Processing result: {}

    Ok(())
}
