//! 需求澄清器 - 阶段2: 澄清需求，转换为项目文档
//! Requirement Clarifier - Phase 2: Clarify requirements and convert to project documents

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use super::types::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Type alias for clarifier function
pub type ClarifierFn = Arc<dyn Fn(&str) -> Vec<ClarificationQuestion> + Send + Sync>;

/// 需求澄清策略
/// Requirement clarification strategy
#[derive(Debug, Clone)]
pub enum ClarificationStrategy {
    /// 自动澄清（使用LLM分析）
    /// Automatic clarification (using LLM analysis)
    Automatic,
    /// 交互式澄清（需要人类确认）
    /// Interactive clarification (requires human confirmation)
    Interactive,
    /// 模板化澄清（使用预定义模板）
    /// Templated clarification (using predefined templates)
    Template(String),
}

/// 澄清问题
/// Clarification question
#[derive(Debug, Clone)]
pub struct ClarificationQuestion {
    /// 问题ID
    /// Question ID
    pub id: String,
    /// 问题内容
    /// Question content
    pub question: String,
    /// 问题类型
    /// Question type
    pub question_type: QuestionType,
    /// 可选答案（如果是选择题）
    /// Optional answers (if it is a multiple-choice question)
    pub options: Option<Vec<String>>,
    /// 默认答案
    /// Default answer
    pub default_answer: Option<String>,
    /// 是否必答
    /// Whether it is required
    pub required: bool,
}

/// 问题类型
/// Question type
#[derive(Debug, Clone)]
pub enum QuestionType {
    /// 开放性问题
    /// Open-ended question
    OpenEnded,
    /// 单选
    /// Single choice
    SingleChoice,
    /// 多选
    /// Multiple choice
    MultipleChoice,
    /// 确认（是/否）
    /// Confirmation (Yes/No)
    Confirmation,
    /// 数值范围
    /// Numeric range
    NumericRange { min: i64, max: i64 },
}

/// 澄清会话
/// Clarification session
pub struct ClarificationSession {
    /// 会话ID
    /// Session ID
    pub session_id: String,
    /// 关联的Todo ID
    /// Associated Todo ID
    pub todo_id: String,
    /// 原始想法
    /// Raw idea
    pub raw_idea: String,
    /// 已回答的问题
    /// Answered questions
    pub answered_questions: Vec<(ClarificationQuestion, String)>,
    /// 待回答的问题
    /// Pending questions
    pub pending_questions: Vec<ClarificationQuestion>,
    /// 澄清后的需求（最终产出）
    /// Clarified requirement (final output)
    pub clarified_requirement: Option<ProjectRequirement>,
}

/// 需求澄清器
/// Requirement clarifier
pub struct RequirementClarifier {
    /// 澄清策略
    /// Clarification strategy
    strategy: ClarificationStrategy,
    /// LLM提示词模板
    /// LLM prompt templates
    prompt_templates: HashMap<String, String>,
    /// 自定义澄清处理器
    /// Custom clarification handler
    clarifier_fn: Option<ClarifierFn>,
}

impl RequirementClarifier {
    /// 创建新的需求澄清器
    /// Create a new requirement clarifier
    pub fn new(strategy: ClarificationStrategy) -> Self {
        let mut prompt_templates = HashMap::new();

        prompt_templates.insert(
            "analyze_requirement".to_string(),
            r#"分析以下用户需求，提取关键信息：

用户需求：{raw_idea}

请回答以下问题：
1. 核心目标是什么？
2. 有哪些具体的功能要求？
3. 有哪些约束条件或限制？
4. 成功的验收标准是什么？
5. 是否有依赖项或前置条件？

请以JSON格式返回分析结果。"#
                .to_string(),
        );

        Self {
            strategy,
            prompt_templates,
            clarifier_fn: None,
        }
    }

    /// 设置自定义澄清处理器
    /// Set custom clarification handler
    pub fn with_custom_clarifier<F>(mut self, clarifier: F) -> Self
    where
        F: Fn(&str) -> Vec<ClarificationQuestion> + Send + Sync + 'static,
    {
        self.clarifier_fn = Some(Arc::new(clarifier));
        self
    }

    /// 添加或更新提示词模板
    /// Add or update prompt templates
    pub fn add_prompt_template(&mut self, name: &str, template: &str) {
        self.prompt_templates
            .insert(name.to_string(), template.to_string());
    }

