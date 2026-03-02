//! Prompt 构建器
//! Prompt Builder
//!
//! 提供链式 API 构建复杂的 Prompt 消息序列
//! Provides a fluent API for building complex Prompt message sequences

use super::template::{PromptError, PromptResult, PromptTemplate};
use crate::llm::types::{ChatMessage, MessageContent, Role};
use std::collections::HashMap;

/// 消息条目
/// Message entry
#[derive(Debug, Clone)]
struct MessageEntry {
    /// 消息角色
    /// Message role
    role: Role,
    /// 原始内容（可能包含变量）
    /// Original content (may contain variables)
    content: String,
    /// 消息名称
    /// Message name
    name: Option<String>,
}

/// Prompt 构建器
/// Prompt Builder
///
/// 链式构建多消息 Prompt，支持变量替换
/// Fluent builder for multi-message Prompts, supporting variable replacement
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::prompt::PromptBuilder;
///
/// let messages = PromptBuilder::new()
///     .system("你是一个专业的{role}。")
///     .user("请帮我{task}。")
///     .with_var("role", "代码审查专家")
///     .with_var("task", "审查这段代码")
///     .build()?;
/// ```
#[derive(Default)]
pub struct PromptBuilder {
    /// 消息列表
    /// Message list
    messages: Vec<MessageEntry>,
    /// 变量映射
    /// Variable mapping
    variables: HashMap<String, String>,
}

