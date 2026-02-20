//！ 智能体结合实际功能插件与Rhai脚本示例
//！
//！ 本示例演示了：
//！ 1. LLM 与工具插件的 Function Calling 交互机制
//！ 2. Rhai 运行时插件机制 - 动态加载和执行脚本插件
//！ 3. 基于文件的动态插件 - 支持运行时修改和自动重载
//！ 4. 动态规则引擎 - 为 LLM 增加实时可配置的规则
//！
//！ 核心概念：
//！ - ToolPlugin: 工具插件，管理多个工具执行器
//！ - ToolPluginExecutor: 将 ToolPlugin 适配为 LLM 的 ToolExecutor
//！ - RhaiPlugin: Rhai 脚本运行时插件，支持动态脚本执行
//！ - LLMAgent: 集成工具调用能力的智能体

use anyhow::Result;
use mofa_sdk::llm::{LLMAgentBuilder, ToolExecutor, ToolPluginExecutor};
use mofa_sdk::plugins::rhai_runtime::{RhaiPlugin, RhaiPluginConfig};
use mofa_sdk::plugins::tools::create_builtin_tool_plugin;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, warn, Level};

// ============================================================================
// 演示 4: 基于文件的动态插件 - 支持运行时修改和自动重载
// ============================================================================

async fn demo_file_based_dynamic_plugin() -> Result<()> {
    info!("\n========== 演示 4: 基于文件的动态插件 - 支持运行时修改和自动重载 ==========\n");

    // 创建插件目录
    tokio::fs::create_dir_all("./plugins").await?;

    // 1. 创建一个初始的插件脚本文件
    let plugin_file_path = PathBuf::from("./plugins/dynamic_rules.rhai");
    let initial_script = r#"
// 动态规则引擎插件
// @name: DynamicRulesEngine
// @version: 1.0.0
// @description: 实时可配置的 LLM 规则引擎

fn execute(prompt) {
    // 规则1: 拒绝敏感词汇
    let sensitive_words = ["敏感词", "关键词", "禁止"];
    for word in sensitive_words {
        if prompt.contains(word) {
            return #{
                allowed: false,
                reason: "包含敏感词汇",
                original: prompt,
                processed: null
            };
        }
    }

    // 规则2: 检查内容长度
    if prompt.len() > 1000 {
        return #{
            allowed: false,
            reason: "内容过长",
            original: prompt,
            processed: null
        };
    }

    // 规则3: 格式化请求
    return #{
        allowed: true,
        reason: "通过所有规则",
        original: prompt,
        processed: prompt.trim()
    };
}
"#;

    // 写入初始脚本
    tokio::fs::write(&plugin_file_path, initial_script).await?;
    info!("创建了插件文件: {:?}", plugin_file_path);

    // 2. 创建文件-based Rhai 插件
    let config = RhaiPluginConfig::new_file("dynamic_rules", &plugin_file_path);
    let mut rhai_plugin = RhaiPlugin::new(config).await?;

    // 创建上下文
    let ctx = mofa_sdk::plugins::PluginContext::new("rules_engine_agent");

    // 加载并启动插件
    rhai_plugin.load(&ctx).await?;
    rhai_plugin.init_plugin().await?;
    rhai_plugin.start().await?;

    // 3. 创建文件监视器
    let _plugin_path_clone = plugin_file_path.clone();
    let mut watcher = RecommendedWatcher::new(move |res| {
        match res {
            Ok(event) => {
                info!("文件变化: {:?}", event);
                // 这里可以触发插件重载逻辑
            },
            Err(e) => warn!("文件监视错误: {:?}", e),
        }
    }, Config::default())?;

    // 监听插件文件变化
    watcher.watch(&plugin_file_path, RecursiveMode::NonRecursive)?;

    // 4. 测试插件执行
    let test_prompts: Vec<String> = vec![
        "这是一个正常的请求".to_string(),
        "这是一个包含敏感词的请求".to_string(),
        "这是一个非常长的请求 ".repeat(100),
    ];

    for (i, prompt) in test_prompts.iter().enumerate() {
        info!("\n测试 {}: {}", i + 1, prompt);
        match rhai_plugin.execute(prompt.clone()).await {
            Ok(result) => {
                info!("规则检查结果: {}", result);
            },
            Err(e) => {
                warn!("规则检查失败: {}", e);
            },
        }
    }

    // 5. 演示动态修改
    info!("\n--- 演示动态修改插件规则 ---");
    info!("现在修改插件文件 {:?} 来更新规则...", plugin_file_path);
    info!("例如，将 '敏感词' 改为 '测试敏感词'，或者添加新的规则");
    info!("等待 10 秒，期间可以修改文件...");

    // 等待10秒，让用户有时间修改文件
    time::sleep(Duration::from_secs(10)).await;

    // 6. 重载并重新测试
    info!("\n--- 重载插件并重新测试 ---");
    rhai_plugin.reload().await?;
    info!("插件已成功重载");

    // 重新测试相同的请求
    let prompt = "这是一个包含敏感词的请求";
    info!("\n测试: {}", prompt);
    match rhai_plugin.execute(prompt.to_string()).await {
        Ok(result) => {
            info!("规则检查结果: {}", result);
        },
        Err(e) => {
            warn!("规则检查失败: {}", e);
        },
    }

    // 清理
    rhai_plugin.stop().await?;
    rhai_plugin.unload().await?;
    tokio::fs::remove_file(plugin_file_path).await?;
    tokio::fs::remove_dir("./plugins").await?;

    Ok(())
}

