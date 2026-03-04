//! 基于人类决策循环（Human-in-the-Loop）的秘书Agent模式示例
//! Example of a Secretary Agent pattern based on the Human-in-the-Loop decision cycle
//!
//! 本示例实现了一个时刻在线的秘书Agent，通过Socket全双工通信接收外部请求
//! This example implements an always-online Secretary Agent that receives requests via full-duplex Socket communication
//! 支持：
//! Supports:
//! 1. 实时Socket通信
//! 1. Real-time Socket communication
//! 2. 人类决策介入
//! 2. Human decision intervention
//! 3. 全双工消息传递
//! 3. Full-duplex message passing
//! 4. 5阶段工作流事件循环
//! 4. 5-stage workflow event loop

use mofa_sdk::secretary::{
    ChannelConnection, ChatMessage, DefaultInput, DefaultOutput,
    DefaultSecretaryBuilder,
    DispatchStrategy, LLMProvider, QueryType, ReportType, SecretaryCommand,
    SecretaryCore, TodoPriority, TodoStatus,
};
use mofa_sdk::kernel::GlobalResult;
use std::io::{BufRead, Write};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};
// =============================================================================
// 简单的Mock LLM Provider（实际项目中替换为真实的LLM）
// Simple Mock LLM Provider (replace with a real LLM in actual projects)
// =============================================================================

struct MockLLMProvider;

#[async_trait::async_trait]
impl LLMProvider for MockLLMProvider {
    fn name(&self) -> &str {
        "mock-llm"
    }