    /// 开始澄清会话
    /// Start clarification session
    pub async fn start_session(&self, todo_id: &str, raw_idea: &str) -> ClarificationSession {
        let session_id = format!(
            "clarify_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        let pending_questions = self.generate_questions(raw_idea).await;

        ClarificationSession {
            session_id,
            todo_id: todo_id.to_string(),
            raw_idea: raw_idea.to_string(),
            answered_questions: Vec::new(),
            pending_questions,
            clarified_requirement: None,
        }
    }

    /// 生成澄清问题
    /// Generate clarification questions
    async fn generate_questions(&self, raw_idea: &str) -> Vec<ClarificationQuestion> {
        if let Some(ref clarifier) = self.clarifier_fn {
            return clarifier(raw_idea);
        }

        match &self.strategy {
            ClarificationStrategy::Automatic => self.generate_automatic_questions(raw_idea),
            ClarificationStrategy::Interactive => self.generate_interactive_questions(raw_idea),
            ClarificationStrategy::Template(template_name) => {
                self.generate_template_questions(raw_idea, template_name)
            }
        }
    }

    fn generate_automatic_questions(&self, _raw_idea: &str) -> Vec<ClarificationQuestion> {
        vec![
            ClarificationQuestion {
                id: "scope".to_string(),
                question: "Please describe the specific scope and boundaries of this requirement.".to_string(),
                // Please describe the specific scope and boundaries of this requirement.
                question_type: QuestionType::OpenEnded,
                options: None,
                default_answer: None,
                required: true,
            },
            ClarificationQuestion {
                id: "priority".to_string(),
                question: "How urgent is this requirement?".to_string(),
                // How urgent is this requirement?
                question_type: QuestionType::SingleChoice,
                options: Some(vec![
                    "Urgent (complete today)".to_string(),
                    // Urgent (complete today)
                    "High priority (complete this week)".to_string(),
                    // High priority (complete this week)
                    "Medium priority (complete this month)".to_string(),
                    // Medium priority (complete this month)
                    "Low priority (do when available)".to_string(),
                    // Low priority (do when available)
                ]),
                default_answer: Some("Medium priority (complete this month)".to_string()),
                // Medium priority (complete this month)
                required: true,
            },
            ClarificationQuestion {
                id: "acceptance".to_string(),
                question: "How will you determine that this requirement is complete? What are the acceptance criteria?".to_string(),
                // How will you determine that this requirement is complete? What are the acceptance criteria?
                question_type: QuestionType::OpenEnded,
                options: None,
                default_answer: None,
                required: true,
            },
            ClarificationQuestion {
                id: "dependencies".to_string(),
                question: "Are there any prerequisites or dependencies required to complete this requirement?".to_string(),
                // Are there any prerequisites or dependencies required to complete this requirement?
                question_type: QuestionType::OpenEnded,
                options: None,
                default_answer: Some("No special dependencies".to_string()),
                // No special dependencies
                required: false,
            },
        ]
    }

    fn generate_interactive_questions(&self, _raw_idea: &str) -> Vec<ClarificationQuestion> {
        vec![
            ClarificationQuestion {
                id: "confirm_understanding".to_string(),
                question: "I understand you want ...; is this understanding correct?".to_string(),
                // I understand you want ..., is this understanding correct?
                question_type: QuestionType::Confirmation,
                options: None,
                default_answer: None,
                required: true,
            },
            ClarificationQuestion {
                id: "additional_details".to_string(),
                question: "Are there any additional details that should be added?".to_string(),
                // Are there any additional details that should be added?
                question_type: QuestionType::OpenEnded,
                options: None,
                default_answer: None,
                required: false,
            },
        ]
    }

    fn generate_template_questions(
        &self,
        _raw_idea: &str,
        template_name: &str,
    ) -> Vec<ClarificationQuestion> {
        match template_name {
            "software_feature" => vec![
                ClarificationQuestion {
                    id: "user_story".to_string(),
                    question: "Please describe the requirement in the format \"As a ... I want ... so that ...\"".to_string(),
                    // Please describe the requirement in the format "As a ... I want ... so that ..."
                    question_type: QuestionType::OpenEnded,
                    options: None,
                    default_answer: None,
                    required: true,
                },
                ClarificationQuestion {
                    id: "affected_modules".to_string(),
                    question: "Which modules or components will this feature affect?".to_string(),
                    // Which modules or components will this feature affect?
                    question_type: QuestionType::MultipleChoice,
                    options: Some(vec![
                        "Frontend UI".to_string(),
                        // Frontend UI
                        "Backend API".to_string(),
                        // Backend API
                        "Database".to_string(),
                        // Database
                        "Third-party integration".to_string(),
                        // Third-party integration
                    ]),
                    default_answer: None,
                    required: true,
                },
            ],
            _ => self.generate_automatic_questions(_raw_idea),
        }
    }

    /// 回答问题
    /// Answer question
    pub async fn answer_question(
        &self,
        session: &mut ClarificationSession,
        question_id: &str,
        answer: &str,
    ) -> GlobalResult<()> {
        let idx = session
            .pending_questions
            .iter()
            .position(|q| q.id == question_id);

        if let Some(idx) = idx {
            let question = session.pending_questions.remove(idx);
            session
                .answered_questions
                .push((question, answer.to_string()));
            Ok(())
        } else {
            Err(GlobalError::Other(format!("Question not found: {}", question_id)))
        }
    }

    /// 完成澄清，生成需求文档
    /// Finalize clarification and generate requirement document
    pub async fn finalize_requirement(
        &self,
        session: &mut ClarificationSession,
    ) -> GlobalResult<ProjectRequirement> {
        let requirement = self.synthesize_requirement(session).await?;
        session.clarified_requirement = Some(requirement.clone());
        Ok(requirement)
    }

    async fn synthesize_requirement(
        &self,
        session: &ClarificationSession,
    ) -> GlobalResult<ProjectRequirement> {
        let mut acceptance_criteria = Vec::new();
        for (question, answer) in &session.answered_questions {
            if question.id == "acceptance" {
                for line in answer.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        acceptance_criteria.push(line.to_string());
                    }
                }
            }
        }