impl PromptBuilder {
    /// 创建新的构建器
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加系统消息
    /// Add a system message
    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.messages.push(MessageEntry {
            role: Role::System,
            content: content.into(),
            name: None,
        });
        self
    }

    /// 添加用户消息
    /// Add a user message
    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.messages.push(MessageEntry {
            role: Role::User,
            content: content.into(),
            name: None,
        });
        self
    }

    /// 添加助手消息
    /// Add an assistant message
    pub fn assistant(mut self, content: impl Into<String>) -> Self {
        self.messages.push(MessageEntry {
            role: Role::Assistant,
            content: content.into(),
            name: None,
        });
        self
    }

    /// 添加带名称的用户消息
    /// Add a named user message
    pub fn user_with_name(mut self, name: impl Into<String>, content: impl Into<String>) -> Self {
        self.messages.push(MessageEntry {
            role: Role::User,
            content: content.into(),
            name: Some(name.into()),
        });
        self
    }

    /// 添加带名称的助手消息
    /// Add a named assistant message
    pub fn assistant_with_name(
        mut self,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        self.messages.push(MessageEntry {
            role: Role::Assistant,
            content: content.into(),
            name: Some(name.into()),
        });
        self
    }

    /// 添加自定义角色消息
    /// Add a custom role message
    pub fn message(mut self, role: Role, content: impl Into<String>) -> Self {
        self.messages.push(MessageEntry {
            role,
            content: content.into(),
            name: None,
        });
        self
    }

    /// 添加变量
    /// Add a variable
    pub fn with_var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(name.into(), value.into());
        self
    }

    /// 批量添加变量
    /// Add variables in batch
    pub fn with_vars<K, V>(mut self, vars: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        for (k, v) in vars {
            self.variables.insert(k.into(), v.into());
        }
        self
    }

    /// 使用模板添加系统消息
    /// Add a system message using a template
    pub fn system_template(self, template: &PromptTemplate) -> Self {
        self.system(&template.content)
    }

    /// 使用模板添加用户消息
    /// Add a user message using a template
    pub fn user_template(self, template: &PromptTemplate) -> Self {
        self.user(&template.content)
    }

    /// 使用模板添加助手消息
    /// Add an assistant message using a template
    pub fn assistant_template(self, template: &PromptTemplate) -> Self {
        self.assistant(&template.content)
    }

    /// 替换变量
    /// Replace variables
    fn render_content(&self, content: &str) -> PromptResult<String> {
        let mut result = content.to_string();

        // 查找所有变量
        // Find all variables
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        let mut missing = Vec::new();

        for cap in re.captures_iter(content) {
            let var_name = &cap[1];
            if let Some(value) = self.variables.get(var_name) {
                let placeholder = format!("{{{}}}", var_name);
                result = result.replace(&placeholder, value);
            } else {
                missing.push(var_name.to_string());
            }
        }

        // 如果有缺失的变量，报错
        // If there are missing variables, return an error
        if !missing.is_empty() {
            return Err(PromptError::MissingVariable(missing.join(", ")));
        }

        Ok(result)
    }

    /// 构建消息列表
    /// Build the message list
    pub fn build(self) -> PromptResult<Vec<ChatMessage>> {
        let mut messages = Vec::with_capacity(self.messages.len());

        for entry in &self.messages {
            let content = self.render_content(&entry.content)?;

            let mut message = match entry.role {
                Role::System => ChatMessage::system(content),
                Role::User => ChatMessage::user(content),
                Role::Assistant => ChatMessage::assistant(content),
                Role::Tool => ChatMessage {
                    role: Role::Tool,
                    content: Some(MessageContent::Text(content)),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            };

            if let Some(ref name) = entry.name {
                message.name = Some(name.clone());
            }

            messages.push(message);
        }

        Ok(messages)
    }

    /// 构建为单个字符串（用分隔符连接）
    /// Build into a single string (connected with a separator)
    pub fn build_string(self, separator: &str) -> PromptResult<String> {
        let mut parts = Vec::with_capacity(self.messages.len());

        for entry in &self.messages {
            let content = self.render_content(&entry.content)?;
            parts.push(content);
        }

        Ok(parts.join(separator))
    }

    /// 部分构建（不验证变量）
    /// Partial build (does not validate variables)
    pub fn build_partial(self) -> Vec<ChatMessage> {
        self.messages
            .into_iter()
            .map(|entry| {
                let mut content = entry.content;

                // 尝试替换变量，但不报错
                // Try to replace variables without returning errors
                for (var_name, value) in &self.variables {
                    let placeholder = format!("{{{}}}", var_name);
                    content = content.replace(&placeholder, value);
                }

                let mut message = match entry.role {
                    Role::System => ChatMessage::system(content),
                    Role::User => ChatMessage::user(content),
                    Role::Assistant => ChatMessage::assistant(content),
                    _ => ChatMessage::user(content),
                };

                if let Some(name) = entry.name {
                    message.name = Some(name);
                }

                message
            })
            .collect()
    }

    /// 检查是否包含某个变量
    /// Check if a variable is included
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// 获取所有需要的变量名
    /// Get all required variable names
    pub fn required_variables(&self) -> Vec<String> {
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        let mut vars = std::collections::HashSet::new();

        for entry in &self.messages {
            for cap in re.captures_iter(&entry.content) {
                vars.insert(cap[1].to_string());
            }
        }

        vars.into_iter().collect()
    }

    /// 获取缺失的变量
    /// Get missing variables
    pub fn missing_variables(&self) -> Vec<String> {
        self.required_variables()
            .into_iter()
            .filter(|v| !self.variables.contains_key(v))
            .collect()
    }

    /// 消息数量
    /// Message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// 是否为空
    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// 清空消息
    /// Clear messages
    pub fn clear_messages(mut self) -> Self {
        self.messages.clear();
        self
    }

    /// 清空变量
    /// Clear variables
    pub fn clear_variables(mut self) -> Self {
        self.variables.clear();
        self
    }
}

/// 对话构建器（支持多轮对话）
/// Conversation builder (supports multi-turn dialogue)
pub struct ConversationBuilder {
    /// 系统提示
    /// System prompt
    system_prompt: Option<String>,
    /// 对话历史
    /// Dialogue history
    history: Vec<(Role, String)>,
    /// 变量
    /// Variables
    variables: HashMap<String, String>,
    /// 最大历史长度
    /// Maximum history length
    max_history: Option<usize>,
}

