//! 金融合规Agent示例
//! Financial Compliance Agent Example
//!
//! 演示金融Agent的实时合规规则执行，包括：
//! Demonstrates real-time compliance rule execution for financial Agents, including:
//! - 编写合规检查插件，定义"禁止提及敏感理财产品"、"利率表述规范"等规则
//! - Developing compliance plugins to define rules like "Banned sensitive products" and "Interest rate standards"
//! - 运行时接收监管部门的规则更新，自动重载插件并执行新规则
//! - Receiving regulatory rule updates at runtime, auto-reloading plugins, and executing new rules

use mofa_sdk::plugins::rhai_runtime::{RhaiPlugin, RhaiPluginConfig};
use mofa_sdk::plugins::{AgentPlugin, PluginContext};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tracing::{info, warn, Level};

// ============================================================================
// 1. 创建合规规则目录和初始规则文件
// 1. Create compliance rules directory and initial rule file
// ============================================================================

async fn create_initial_rules_file(plugin_dir: &Path, rules_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // 创建插件目录
    // Create plugin directory
    tokio::fs::create_dir_all(plugin_dir).await?;

    // 初始合规规则
    // Initial compliance rules
    let initial_rules = r#"
// 金融合规检查规则
// Financial compliance inspection rules
// @name: FinancialComplianceRules
// @version: 1.0.0
// @description: 金融Agent合规检查规则引擎
// @description: Financial Agent compliance inspection rule engine

fn execute(content) {
    // 规则1: 禁止提及敏感理财产品
    // Rule 1: Prohibition of mentioning sensitive financial products
    let sensitive_products = ["高风险", "非法集资", "虚拟货币", "传销"];
    for product in sensitive_products {
        if content.contains(product) {
            return #{
                compliant: false,
                reason: "包含禁止提及的敏感理财产品: " + product,
                rule_id: "FIN-001",
                content: content
            };
        }
    }

    // 规则2: 利率表述规范 - 必须明确说明年化/月化
    // Rule 2: Interest rate expression standard - must specify annual/monthly
    let interest_keywords = ["利率", "年化", "月化", "收益率"];
    let has_interest = interest_keywords.some(|keyword| content.contains(keyword));
    let has_rate_type = content.contains("年化") || content.contains("月化");

    if has_interest && !has_rate_type {
        return #{
            compliant: false,
            reason: "利率表述不规范，必须明确说明年化或月化",
            rule_id: "FIN-002",
            content: content
        };
    }

    // 规则3: 禁止承诺保本保收益
    // Rule 3: Prohibition of promising principal or return guarantees
    let guarantee_words = ["保本", "保收益", "无风险"];
    for word in guarantee_words {
        if content.contains(word) {
            return #{
                compliant: false,
                reason: "禁止承诺保本保收益",
                rule_id: "FIN-003",
                content: content
            };
        }
    }

    // 所有规则通过
    // All rules passed
    return #{
        compliant: true,
        reason: "合规检查通过",
        rule_id: "PASS",
        content: content
    };
}
"#;

    tokio::fs::write(rules_file, initial_rules).await?;
    info!("创建了初始合规规则文件: {:?}", rules_file);
    info!("Created initial compliance rules file: {:?}", rules_file);

    Ok(())
}