// ============================================================================
// 演示 5: 动态规则引擎与 LLM 集成
// ============================================================================

async fn demo_dynamic_rules_engine_for_llm() -> Result<()> {
    info!("\n========== 演示 5: 动态规则引擎与 LLM 集成 ==========\n");

    // 创建插件目录和规则文件
    tokio::fs::create_dir_all("./plugins").await?;

    // 创建规则插件
    let rules_file = Path::new("./plugins/llm_rules.rhai");
    tokio::fs::write(rules_file, r#"
// LLM 响应处理规则引擎
// 这个插件将在 LLM 生成响应后执行，用于过滤和格式化响应

fn execute(llm_response) {
    let response = parse_json(llm_response);

    // 规则1: 过滤消极内容
    let negative_words = ["不好", "不行", "错误", "失败"];
    for word in negative_words {
        if response.contains(word) {
            return #{
                status: "filtered",
                reason: "包含消极内容",
                original: response,
                processed: "我无法提供相关帮助"
            };
        }
    }

    // 规则2: 确保响应积极
    return #{
        status: "ok",
        reason: "通过所有规则",
        original: response,
        processed: response
    };
}
"#).await?;

    // 创建规则插件
    let rules_config = RhaiPluginConfig::new_file("llm_rules", &rules_file.to_path_buf());
    let mut rules_plugin = RhaiPlugin::new(rules_config).await?;

    let ctx = mofa_sdk::plugins::PluginContext::new("llm_rules_agent");
    rules_plugin.load(&ctx).await?;
    rules_plugin.init_plugin().await?;
    rules_plugin.start().await?;

    // 创建工具插件和适配器
    let mut tool_plugin = create_builtin_tool_plugin("comprehensive_tools")?;
    tool_plugin.init_plugin().await?;

    let executor: Arc<dyn ToolExecutor> = Arc::new(ToolPluginExecutor::new(tool_plugin));

    // 创建 LLM Provider
    let provider = Arc::new(mofa_sdk::llm::openai::OpenAIProvider::from_env());

    // 创建带工具的 LLMAgent
    let agent = LLMAgentBuilder::new()
        .with_id("tool_calling_agent")
        .with_name("工具调用智能体")
        .with_provider(provider)
        .with_system_prompt(
            r#"你是一个强大的AI助手，可以使用计算器工具来帮助用户。"#,
        )
        .with_tool_executor(executor)
        .build();

    // 测试请求
    let question = "1 + 1 等于多少？";
    info!("\n用户: {}", question);

    match agent.ask(question).await {
        Ok(response) => {
            info!("LLM 原始响应: {}", response);

            // 应用动态规则引擎
            match rules_plugin.execute(response.clone()).await {
                Ok(rules_result) => {
                    info!("规则处理结果: {}", rules_result);
                },
                Err(e) => {
                    warn!("规则处理失败: {}", e);
                },
            }
        },
        Err(e) => {
            warn!("请求失败: {}\n", e);
        }
    }

    // 清理 - LLMAgent 会自动清理资源
    rules_plugin.stop().await?;
    rules_plugin.unload().await?;
    tokio::fs::remove_file(rules_file).await?;
    tokio::fs::remove_dir("./plugins").await?;

    Ok(())
}

// ============================================================================
// 主函数
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== MoFA 智能体插件与 Rhai 运行时演示 ===\n");

    // 演示 4: 基于文件的动态插件 - 支持运行时修改和自动重载
    match demo_file_based_dynamic_plugin().await {
        Ok(_) => info!("演示 4 完成"),
        Err(e) => warn!("演示 4 跳过或失败: {}", e),
    }

    // 演示 5: 动态规则引擎与 LLM 集成
    match demo_dynamic_rules_engine_for_llm().await {
        Ok(_) => info!("演示 5 完成"),
        Err(e) => warn!("演示 5 跳过或失败: {}", e),
    }

    info!("\n=== 演示完成 ===");
    Ok(())
}