impl Default for ConversationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationBuilder {
    /// 创建新的对话构建器
    /// Create a new conversation builder
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            history: Vec::new(),
            variables: HashMap::new(),
            max_history: None,
        }
    }

    /// 设置系统提示
    /// Set system prompt
    pub fn system(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置最大历史长度
    /// Set maximum history length
    pub fn max_history(mut self, max: usize) -> Self {
        self.max_history = Some(max);
        self
    }

    /// 添加用户消息
    /// Add a user message
    pub fn add_user(&mut self, content: impl Into<String>) {
        self.history.push((Role::User, content.into()));
        self.trim_history();
    }

    /// 添加助手消息
    /// Add an assistant message
    pub fn add_assistant(&mut self, content: impl Into<String>) {
        self.history.push((Role::Assistant, content.into()));
        self.trim_history();
    }

    /// 设置变量
    /// Set a variable
    pub fn set_var(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(name.into(), value.into());
    }

    /// 裁剪历史
    /// Trim history
    fn trim_history(&mut self) {
        if let Some(max) = self.max_history {
            while self.history.len() > max {
                self.history.remove(0);
            }
        }
    }

    /// 构建消息列表
    /// Build the message list
    pub fn build(&self) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // 添加系统提示
        // Add system prompt
        if let Some(ref system) = self.system_prompt {
            let mut content = system.clone();
            for (name, value) in &self.variables {
                content = content.replace(&format!("{{{}}}", name), value);
            }
            messages.push(ChatMessage::system(content));
        }

        // 添加历史
        // Add history
        for (role, content) in &self.history {
            let message = match role {
                Role::User => ChatMessage::user(content),
                Role::Assistant => ChatMessage::assistant(content),
                _ => continue,
            };
            messages.push(message);
        }

        messages
    }

    /// 构建并添加新的用户消息
    /// Build and add a new user message
    pub fn build_with_user(&mut self, user_message: impl Into<String>) -> Vec<ChatMessage> {
        self.add_user(user_message);
        self.build()
    }

    /// 清空历史
    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 历史长度
    /// History length
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let messages = PromptBuilder::new()
            .system("You are a helpful assistant.")
            .user("Hello!")
            .build()
            .unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(messages[1].role, Role::User);
    }

    #[test]
    fn test_builder_with_vars() {
        let messages = PromptBuilder::new()
            .system("You are a {role} assistant.")
            .user("Help me with {task}.")
            .with_var("role", "professional")
            .with_var("task", "coding")
            .build()
            .unwrap();

        assert_eq!(
            messages[0].text_content().unwrap(),
            "You are a professional assistant."
        );
        assert_eq!(messages[1].text_content().unwrap(), "Help me with coding.");
    }

    #[test]
    fn test_builder_missing_var() {
        let result = PromptBuilder::new().user("Hello, {name}!").build();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PromptError::MissingVariable(_)
        ));
    }

    #[test]
    fn test_builder_partial() {
        let messages = PromptBuilder::new()
            .user("Hello, {name}! Welcome to {place}.")
            .with_var("name", "Alice")
            .build_partial();

        // 部分替换：name 被替换，place 保留
        // Partial replacement: name is replaced, place is kept
        assert_eq!(
            messages[0].text_content().unwrap(),
            "Hello, Alice! Welcome to {place}."
        );
    }

    #[test]
    fn test_builder_string() {
        let result = PromptBuilder::new()
            .system("Line 1")
            .user("Line 2")
            .assistant("Line 3")
            .build_string("\n")
            .unwrap();

        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_required_variables() {
        let builder = PromptBuilder::new()
            .system("You are a {role}.")
            .user("{task} with {context}");

        let required = builder.required_variables();
        assert_eq!(required.len(), 3);
        assert!(required.contains(&"role".to_string()));
        assert!(required.contains(&"task".to_string()));
        assert!(required.contains(&"context".to_string()));
    }

    #[test]
    fn test_missing_variables() {
        let builder = PromptBuilder::new()
            .user("{a} {b} {c}")
            .with_var("a", "value_a");

        let missing = builder.missing_variables();
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&"b".to_string()));
        assert!(missing.contains(&"c".to_string()));
    }

    #[test]
    fn test_conversation_builder() {
        let mut conv = ConversationBuilder::new()
            .system("You are {role}.")
            .max_history(4);

        conv.set_var("role", "a helpful assistant");

        conv.add_user("Hello!");
        conv.add_assistant("Hi! How can I help?");
        conv.add_user("What is Rust?");

        let messages = conv.build();

        assert_eq!(messages.len(), 4); // system + 3 history
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(
            messages[0].text_content().unwrap(),
            "You are a helpful assistant."
        );
    }

    #[test]
    fn test_conversation_max_history() {
        let mut conv = ConversationBuilder::new().max_history(2);

        conv.add_user("Message 1");
        conv.add_assistant("Response 1");
        conv.add_user("Message 2");
        conv.add_assistant("Response 2");
        conv.add_user("Message 3");

        // 应该只保留最后 2 条
        // Should only keep the last 2 entries
        assert_eq!(conv.history_len(), 2);

        let messages = conv.build();
        assert_eq!(messages.len(), 2);
    }
}