// ============================================================================
// 主函数 - 演示合规检查流程
// Main Function - Demonstrating compliance check workflow
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== 金融合规Agent演示开始 ===\n");
    info!("=== Financial Compliance Agent Demo Starts ===\n");

    // 定义规则文件路径
    // Define rule file path
    let plugin_dir = Path::new("./compliance_rules");
    let rules_file = plugin_dir.join("financial_rules.rhai");

    // 创建初始规则文件
    // Create initial rules file
    create_initial_rules_file(plugin_dir, &rules_file).await?;

    // 创建插件上下文
    // Create plugin context
    let ctx = PluginContext::new("financial_compliance_agent");

    // 创建并加载合规插件
    // Create and load compliance plugin
    let config = RhaiPluginConfig::new_file("financial_compliance", &rules_file);
    let mut rhai_plugin = RhaiPlugin::new(config).await?;
    rhai_plugin.load(&ctx).await?;
    rhai_plugin.init_plugin().await?;
    rhai_plugin.start().await?;

    // 将插件放入 Arc<Mutex> 中以便在文件变化处理中使用
    // Put plugin in Arc<Mutex> for use in file change handling
    let plugin_arc = Arc::new(Mutex::new(rhai_plugin));

    // 初始化文件监视器
    // Initialize file watcher
    let (tx, mut rx) = mpsc::channel(10);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.blocking_send(res);
        },
        Config::default()
    )?;

    watcher.watch(&rules_file, RecursiveMode::NonRecursive)?;
    info!("开始监视规则文件变化: {:?}", rules_file);
    info!("Started monitoring rule file changes: {:?}", rules_file);

    // 克隆插件引用以便在文件变化处理任务中使用
    // Clone plugin reference for use in file change processing task
    let plugin_arc_clone = Arc::clone(&plugin_arc);

    // 启动文件变化处理任务
    // Start file change processing task
    let watcher_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(Ok(event)) => {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        info!("检测到规则文件变化，正在重载...");
                        info!("Rule file change detected, reloading...");

                        // 加锁并重新加载插件
                        // Lock and reload the plugin
                        let mut plugin = plugin_arc_clone.lock().await;
                        match plugin.reload().await {
                            Ok(_) => info!("规则插件重载成功"),
                            Ok(_) => info!("Rule plugin reloaded successfully"),
                            Err(e) => warn!("规则插件重载失败: {}", e),
                            Err(e) => warn!("Rule plugin reload failed: {}", e),
                        }
                    }
                },
                Some(Err(e)) => {
                    warn!("文件监视错误: {}", e);
                    warn!("File watcher error: {}", e);
                    break;
                },
                None => {
                    // 通道关闭
                    // Channel closed
                    break;
                }
            }
        }
    });

    // 4. 测试初始合规规则
    // 4. Test initial compliance rules
    info!("\n--- 测试初始合规规则 ---");
    info!("\n--- Testing initial compliance rules ---");

    let test_contents = vec![
        "我们公司提供年化10%的理财产品",  // 合规
        // "Our company provides 10% APY wealth products", // Compliant
        "我们公司提供10%的理财产品",        // 不合规 - 利率未说明年化/月化
        // "Our company provides 10% wealth products", // Non-compliant - No period specified
        "我们公司提供保本保收益的产品",      // 不合规 - 承诺保本保收益
        // "Our company provides guaranteed principal products", // Non-compliant - Guarantee
        "我们公司提供虚拟货币投资服务",      // 不合规 - 敏感理财产品
        // "Our company provides crypto investment services", // Non-compliant - Sensitive
        "我们公司提供高风险的投资项目",      // 不合规 - 敏感理财产品
        // "Our company provides high-risk investment projects", // Non-compliant - Sensitive
    ];

    // 使用插件进行合规检查
    // Use plugin for compliance check
    let mut plugin = plugin_arc.lock().await;
    for content in test_contents {
        match plugin.execute(content.to_string()).await {
            Ok(result) => {
                info!("内容: {}", content);
                info!("Content: {}", content);
                info!("结果: {}", result);
                info!("Result: {}", result);
            },
            Err(e) => {
                warn!("合规检查失败: {}", e);
                warn!("Compliance check failed: {}", e);
            }
        }
        println!();
    }

    // 5. 演示规则动态更新
    // 5. Demonstrate dynamic rule updates
    info!("\n--- 演示规则动态更新 ---");
    info!("\n--- Demonstrating dynamic rule updates ---");
    info!("现在修改规则文件: {:?}", rules_file);
    info!("Now modify rule file: {:?}", rules_file);
    info!("建议修改：");
    info!("Recommended modifications:");
    info!("1. 添加新的敏感词汇，如\"外汇保证金\"");
    info!("1. Add new sensitive words, e.g., \"Forex Margin\"");
    info!("2. 修改利率表述规则");
    info!("2. Modify interest rate expression rules");
    info!("或其他任何合规规则的修改...");
    info!("Or any other modifications to compliance rules...");
    info!("\n等待30秒，期间可以修改文件内容...");
    info!("\nWaiting 30 seconds for manual file modification...");

    // 等待30秒让用户修改文件
    // Wait for 30 seconds for user to modify file
    tokio::time::sleep(Duration::from_secs(30)).await;

    // 6. 测试更新后的规则
    // 6. Test updated rules
    info!("\n--- 测试更新后的规则 ---");
    info!("\n--- Testing updated rules ---");

    // 测试更新后的规则
    // Test the rules after update
    let updated_test_contents = vec![
        "我们公司提供年化10%的理财产品",      // 合规
        // "Our company provides 10% APY wealth products", // Compliant
        "我们公司提供外汇保证金交易服务",      // 新的敏感词汇测试
        // "Our company provides Forex Margin trading", // New sensitive word test
    ];

    // 再次检查合规性
    // Check compliance again
    for content in updated_test_contents {
        match plugin.execute(content.to_string()).await {
            Ok(result) => {
                info!("内容: {}", content);
                info!("Content: {}", content);
                info!("结果: {}", result);
                info!("Result: {}", result);
            },
            Err(e) => {
                warn!("合规检查失败: {}", e);
                warn!("Compliance check failed: {}", e);
            }
        }
        println!();
    }

    // 清理资源
    // Cleanup resources
    plugin.stop().await?;
    plugin.unload().await?;
    watcher_task.abort();

    // 删除测试文件和目录
    // Delete test files and directory
    tokio::fs::remove_file(rules_file).await?;
    tokio::fs::remove_dir(plugin_dir).await?;

    info!("\n=== 金融合规Agent演示完成 ===\n");
    info!("\n=== Financial Compliance Agent Demo Finished ===\n");

    Ok(())
}