        if acceptance_criteria.is_empty() {
            acceptance_criteria.push("The feature works as expected".to_string());
            // Feature works as expected
            acceptance_criteria.push("No obvious errors".to_string());
            // No obvious errors
        }

        let subtasks = self.decompose_into_subtasks(&session.raw_idea);

        let mut dependencies = Vec::new();
        for (question, answer) in &session.answered_questions {
            if question.id == "dependencies" && answer != "No special dependencies" {
                dependencies.push(answer.clone());
            }
        }

        Ok(ProjectRequirement {
            title: self.generate_title(&session.raw_idea),
            description: session.raw_idea.clone(),
            acceptance_criteria,
            subtasks,
            dependencies,
            estimated_effort: None,
            resources: Vec::new(),
        })
    }

    fn generate_title(&self, raw_idea: &str) -> String {
        let title: String = raw_idea.chars().take(50).collect();
        if raw_idea.len() > 50 {
            format!("{}...", title)
        } else {
            title
        }
    }

    fn decompose_into_subtasks(&self, raw_idea: &str) -> Vec<Subtask> {
        let mut subtasks = Vec::new();
        let idea_lower = raw_idea.to_lowercase();

        if idea_lower.contains("api") || idea_lower.contains("interface") {
            subtasks.push(Subtask {
                id: "subtask_api_design".to_string(),
                description: "Design API interface specifications".to_string(),
                // Design API interface specifications
                required_capabilities: vec!["api_design".to_string()],
                order: 1,
                depends_on: Vec::new(),
            });
            subtasks.push(Subtask {
                id: "subtask_api_impl".to_string(),
                description: "Implement API interfaces".to_string(),
                // Implement API interfaces
                required_capabilities: vec!["backend".to_string()],
                order: 2,
                depends_on: vec!["subtask_api_design".to_string()],
            });
        }

        if idea_lower.contains("ui") || idea_lower.contains("interface") || idea_lower.contains("frontend")
        {
            subtasks.push(Subtask {
                id: "subtask_ui_design".to_string(),
                description: "Design UI interface".to_string(),
                // Design UI interface
                required_capabilities: vec!["ui_design".to_string()],
                order: 1,
                depends_on: Vec::new(),
            });
            subtasks.push(Subtask {
                id: "subtask_ui_impl".to_string(),
                description: "Implement UI interface".to_string(),
                // Implement UI interface
                required_capabilities: vec!["frontend".to_string()],
                order: 2,
                depends_on: vec!["subtask_ui_design".to_string()],
            });
        }

        if subtasks.is_empty() {
            subtasks.push(Subtask {
                id: "subtask_main".to_string(),
                description: raw_idea.to_string(),
                required_capabilities: vec!["general".to_string()],
                order: 1,
                depends_on: Vec::new(),
            });
        }

        subtasks
    }

    /// 快速澄清（跳过交互，直接生成需求）
    /// Quick clarification (skip interaction, generate requirements directly)
    pub async fn quick_clarify(
        &self,
        todo_id: &str,
        raw_idea: &str,
    ) -> GlobalResult<ProjectRequirement> {
        let mut session = self.start_session(todo_id, raw_idea).await;

        let pending = session.pending_questions.clone();
        for question in pending {
            let answer = question
                .default_answer
                .clone()
                .unwrap_or_else(|| "待定".to_string());
            // Pending
            self.answer_question(&mut session, &question.id, &answer)
                .await?;
        }

        self.finalize_requirement(&mut session).await
    }
}

impl Default for RequirementClarifier {
    fn default() -> Self {
        Self::new(ClarificationStrategy::Automatic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_session() {
        let clarifier = RequirementClarifier::new(ClarificationStrategy::Automatic);
        let session = clarifier.start_session("todo_1", "Build a REST API").await;

        assert_eq!(session.todo_id, "todo_1");
        assert!(!session.pending_questions.is_empty());
    }

    #[tokio::test]
    async fn test_quick_clarify() {
        let clarifier = RequirementClarifier::new(ClarificationStrategy::Automatic);
        let requirement = clarifier
            .quick_clarify("todo_1", "Build a REST API")
            .await
            .unwrap();

        assert!(!requirement.title.is_empty());
        assert!(!requirement.acceptance_criteria.is_empty());
    }
}
