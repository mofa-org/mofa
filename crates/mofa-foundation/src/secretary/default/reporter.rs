//! 汇报器 - 阶段5: 验收汇报，更新Todo，生成报告
//! Reporter - Phase 5: Acceptance reporting, updating Todo, and generating reports

use super::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 汇报格式
/// Report format
#[derive(Debug, Clone)]
pub enum ReportFormat {
    /// Markdown格式
    /// Markdown format
    Markdown,
    /// 纯文本
    /// Plain text
    PlainText,
    /// JSON格式
    /// JSON format
    Json,
}

/// 汇报配置
/// Report configuration
#[derive(Debug, Clone)]
pub struct ReportConfig {
    /// 默认格式
    /// Default format
    pub default_format: ReportFormat,
    /// 是否包含统计信息
    /// Whether to include statistics
    pub include_statistics: bool,
    /// 是否包含详细信息
    /// Whether to include detailed info
    pub include_details: bool,
    /// 最大历史记录数
    /// Maximum number of history records
    pub max_history: usize,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            default_format: ReportFormat::Markdown,
            include_statistics: true,
            include_details: true,
            max_history: 100,
        }
    }
}

/// 汇报器
/// Reporter
pub struct Reporter {
    /// 配置
    /// Configuration
    config: ReportConfig,
    /// 汇报历史
    /// Report history
    history: Arc<RwLock<Vec<Report>>>,
    /// 计数器
    /// Counter
    counter: Arc<RwLock<u64>>,
}