    async fn chat(&self, messages: Vec<ChatMessage>) -> GlobalResult<String> {
        // 简单的mock响应，实际项目中调用真实LLM API
        // Simple mock response; call real LLM APIs in actual projects
        let last_message = messages.last().map(|m| m.content.as_str()).unwrap_or("");

        if last_message.contains("分析") || last_message.contains("需求") {
            Ok(r#"```json
{
    "title": "项目需求分析",
    "description": "用户需求的详细分析",
    "acceptance_criteria": ["功能完整", "测试通过", "文档齐全"],
    "subtasks": [
        {
            "id": "subtask_1",
            "description": "需求分析和设计",
            "required_capabilities": ["analysis"],
            "order": 1,
            "depends_on": []
        },
        {
            "id": "subtask_2",
            "description": "开发实现",
            "required_capabilities": ["backend"],
            "order": 2,
            "depends_on": ["subtask_1"]
        }
    ],
    "dependencies": [],
    "estimated_effort": "1天"
}
```"#
            .to_string())
        } else {
            Ok("已收到您的请求，正在处理中...".to_string())
        }
    }
}

// =============================================================================
// 命令行交互处理
// Command-line interaction handling
// =============================================================================

async fn handle_command(
    input_tx: &mpsc::Sender<DefaultInput>,
    cmd: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let cmd = cmd.trim();

    // 解析命令
    // Parse command
    if cmd.is_empty() {
        return Ok(true);
    }

    match cmd {
        "help" | "h" | "?" => {
            print_help();
        }
        "exit" | "quit" | "q" => {
            info!("正在关闭秘书Agent...");
            // Shutting down the Secretary Agent...
            input_tx
                .send(DefaultInput::Command(SecretaryCommand::Shutdown))
                .await?;
            return Ok(false);
        }
        "status" | "s" => {
            input_tx.send(DefaultInput::Query(QueryType::Statistics)).await?;
        }
        "list" | "ls" => {
            input_tx
                .send(DefaultInput::Query(QueryType::ListTodos { filter: None }))
                .await?;
        }
        "pending" => {
            input_tx
                .send(DefaultInput::Query(QueryType::ListTodos {
                    filter: Some(TodoStatus::Pending),
                }))
                .await?;
        }
        "decisions" | "d" => {
            input_tx
                .send(DefaultInput::Query(QueryType::PendingDecisions))
                .await?;
        }
        "report" | "r" => {
            input_tx
                .send(DefaultInput::Command(SecretaryCommand::GenerateReport {
                    report_type: ReportType::Progress,
                }))
                .await?;
        }
        "daily" => {
            input_tx
                .send(DefaultInput::Command(SecretaryCommand::GenerateReport {
                    report_type: ReportType::DailySummary,
                }))
                .await?;
        }
        "pause" => {
            input_tx
                .send(DefaultInput::Command(SecretaryCommand::Pause))
                .await?;
        }
        "resume" => {
            input_tx
                .send(DefaultInput::Command(SecretaryCommand::Resume))
                .await?;
        }
        _ if cmd.starts_with("idea:") || cmd.starts_with("i:") => {
            let content = cmd
                .strip_prefix("idea:")
                .or_else(|| cmd.strip_prefix("i:"))
                .unwrap_or("")
                .trim();

            if content.is_empty() {
                info!("请提供想法内容，例如: idea:开发一个REST API");
                // Please provide idea content, e.g., idea:Develop a REST API
            } else {
                // 解析优先级
                // Parse priority
                let (content, priority) = parse_priority(content);

                input_tx
                    .send(DefaultInput::Idea {
                        content,
                        priority: Some(priority),
                        metadata: None,
                    })
                    .await?;
            }
        }
        _ if cmd.starts_with("clarify:") || cmd.starts_with("c:") => {
            let todo_id = cmd
                .strip_prefix("clarify:")
                .or_else(|| cmd.strip_prefix("c:"))
                .unwrap_or("")
                .trim();

            if todo_id.is_empty() {
                info!("请提供任务ID，例如: clarify:todo_1");
                // Please provide a task ID, e.g., clarify:todo_1
            } else {
                input_tx
                    .send(DefaultInput::Command(SecretaryCommand::Clarify {
                        todo_id: todo_id.to_string(),
                    }))
                    .await?;
            }
        }
        _ if cmd.starts_with("dispatch:") || cmd.starts_with("dp:") => {
            let todo_id = cmd
                .strip_prefix("dispatch:")
                .or_else(|| cmd.strip_prefix("dp:"))
                .unwrap_or("")
                .trim();

            if todo_id.is_empty() {
                info!("请提供任务ID，例如: dispatch:todo_1");
                // Please provide a task ID, e.g., dispatch:todo_1
            } else {
                input_tx
                    .send(DefaultInput::Command(SecretaryCommand::Dispatch {
                        todo_id: todo_id.to_string(),
                    }))
                    .await?;
            }
        }
        _ if cmd.starts_with("cancel:") => {
            let parts: Vec<&str> = cmd
                .strip_prefix("cancel:")
                .unwrap_or("")
                .splitn(2, ':')
                .collect();

            if parts.is_empty() || parts[0].trim().is_empty() {
                info!("请提供任务ID，例如: cancel:todo_1:原因");
                // Please provide a task ID, e.g., cancel:todo_1:reason
            } else {
                let todo_id = parts[0].trim().to_string();
                let reason = parts.get(1).map(|s| s.trim()).unwrap_or("用户取消").to_string();
                // "User cancelled"

                input_tx
                    .send(DefaultInput::Command(SecretaryCommand::Cancel {
                        todo_id,
                        reason,
                    }))
                    .await?;
            }
        }
        _ if cmd.starts_with("decide:") || cmd.starts_with("dec:") => {
            let parts: Vec<&str> = cmd
                .strip_prefix("decide:")
                .or_else(|| cmd.strip_prefix("dec:"))
                .unwrap_or("")
                .splitn(2, ':')
                .collect();

            if parts.len() < 2 {
                info!("请提供决策ID和选项，例如: decide:decision_1:0");
                // Please provide decision ID and option, e.g., decide:decision_1:0
            } else {
                let decision_id = parts[0].trim().to_string();
                let selected_option: usize = parts[1].trim().parse().unwrap_or(0);

                input_tx
                    .send(DefaultInput::Decision {
                        decision_id,
                        selected_option,
                        comment: None,
                    })
                    .await?;
            }
        }
        _ if cmd.starts_with("get:") => {
            let todo_id = cmd.strip_prefix("get:").unwrap_or("").trim();

            if todo_id.is_empty() {
                info!("请提供任务ID，例如: get:todo_1");
                // Please provide a task ID, e.g., get:todo_1
            } else {
                input_tx
                    .send(DefaultInput::Query(QueryType::GetTodo {
                        todo_id: todo_id.to_string(),
                    }))
                    .await?;
            }
        }
        _ => {
            // 默认作为新想法处理
            // Treat as a new idea by default
            let (content, priority) = parse_priority(cmd);
            input_tx
                .send(DefaultInput::Idea {
                    content,
                    priority: Some(priority),
                    metadata: None,
                })
                .await?;
        }
    }

    Ok(true)
}

/// 解析优先级标记
/// Parse priority markers
fn parse_priority(content: &str) -> (String, TodoPriority) {
    if content.ends_with("!!!") || content.contains("[urgent]") || content.contains("[紧急]") {
        (
            content
                .trim_end_matches("!!!")
                .replace("[urgent]", "")
                .as_str()
                .replace("[紧急]", "")
                .trim()
                .to_string(),
            TodoPriority::Urgent,
        )
    } else if content.ends_with("!!") || content.contains("[high]") || content.contains("[高]") {
        (
            content
                .trim_end_matches("!!")
                .replace("[high]", "")
                .as_str()
                .replace("[高]", "")
                .trim()
                .to_string(),
            TodoPriority::High,
        )
    } else if content.ends_with("!") || content.contains("[medium]") || content.contains("[中]") {
        (
            content
                .trim_end_matches("!")
                .replace("[medium]", "")
                .as_str()
                .replace("[中]", "")
                .trim()
                .to_string(),
            TodoPriority::Medium,
        )
    } else if content.contains("[low]") || content.contains("[低]") {
        (
            content.replace("[low]", "")
                .as_str()
                .replace("[低]", "").trim().to_string(),
            TodoPriority::Low,
        )
    } else {
        (content.to_string(), TodoPriority::Medium)
    }
}

/// 处理秘书输出
/// Handle secretary output
fn handle_output(output: DefaultOutput) {
    match output {
        DefaultOutput::Acknowledgment { message } => {
            info!("\n📋 {}", message);
        }
        DefaultOutput::Message { content } => {
            info!("\n💬 {}", content);
        }
        DefaultOutput::DecisionRequired { decision } => {
            info!("\n⚠️  需要您的决策！");
            // Decision required!
            info!("   决策ID: {}", decision.id);
            // Decision ID
            info!("   描述: {}", decision.description);
            // Description
            info!("   选项:");
            // Options
            for (i, opt) in decision.options.iter().enumerate() {
                let marker = if decision.recommended_option == Some(i) {
                    "★"
                } else {
                    " "
                };
                info!("   {} [{}] {}: {}", marker, i, opt.label, opt.description);
            }
            info!("   使用 'decide:{}:<选项号>' 来做出决策", decision.id);
            // Use 'decide:{}:<option_index>' to make a decision
        }
        DefaultOutput::Report { report } => {
            info!("\n📊 汇报 ({:?})", report.report_type);
            // Report
            info!("{}", "-".repeat(50));
            info!("{}", report.content);
            info!("{}", "-".repeat(50));
        }
        DefaultOutput::StatusUpdate { todo_id, status } => {
            info!("\n🔄 任务 {} 状态更新: {:?}", todo_id, status);
            // Task status update
        }
        DefaultOutput::TaskCompleted { todo_id, result } => {
            let emoji = if result.success { "✅" } else { "❌" };
            info!("\n{} 任务 {} 已完成", emoji, todo_id);
            // Task completed
            info!("   摘要: {}", result.summary);
            // Summary
        }
        DefaultOutput::Error { message } => {
            info!("\n❌ 错误: {}", message);
            // Error
        }
    }
}

/// 打印帮助信息
/// Print help information
fn print_help() {
    info!(
        r#"
╔═══════════════════════════════════════════════════════════════════╗
║                     秘书Agent命令帮助                      ║
║                     Secretary Agent Command Help          ║
╠═══════════════════════════════════════════════════════════════════╣
║ 想法/需求:                                                        ║
║ Idea/Requirement:                                                 ║
║   <内容>          直接输入内容作为新想法                           ║
║   <content>       Enter content directly as a new idea            ║
║   idea:<内容>     创建新想法 (可简写为 i:)                        ║
║   idea:<content>  Create a new idea (shorthand i:)                ║
║   <内容>!!!       紧急优先级                                      ║
║   <content>!!!    Urgent priority                                 ║
║   <内容>!!        高优先级                                        ║
║   <content>!!     High priority                                   ║
║   <内容>!         中优先级                                        ║
║   <content>!      Medium priority                                 ║
║   <内容>[low]     低优先级                                        ║
║   <content>[low]  Low priority                                    ║
║                                                                   ║
║ 任务管理:                                                         ║
║ Task Management:                                                  ║
║   list, ls        列出所有任务                                    ║
║   list, ls        List all tasks                                  ║
║   pending         列出待处理任务                                  ║
║   pending         List pending tasks                              ║
║   get:<todo_id>   查看任务详情                                    ║
║   get:<todo_id>   View task details                               ║
║   clarify:<todo_id> 澄清任务需求 (可简写为 c:)                     ║
║   clarify:<todo_id> Clarify task needs (shorthand c:)             ║
║   dispatch:<todo_id> 分配任务 (可简写为 dp:)                       ║
║   dispatch:<todo_id> Dispatch task (shorthand dp:)               ║
║   cancel:<todo_id> 取消任务                                       ║
║   cancel:<todo_id> Cancel task                                    ║
║                                                                   ║
║ 决策:                                                             ║
║ Decision:                                                         ║
║   decisions, d    查看待决策列表                                  ║
║   decisions, d    View pending decisions                          ║
║   decide:<id>:<选项> 做出决策 (可简写为 dec:)                      ║
║   decide:<id>:<opt> Make decision (shorthand dec:)                ║
║                                                                   ║
║ 汇报:                                                             ║
║ Reporting:                                                        ║
║   report, r       生成进度汇报                                    ║
║   report, r       Generate progress report                        ║
║   daily           生成每日总结                                    ║
║   daily           Generate daily summary                          ║
║   status, s       查看统计信息                                    ║
║   status, s       View statistics                                 ║
║                                                                   ║
║ 控制:                                                             ║
║ Control:                                                          ║
║   pause           暂停秘书Agent                                   ║
║   pause           Pause Secretary Agent                           ║
║   resume          恢复秘书Agent                                   ║
║   resume          Resume Secretary Agent                          ║
║   help, h, ?      显示此帮助                                      ║
║   help, h, ?      Show this help                                  ║
║   exit, quit, q   退出                                           ║
║   exit, quit, q   Exit                                            ║
╚═══════════════════════════════════════════════════════════════════╝
"#
    );
}

// =============================================================================
// 主程序
// Main Program
// =============================================================================

/// 运行秘书Agent
/// Run Secretary Agent
async fn run_secretary() -> Result<(), Box<dyn std::error::Error>> {
    // 创建通道连接
    // Create channel connection
    let (connection, input_tx, mut output_rx) = ChannelConnection::<DefaultInput, DefaultOutput>::new_pair(64);

    // 创建秘书行为
    // Create secretary behavior
    let llm = Arc::new(MockLLMProvider);

    let behavior = DefaultSecretaryBuilder::new()
        .with_name("项目秘书")
        // Project Secretary
        .with_llm(llm)
        .with_dispatch_strategy(DispatchStrategy::CapabilityFirst)
        .with_auto_clarify(true)
        .with_auto_dispatch(false)
        .build();

    // 创建秘书核心引擎
    // Create secretary core engine
    let core = SecretaryCore::new(behavior);

    // 启动事件循环
    // Start the event loop
    let (_handle, _join_handle) = core.start(connection).await;

    // 启动输出处理任务
    // Start output processing task
    let output_handle = tokio::spawn(async move {
        while let Some(output) = output_rx.recv().await {
            handle_output(output);
            print!("\n秘书> ");
            // Secretary>
            std::io::stdout().flush().ok();
        }
    });

    // 主循环 - 处理用户输入
    // Main loop - handle user input
    let stdin = std::io::stdin();
    print!("秘书> ");
    // Secretary>
    std::io::stdout().flush()?;

    for line in stdin.lock().lines() {
        match line {
            Ok(cmd) => {
                if !handle_command(&input_tx, &cmd).await? {
                    break;
                }
                print!("秘书> ");
                // Secretary>
                std::io::stdout().flush()?;
            }
            Err(e) => {
                error!("读取输入错误: {}", e);
                // Error reading input
                break;
            }
        }
    }

    // 等待任务完成
    // Wait for tasks to complete
    drop(input_tx);
    output_handle.abort();

    info!("\n秘书Agent已关闭。再见！");
    // Secretary Agent closed. Goodbye!
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mofa_foundation=info".parse()?)
                .add_directive("hitl_secretary=info".parse()?),
        )
        .init();

    info!(
        r#"
╔══════════════════════════════════════════════════════════════╗
║           秘书Agent模式 - Human-in-the-Loop                   ║
║           Secretary Agent Pattern - Human-in-the-Loop         ║
║                                                              ║
║   5阶段工作循环：                                             ║
║   5-Stage Work Cycle:                                        ║
║   1. 接收想法 → 记录并生成TODO                                ║
║   1. Receive Idea → Log and generate TODO                    ║
║   2. 澄清需求 → 转换为项目文档                                ║
║   2. Clarify Needs → Convert to project documents            ║
║   3. 调度分配 → 调用对应的执行Agent                           ║
║   3. Dispatching → Call corresponding execution Agent        ║
║   4. 监控反馈 → 推送关键决策给人类                            ║
║   4. Monitor Feedback → Push key decisions to human          ║
║   5. 验收汇报 → 更新TODO                                     ║
║   5. Acceptance Report → Update TODO                         ║
║                                                              ║
║   输入 'help' 查看可用命令                                    ║
║   Type 'help' to see available commands                      ║
╚══════════════════════════════════════════════════════════════╝
"#
    );

    run_secretary().await
}
