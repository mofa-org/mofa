//! 金融合规Agent示例
//!
//! 演示金融Agent的实时合规规则执行，包括：
//! - 编写合规检查插件，定义"禁止提及敏感理财产品"、"利率表述规范"等规则
//! - 运行时接收监管部门的规则更新，自动重载插件并执行新规则

use anyhow::Result;
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
// ============================================================================

async fn create_initial_rules_file(plugin_dir: &Path, rules_file: &Path) -> Result<()> {
    // 创建插件目录
    tokio::fs::create_dir_all(plugin_dir).await?;

    // 初始合规规则
    let initial_rules = r#"
// 金融合规检查规则
// @name: FinancialComplianceRules
// @version: 1.0.0
// @description: 金融Agent合规检查规则引擎

fn execute(content) {
    // 规则1: 禁止提及敏感理财产品
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

    Ok(())
}

// ============================================================================
// 主函数 - 演示合规检查流程
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== 金融合规Agent演示开始 ===\n");

    // 定义规则文件路径
    let plugin_dir = Path::new("./compliance_rules");
    let rules_file = plugin_dir.join("financial_rules.rhai");

    // 创建初始规则文件
    create_initial_rules_file(plugin_dir, &rules_file).await?;

    // 创建插件上下文
    let ctx = PluginContext::new("financial_compliance_agent");

    // 创建并加载合规插件
    let config = RhaiPluginConfig::new_file("financial_compliance", &rules_file);
    let mut rhai_plugin = RhaiPlugin::new(config).await?;
    rhai_plugin.load(&ctx).await?;
    rhai_plugin.init_plugin().await?;
    rhai_plugin.start().await?;

    // 将插件放入 Arc<Mutex> 中以便在文件变化处理中使用
    let plugin_arc = Arc::new(Mutex::new(rhai_plugin));

    // 初始化文件监视器
    let (tx, mut rx) = mpsc::channel(10);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.blocking_send(res);
        },
        Config::default()
    )?;

    watcher.watch(&rules_file, RecursiveMode::NonRecursive)?;
    info!("开始监视规则文件变化: {:?}", rules_file);

    // 克隆插件引用以便在文件变化处理任务中使用
    let plugin_arc_clone = Arc::clone(&plugin_arc);

    // 启动文件变化处理任务
    let watcher_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(Ok(event)) => {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        info!("检测到规则文件变化，正在重载...");

                        // 加锁并重新加载插件
                        let mut plugin = plugin_arc_clone.lock().await;
                        match plugin.reload().await {
                            Ok(_) => info!("规则插件重载成功"),
                            Err(e) => warn!("规则插件重载失败: {}", e),
                        }
                    }
                },
                Some(Err(e)) => {
                    warn!("文件监视错误: {}", e);
                    break;
                },
                None => {
                    // 通道关闭
                    break;
                }
            }
        }
    });

    // 4. 测试初始合规规则
    info!("\n--- 测试初始合规规则 ---");

    let test_contents = vec![
        "我们公司提供年化10%的理财产品",  // 合规
        "我们公司提供10%的理财产品",        // 不合规 - 利率未说明年化/月化
        "我们公司提供保本保收益的产品",      // 不合规 - 承诺保本保收益
        "我们公司提供虚拟货币投资服务",      // 不合规 - 敏感理财产品
        "我们公司提供高风险的投资项目",      // 不合规 - 敏感理财产品
    ];

    // 使用插件进行合规检查
    let mut plugin = plugin_arc.lock().await;
    for content in test_contents {
        match plugin.execute(content.to_string()).await {
            Ok(result) => {
                info!("内容: {}", content);
                info!("结果: {}", result);
            },
            Err(e) => {
                warn!("合规检查失败: {}", e);
            }
        }
        println!();
    }

    // 5. 演示规则动态更新
    info!("\n--- 演示规则动态更新 ---");
    info!("现在修改规则文件: {:?}", rules_file);
    info!("建议修改：");
    info!("1. 添加新的敏感词汇，如\"外汇保证金\"");
    info!("2. 修改利率表述规则");
    info!("或其他任何合规规则的修改...");
    info!("\n等待30秒，期间可以修改文件内容...");

    // 等待30秒让用户修改文件
    tokio::time::sleep(Duration::from_secs(30)).await;

    // 6. 测试更新后的规则
    info!("\n--- 测试更新后的规则 ---");

    // 测试更新后的规则
    let updated_test_contents = vec![
        "我们公司提供年化10%的理财产品",      // 合规
        "我们公司提供外汇保证金交易服务",      // 新的敏感词汇测试
    ];

    // 再次检查合规性
    for content in updated_test_contents {
        match plugin.execute(content.to_string()).await {
            Ok(result) => {
                info!("内容: {}", content);
                info!("结果: {}", result);
            },
            Err(e) => {
                warn!("合规检查失败: {}", e);
            }
        }
        println!();
    }

    // 清理资源
    plugin.stop().await?;
    plugin.unload().await?;
    watcher_task.abort();

    // 删除测试文件和目录
    tokio::fs::remove_file(rules_file).await?;
    tokio::fs::remove_dir(plugin_dir).await?;

    info!("\n=== 金融合规Agent演示完成 ===\n");

    Ok(())
}