impl Reporter {
    /// 创建新的汇报器
    /// Create a new reporter
    pub fn new(config: ReportConfig) -> Self {
        Self {
            config,
            history: Arc::new(RwLock::new(Vec::new())),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    /// 生成报告ID
    /// Generate report ID
    async fn generate_id(&self) -> String {
        let mut counter = self.counter.write().await;
        *counter += 1;
        format!("report_{}", *counter)
    }

    /// 保存报告到历史
    /// Save report to history
    async fn save_report(&self, report: Report) {
        let mut history = self.history.write().await;
        history.push(report);

        // 限制历史记录数量
        // Limit the number of history records
        while history.len() > self.config.max_history {
            history.remove(0);
        }
    }

    /// 生成任务完成汇报
    /// Generate task completion report
    pub async fn generate_completion_report(
        &self,
        todo: &TodoItem,
        result: &ExecutionResult,
    ) -> Report {
        let id = self.generate_id().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let content = match self.config.default_format {
            ReportFormat::Markdown => self.format_completion_markdown(todo, result),
            ReportFormat::PlainText => self.format_completion_plain(todo, result),
            ReportFormat::Json => self.format_completion_json(todo, result),
        };

        let mut statistics = HashMap::new();
        statistics.insert(
            "execution_time_ms".to_string(),
            serde_json::json!(result.execution_time_ms),
        );
        statistics.insert("success".to_string(), serde_json::json!(result.success));
        statistics.insert(
            "artifacts_count".to_string(),
            serde_json::json!(result.artifacts.len()),
        );

        let report = Report {
            id,
            report_type: ReportType::TaskCompletion,
            todo_ids: vec![todo.id.clone()],
            content,
            statistics,
            created_at: now,
        };

        self.save_report(report.clone()).await;
        report
    }

    fn format_completion_markdown(&self, todo: &TodoItem, result: &ExecutionResult) -> String {
        let status = if result.success {
            "✅ 成功"
            // ✅ Success
        } else {
            "❌ 失败"
            // ❌ Failure
        };

        let mut content = format!(
            "# 任务完成汇报\n\n\
             ## 基本信息\n\
             - **任务ID**: {}\n\
             - **状态**: {}\n\
             - **执行时间**: {}ms\n\n\
             ## 任务描述\n\
             {}\n\n\
             ## 执行结果\n\
             {}\n",
            todo.id, status, result.execution_time_ms, todo.raw_idea, result.summary
        );

        if self.config.include_details && !result.details.is_empty() {
            content.push_str("\n## 详细信息\n");
            // ## Detailed Information
            for (key, value) in &result.details {
                content.push_str(&format!("- **{}**: {}\n", key, value));
            }
        }

        if !result.artifacts.is_empty() {
            content.push_str("\n## 产出物\n");
            // ## Artifacts
            for artifact in &result.artifacts {
                content.push_str(&format!(
                    "- {} ({})\n",
                    artifact.name, artifact.artifact_type
                ));
            }
        }

        if let Some(ref error) = result.error {
            content.push_str(&format!("\n## 错误信息\n```\n{}\n```\n", error));
            // ## Error Message
        }

        content
    }

    fn format_completion_plain(&self, todo: &TodoItem, result: &ExecutionResult) -> String {
        let status = if result.success { "成功" } else { "失败" };
        // Status: Success / Failure

        format!(
            "任务完成汇报\n\
             ==============\n\
             任务ID: {}\n\
             状态: {}\n\
             执行时间: {}ms\n\
             任务描述: {}\n\
             执行结果: {}\n",
            todo.id, status, result.execution_time_ms, todo.raw_idea, result.summary
        )
    }

    fn format_completion_json(&self, todo: &TodoItem, result: &ExecutionResult) -> String {
        let report = serde_json::json!({
            "todo_id": todo.id,
            "success": result.success,
            "execution_time_ms": result.execution_time_ms,
            "description": todo.raw_idea,
            "summary": result.summary,
            "details": result.details,
            "artifacts": result.artifacts,
            "error": result.error,
        });

        serde_json::to_string_pretty(&report).unwrap_or_default()
    }

    /// 生成进度汇报
    /// Generate progress report
    pub async fn generate_progress_report(
        &self,
        todos: &[TodoItem],
        statistics: HashMap<String, serde_json::Value>,
    ) -> Report {
        let id = self.generate_id().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let todo_ids: Vec<String> = todos.iter().map(|t| t.id.clone()).collect();

        let content = match self.config.default_format {
            ReportFormat::Markdown => self.format_progress_markdown(todos, &statistics),
            ReportFormat::PlainText => self.format_progress_plain(todos, &statistics),
            ReportFormat::Json => self.format_progress_json(todos, &statistics),
        };

        let report = Report {
            id,
            report_type: ReportType::Progress,
            todo_ids,
            content,
            statistics,
            created_at: now,
        };

        self.save_report(report.clone()).await;
        report
    }

    fn format_progress_markdown(
        &self,
        todos: &[TodoItem],
        statistics: &HashMap<String, serde_json::Value>,
    ) -> String {
        let mut content = "# 进度汇报\n\n".to_string();
        // # Progress Report

        if self.config.include_statistics {
            content.push_str("## 统计信息\n");
            // ## Statistics
            for (key, value) in statistics {
                content.push_str(&format!("- **{}**: {}\n", key, value));
            }
            content.push('\n');
        }

        content.push_str("## 任务列表\n");
        // ## Task List
        for todo in todos {
            let status_icon = match todo.status {
                TodoStatus::Completed => "✅",
                TodoStatus::InProgress => "🔄",
                TodoStatus::Pending => "⏳",
                TodoStatus::Cancelled => "❌",
                _ => "📋",
            };
            content.push_str(&format!(
                "- {} [{}] {:?}: {}\n",
                status_icon,
                todo.id,
                todo.priority,
                todo.raw_idea.chars().take(50).collect::<String>()
            ));
        }

        content
    }

    fn format_progress_plain(
        &self,
        todos: &[TodoItem],
        statistics: &HashMap<String, serde_json::Value>,
    ) -> String {
        let mut content = "进度汇报\n========\n\n".to_string();
        // Progress Report

        if self.config.include_statistics {
            content.push_str("统计信息:\n");
            // Statistics:
            for (key, value) in statistics {
                content.push_str(&format!("  {}: {}\n", key, value));
            }
            content.push('\n');
        }

        content.push_str("任务列表:\n");
        // Task List:
        for todo in todos {
            content.push_str(&format!(
                "  - [{}] {:?} {:?}: {}\n",
                todo.id,
                todo.status,
                todo.priority,
                todo.raw_idea.chars().take(50).collect::<String>()
            ));
        }

        content
    }

    fn format_progress_json(
        &self,
        todos: &[TodoItem],
        statistics: &HashMap<String, serde_json::Value>,
    ) -> String {
        let report = serde_json::json!({
            "statistics": statistics,
            "todos": todos.iter().map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "status": format!("{:?}", t.status),
                    "priority": format!("{:?}", t.priority),
                    "description": t.raw_idea,
                })
            }).collect::<Vec<_>>(),
        });

        serde_json::to_string_pretty(&report).unwrap_or_default()
    }

    /// 生成每日总结
    /// Generate daily summary
    pub async fn generate_daily_summary(&self, todos: &[TodoItem]) -> Report {
        let id = self.generate_id().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let todo_ids: Vec<String> = todos.iter().map(|t| t.id.clone()).collect();

        // 统计
        // Statistics
        let total = todos.len();
        let completed = todos
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        let in_progress = todos
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();
        let pending = todos
            .iter()
            .filter(|t| t.status == TodoStatus::Pending)
            .count();

        let mut statistics = HashMap::new();
        statistics.insert("total".to_string(), serde_json::json!(total));
        statistics.insert("completed".to_string(), serde_json::json!(completed));
        statistics.insert("in_progress".to_string(), serde_json::json!(in_progress));
        statistics.insert("pending".to_string(), serde_json::json!(pending));

        let content = format!(
            "# 每日总结\n\n\
             ## 概览\n\
             - 总任务数: {}\n\
             - 已完成: {}\n\
             - 进行中: {}\n\
             - 待处理: {}\n\
             - 完成率: {:.1}%\n\n\
             ## 今日完成\n{}\n\
             ## 进行中\n{}\n",
            total,
            completed,
            in_progress,
            pending,
            if total > 0 {
                (completed as f64 / total as f64) * 100.0
            } else {
                0.0
            },
            todos
                .iter()
                .filter(|t| t.status == TodoStatus::Completed)
                .map(|t| format!("- {}\n", t.raw_idea.chars().take(50).collect::<String>()))
                .collect::<String>(),
            todos
                .iter()
                .filter(|t| t.status == TodoStatus::InProgress)
                .map(|t| format!("- {}\n", t.raw_idea.chars().take(50).collect::<String>()))
                .collect::<String>(),
        );

        let report = Report {
            id,
            report_type: ReportType::DailySummary,
            todo_ids,
            content,
            statistics,
            created_at: now,
        };

        self.save_report(report.clone()).await;
        report
    }

    /// 获取汇报历史
    /// Get report history
    pub async fn get_history(&self) -> Vec<Report> {
        let history = self.history.read().await;
        history.clone()
    }

    /// 按类型获取汇报
    /// Get reports by type
    pub async fn get_by_type(&self, report_type: ReportType) -> Vec<Report> {
        let history = self.history.read().await;
        history
            .iter()
            .filter(|r| r.report_type == report_type)
            .cloned()
            .collect()
    }

    /// 获取最近的汇报
    /// Get recent reports
    pub async fn get_recent(&self, count: usize) -> Vec<Report> {
        let history = self.history.read().await;
        history.iter().rev().take(count).cloned().collect()
    }
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new(ReportConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_completion_report() {
        let reporter = Reporter::new(ReportConfig::default());

        let todo = TodoItem::new("todo_1", "Build API", TodoPriority::High);
        let result = ExecutionResult {
            success: true,
            summary: "API built successfully".to_string(),
            details: HashMap::new(),
            artifacts: vec![],
            execution_time_ms: 5000,
            error: None,
        };

        let report = reporter.generate_completion_report(&todo, &result).await;

        assert_eq!(report.report_type, ReportType::TaskCompletion);
        assert!(report.content.contains("成功"));
        assert!(report.content.contains("Build API"));
    }

    #[tokio::test]
    async fn test_generate_daily_summary() {
        let reporter = Reporter::new(ReportConfig::default());

        let mut todos = vec![
            TodoItem::new("todo_1", "Task 1", TodoPriority::High),
            TodoItem::new("todo_2", "Task 2", TodoPriority::Medium),
        ];
        todos[0].status = TodoStatus::Completed;
        todos[1].status = TodoStatus::InProgress;

        let report = reporter.generate_daily_summary(&todos).await;

        assert_eq!(report.report_type, ReportType::DailySummary);
        assert!(report.content.contains("50.0%")); // 50% completion rate
    }
}
