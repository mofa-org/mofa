//! 标准 LLM Agent 实现
//!
//! 框架提供的开箱即用的 LLM Agent，用户只需配置 provider 即可使用
//!
//! # 示例
//!
//! ```rust,ignore
//! use mofa_sdk::{run_agent, llm::{LLMAgentBuilder, openai_from_env}};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let agent = LLMAgentBuilder::new("my-llm-agent")
//!         .with_provider(Arc::new(openai_from_env()))
//!         .with_system_prompt("You are a helpful assistant.")
//!         .build();
//!
//!     run_agent(agent).await
//! }
//! ```

use super::client::{ChatSession, LLMClient, ToolExecutor};
use super::provider::{ChatStream, LLMProvider};
use super::types::{ChatMessage, LLMError, LLMResult, Tool};
use crate::prompt;
use futures::Stream;
use mofa_kernel::agent::AgentMetadata;
use mofa_kernel::agent::AgentState;
use mofa_kernel::plugin::AgentPlugin;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 流式文本响应类型
///
/// 每次 yield 一个文本片段（delta content）
pub type TextStream = Pin<Box<dyn Stream<Item = LLMResult<String>> + Send>>;

/// 流式响应事件
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 文本片段
    Text(String),
    /// 工具调用开始
    ToolCallStart { id: String, name: String },
    /// 工具调用参数片段
    ToolCallDelta { id: String, arguments_delta: String },
    /// 完成原因
    Done(Option<String>),
}

/// LLM Agent 配置
#[derive(Clone)]
pub struct LLMAgentConfig {
    /// Agent ID
    pub agent_id: String,
    /// Agent 名称
    pub name: String,
    /// 系统提示词
    pub system_prompt: Option<String>,
    /// 默认温度
    pub temperature: Option<f32>,
    /// 默认最大 token 数
    pub max_tokens: Option<u32>,
    /// 自定义配置
    pub custom_config: HashMap<String, String>,
}

impl Default for LLMAgentConfig {
    fn default() -> Self {
        Self {
            agent_id: "llm-agent".to_string(),
            name: "LLM Agent".to_string(),
            system_prompt: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
            custom_config: HashMap::new(),
        }
    }
}

/// 标准 LLM Agent
///
/// 框架提供的开箱即用的 LLM Agent 实现
///
/// # 多会话支持
///
/// LLMAgent 支持多会话管理，每个会话有唯一的 session_id：
///
/// ```rust,ignore
/// // 创建新会话
/// let session_id = agent.create_session().await;
///
/// // 使用指定会话对话
/// agent.chat_with_session(&session_id, "Hello").await?;
///
/// // 切换默认会话
/// agent.switch_session(&session_id).await?;
///
/// // 获取所有会话ID
/// let sessions = agent.list_sessions().await;
/// ```
pub struct LLMAgent {
    config: LLMAgentConfig,
    /// 智能体元数据
    metadata: AgentMetadata,
    client: LLMClient,
    /// 多会话存储 (session_id -> ChatSession)
    sessions: Arc<RwLock<HashMap<String, Arc<RwLock<ChatSession>>>>>,
    /// 当前活动会话ID
    active_session_id: Arc<RwLock<String>>,
    tools: Vec<Tool>,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    event_handler: Option<Box<dyn LLMAgentEventHandler>>,
    /// 插件列表
    plugins: Vec<Box<dyn AgentPlugin>>,
    /// 当前智能体状态
    state: AgentState,
    /// 保存 provider 用于创建新会话
    provider: Arc<dyn LLMProvider>,
    /// Prompt 模板插件
    prompt_plugin: Option<Box<dyn prompt::PromptTemplatePlugin>>,
}

/// LLM Agent 事件处理器
///
/// 允许用户自定义事件处理逻辑
#[async_trait::async_trait]
pub trait LLMAgentEventHandler: Send + Sync {
    /// Clone this handler trait object
    fn clone_box(&self) -> Box<dyn LLMAgentEventHandler>;

    /// 处理用户消息前的钩子
    async fn before_chat(&self, message: &str) -> LLMResult<Option<String>> {
        Ok(Some(message.to_string()))
    }

    /// 处理 LLM 响应后的钩子
    async fn after_chat(&self, response: &str) -> LLMResult<Option<String>> {
        Ok(Some(response.to_string()))
    }

    /// 处理工具调用
    async fn on_tool_call(&self, name: &str, arguments: &str) -> LLMResult<Option<String>> {
        let _ = (name, arguments);
        Ok(None)
    }

    /// 处理错误
    async fn on_error(&self, error: &LLMError) -> LLMResult<Option<String>> {
        let _ = error;
        Ok(None)
    }
}

impl Clone for Box<dyn LLMAgentEventHandler> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl LLMAgent {
    /// 创建新的 LLM Agent
    pub fn new(config: LLMAgentConfig, provider: Arc<dyn LLMProvider>) -> Self {
        let client = LLMClient::new(provider.clone());
        let mut session = ChatSession::new(LLMClient::new(provider.clone()));

        if let Some(ref prompt) = config.system_prompt {
            session = session.with_system(prompt.clone());
        }

        let session_id = session.session_id().to_string();
        let session_arc = Arc::new(RwLock::new(session));

        // 初始化会话存储
        let mut sessions = HashMap::new();
        sessions.insert(session_id.clone(), session_arc);

        // Clone fields needed for metadata before moving config
        let agent_id = config.agent_id.clone();
        let name = config.name.clone();

        // 创建 AgentCapabilities
        let capabilities = mofa_kernel::agent::AgentCapabilities::builder()
            .tags(vec![
                "llm".to_string(),
                "chat".to_string(),
                "text-generation".to_string(),
                "multi-session".to_string(),
            ])
            .build();

        Self {
            config,
            metadata: AgentMetadata {
                id: agent_id,
                name,
                description: None,
                version: None,
                capabilities,
                state: AgentState::Created,
            },
            client,
            sessions: Arc::new(RwLock::new(sessions)),
            active_session_id: Arc::new(RwLock::new(session_id)),
            tools: Vec::new(),
            tool_executor: None,
            event_handler: None,
            plugins: Vec::new(),
            state: AgentState::Created,
            provider,
            prompt_plugin: None,
        }
    }

    /// 获取配置
    pub fn config(&self) -> &LLMAgentConfig {
        &self.config
    }

    /// 获取 LLM Client
    pub fn client(&self) -> &LLMClient {
        &self.client
    }

    // ========================================================================
    // 会话管理方法
    // ========================================================================

    /// 获取当前活动会话ID
    pub async fn current_session_id(&self) -> String {
        self.active_session_id.read().await.clone()
    }

    /// 创建新会话
    ///
    /// 返回新会话的 session_id
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let session_id = agent.create_session().await;
    /// agent.chat_with_session(&session_id, "Hello").await?;
    /// ```
    pub async fn create_session(&self) -> String {
        let mut session = ChatSession::new(LLMClient::new(self.provider.clone()));

        // 使用动态 Prompt 模板（如果可用）
        let mut system_prompt = self.config.system_prompt.clone();

        if let Some(ref plugin) = self.prompt_plugin
            && let Some(template) = plugin.get_current_template().await {
                // 渲染默认模板
                system_prompt = match template.render(&[]) {
                    Ok(prompt) => Some(prompt),
                    Err(_) => self.config.system_prompt.clone(),
                };
            }

        if let Some(ref prompt) = system_prompt {
            session = session.with_system(prompt.clone());
        }

        let session_id = session.session_id().to_string();
        let session_arc = Arc::new(RwLock::new(session));

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session_arc);

        session_id
    }

    /// 使用指定ID创建新会话
    ///
    /// 如果 session_id 已存在，返回错误
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let session_id = agent.create_session_with_id("user-123-session").await?;
    /// ```
    pub async fn create_session_with_id(&self, session_id: impl Into<String>) -> LLMResult<String> {
        let session_id = session_id.into();

        {
            let sessions = self.sessions.read().await;
            if sessions.contains_key(&session_id) {
                return Err(LLMError::Other(format!(
                    "Session with id '{}' already exists",
                    session_id
                )));
            }
        }

        let mut session = ChatSession::with_id_str(&session_id, LLMClient::new(self.provider.clone()));

        // 使用动态 Prompt 模板（如果可用）
        let mut system_prompt = self.config.system_prompt.clone();

        if let Some(ref plugin) = self.prompt_plugin
            && let Some(template) = plugin.get_current_template().await {
                // 渲染默认模板
                system_prompt = match template.render(&[]) {
                    Ok(prompt) => Some(prompt),
                    Err(_) => self.config.system_prompt.clone(),
                };
            }

        if let Some(ref prompt) = system_prompt {
            session = session.with_system(prompt.clone());
        }

        let session_arc = Arc::new(RwLock::new(session));

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session_arc);

        Ok(session_id)
    }

    /// 切换当前活动会话
    ///
    /// # 错误
    /// 如果 session_id 不存在则返回错误
    pub async fn switch_session(&self, session_id: &str) -> LLMResult<()> {
        let sessions = self.sessions.read().await;
        if !sessions.contains_key(session_id) {
            return Err(LLMError::Other(format!(
                "Session '{}' not found",
                session_id
            )));
        }
        drop(sessions);

        let mut active = self.active_session_id.write().await;
        *active = session_id.to_string();
        Ok(())
    }

    /// 获取或创建会话
    ///
    /// 如果 session_id 存在则返回它，否则使用该 ID 创建新会话
    pub async fn get_or_create_session(&self, session_id: impl Into<String>) -> String {
        let session_id = session_id.into();

        {
            let sessions = self.sessions.read().await;
            if sessions.contains_key(&session_id) {
                return session_id;
            }
        }

        // 会话不存在，创建新的
        let _ = self.create_session_with_id(&session_id).await;
        session_id
    }

    /// 删除会话
    ///
    /// # 注意
    /// 不能删除当前活动会话，需要先切换到其他会话
    pub async fn remove_session(&self, session_id: &str) -> LLMResult<()> {
        let active = self.active_session_id.read().await.clone();
        if active == session_id {
            return Err(LLMError::Other(
                "Cannot remove active session. Switch to another session first.".to_string(),
            ));
        }

        let mut sessions = self.sessions.write().await;
        if sessions.remove(session_id).is_none() {
            return Err(LLMError::Other(format!(
                "Session '{}' not found",
                session_id
            )));
        }

        Ok(())
    }

    /// 列出所有会话ID
    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// 获取会话数量
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// 检查会话是否存在
    pub async fn has_session(&self, session_id: &str) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(session_id)
    }

    /// 内部方法：获取会话 Arc
    async fn get_session_arc(&self, session_id: &str) -> LLMResult<Arc<RwLock<ChatSession>>> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| LLMError::Other(format!("Session '{}' not found", session_id)))
    }

    // ========================================================================
    // 对话方法
    // ========================================================================

    /// 发送消息并获取响应（使用当前活动会话）
    pub async fn chat(&self, message: impl Into<String>) -> LLMResult<String> {
        let session_id = self.active_session_id.read().await.clone();
        self.chat_with_session(&session_id, message).await
    }

    /// 使用指定会话发送消息并获取响应
    ///
    /// # 参数
    /// - `session_id`: 会话唯一标识
    /// - `message`: 用户消息
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let session_id = agent.create_session().await;
    /// let response = agent.chat_with_session(&session_id, "Hello").await?;
    /// ```
    pub async fn chat_with_session(
        &self,
        session_id: &str,
        message: impl Into<String>,
    ) -> LLMResult<String> {
        let message = message.into();

        // 调用 before_chat 钩子
        let processed_message = if let Some(ref handler) = self.event_handler {
            match handler.before_chat(&message).await? {
                Some(msg) => msg,
                None => return Ok(String::new()),
            }
        } else {
            message
        };

        // 获取会话
        let session = self.get_session_arc(session_id).await?;

        // 发送消息
        let mut session_guard = session.write().await;
        let response = match session_guard.send(&processed_message).await {
            Ok(resp) => resp,
            Err(e) => {
                if let Some(ref handler) = self.event_handler
                    && let Some(fallback) = handler.on_error(&e).await?
                {
                    return Ok(fallback);
                }
                return Err(e);
            }
        };

        // 调用 after_chat 钩子
        let final_response = if let Some(ref handler) = self.event_handler {
            match handler.after_chat(&response).await? {
                Some(resp) => resp,
                None => response,
            }
        } else {
            response
        };

        Ok(final_response)
    }

    /// 简单问答（不保留上下文）
    pub async fn ask(&self, question: impl Into<String>) -> LLMResult<String> {
        let question = question.into();

        let mut builder = self.client.chat();

        // 使用动态 Prompt 模板（如果可用）
        let mut system_prompt = self.config.system_prompt.clone();

        if let Some(ref plugin) = self.prompt_plugin
            && let Some(template) = plugin.get_current_template().await {
                // 渲染默认模板（可以根据需要添加变量）
                match template.render(&[]) {
                    Ok(prompt) => system_prompt = Some(prompt),
                    Err(_) => {
                        // 如果渲染失败，使用回退的系统提示词
                        system_prompt = self.config.system_prompt.clone();
                    }
                }
            }

        // 设置系统提示词
        if let Some(ref system) = system_prompt {
            builder = builder.system(system.clone());
        }

        if let Some(temp) = self.config.temperature {
            builder = builder.temperature(temp);
        }

        if let Some(tokens) = self.config.max_tokens {
            builder = builder.max_tokens(tokens);
        }

        builder = builder.user(question);

        // 添加工具
        if !self.tools.is_empty() {
            builder = builder.tools(self.tools.clone());
            if let Some(ref executor) = self.tool_executor {
                builder = builder.with_tool_executor(executor.clone());
                let response = builder.send_with_tools().await?;
                return response
                    .content()
                    .map(|s| s.to_string())
                    .ok_or_else(|| LLMError::Other("No content in response".to_string()));
            }
        }

        let response = builder.send().await?;
        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }

    /// 设置 Prompt 场景
    pub async fn set_prompt_scenario(&self, scenario: impl Into<String>) {
        let scenario = scenario.into();

        if let Some(ref plugin) = self.prompt_plugin {
            plugin.set_active_scenario(&scenario).await;
        }
    }

    /// 清空对话历史（当前活动会话）
    pub async fn clear_history(&self) {
        let session_id = self.active_session_id.read().await.clone();
        let _ = self.clear_session_history(&session_id).await;
    }

    /// 清空指定会话的对话历史
    pub async fn clear_session_history(&self, session_id: &str) -> LLMResult<()> {
        let session = self.get_session_arc(session_id).await?;
        let mut session_guard = session.write().await;
        session_guard.clear();
        Ok(())
    }

    /// 获取对话历史（当前活动会话）
    pub async fn history(&self) -> Vec<ChatMessage> {
        let session_id = self.active_session_id.read().await.clone();
        self.get_session_history(&session_id)
            .await
            .unwrap_or_default()
    }

    /// 获取指定会话的对话历史
    pub async fn get_session_history(&self, session_id: &str) -> LLMResult<Vec<ChatMessage>> {
        let session = self.get_session_arc(session_id).await?;
        let session_guard = session.read().await;
        Ok(session_guard.messages().to_vec())
    }

    /// 设置工具
    pub fn set_tools(&mut self, tools: Vec<Tool>, executor: Arc<dyn ToolExecutor>) {
        self.tools = tools;
        self.tool_executor = Some(executor);

        // 更新 session 中的工具
        // 注意：这需要重新创建 session，因为 with_tools 消耗 self
    }

    /// 设置事件处理器
    pub fn set_event_handler(&mut self, handler: Box<dyn LLMAgentEventHandler>) {
        self.event_handler = Some(handler);
    }

    /// 向智能体添加插件
    pub fn add_plugin<P: AgentPlugin + 'static>(&mut self, plugin: P) {
        self.plugins.push(Box::new(plugin));
    }

    /// 向智能体添加插件列表
    pub fn add_plugins(&mut self, plugins: Vec<Box<dyn AgentPlugin>>) {
        self.plugins.extend(plugins);
    }

    // ========================================================================
    // 流式对话方法
    // ========================================================================

    /// 流式问答（不保留上下文）
    ///
    /// 返回一个 Stream，每次 yield 一个文本片段
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// let mut stream = agent.ask_stream("Tell me a story").await?;
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(text) => print!("{}", text),
    ///         Err(e) => einfo!("Error: {}", e),
    ///     }
    /// }
    /// ```
    pub async fn ask_stream(&self, question: impl Into<String>) -> LLMResult<TextStream> {
        let question = question.into();

        let mut builder = self.client.chat();

        if let Some(ref system) = self.config.system_prompt {
            builder = builder.system(system.clone());
        }

        if let Some(temp) = self.config.temperature {
            builder = builder.temperature(temp);
        }

        if let Some(tokens) = self.config.max_tokens {
            builder = builder.max_tokens(tokens);
        }

        builder = builder.user(question);

        // 发送流式请求
        let chunk_stream = builder.send_stream().await?;

        // 转换为纯文本流
        Ok(Self::chunk_stream_to_text_stream(chunk_stream))
    }

    /// 流式多轮对话（保留上下文）
    ///
    /// 注意：流式对话会在收到完整响应后更新历史记录
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// let mut stream = agent.chat_stream("Hello!").await?;
    /// let mut full_response = String::new();
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(text) => {
    ///             print!("{}", text);
    ///             full_response.push_str(&text);
    ///         }
    ///         Err(e) => einfo!("Error: {}", e),
    ///     }
    /// }
    /// info!();
    /// ```
    pub async fn chat_stream(&self, message: impl Into<String>) -> LLMResult<TextStream> {
        let session_id = self.active_session_id.read().await.clone();
        self.chat_stream_with_session(&session_id, message).await
    }

    /// 使用指定会话进行流式多轮对话
    ///
    /// # 参数
    /// - `session_id`: 会话唯一标识
    /// - `message`: 用户消息
    pub async fn chat_stream_with_session(
        &self,
        session_id: &str,
        message: impl Into<String>,
    ) -> LLMResult<TextStream> {
        let message = message.into();

        // 调用 before_chat 钩子
        let processed_message = if let Some(ref handler) = self.event_handler {
            match handler.before_chat(&message).await? {
                Some(msg) => msg,
                None => return Ok(Box::pin(futures::stream::empty())),
            }
        } else {
            message
        };

        // 获取会话
        let session = self.get_session_arc(session_id).await?;

        // 获取当前历史
        let history = {
            let session_guard = session.read().await;
            session_guard.messages().to_vec()
        };

        // 构建请求
        let mut builder = self.client.chat();

        if let Some(ref system) = self.config.system_prompt {
            builder = builder.system(system.clone());
        }

        if let Some(temp) = self.config.temperature {
            builder = builder.temperature(temp);
        }

        if let Some(tokens) = self.config.max_tokens {
            builder = builder.max_tokens(tokens);
        }

        // 添加历史消息
        builder = builder.messages(history);
        builder = builder.user(processed_message.clone());

        // 发送流式请求
        let chunk_stream = builder.send_stream().await?;

        // 在流式处理前，先添加用户消息到历史
        {
            let mut session_guard = session.write().await;
            session_guard
                .messages_mut()
                .push(ChatMessage::user(&processed_message));
        }

        // 创建一个包装流，在完成时更新历史并调用事件处理
        let event_handler = self.event_handler.clone().map(Arc::new);
        let wrapped_stream = Self::create_history_updating_stream(chunk_stream, session, event_handler);

        Ok(wrapped_stream)
    }

    /// 获取原始流式响应块（包含完整信息）
    ///
    /// 如果需要访问工具调用等详细信息，使用此方法
    pub async fn ask_stream_raw(&self, question: impl Into<String>) -> LLMResult<ChatStream> {
        let question = question.into();

        let mut builder = self.client.chat();

        if let Some(ref system) = self.config.system_prompt {
            builder = builder.system(system.clone());
        }

        if let Some(temp) = self.config.temperature {
            builder = builder.temperature(temp);
        }

        if let Some(tokens) = self.config.max_tokens {
            builder = builder.max_tokens(tokens);
        }

        builder = builder.user(question);

        builder.send_stream().await
    }

    /// 流式对话并收集完整响应（使用当前活动会话）
    ///
    /// 同时返回流和一个 channel 用于获取完整响应
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// let (mut stream, full_response_rx) = agent.chat_stream_with_full("Hi").await?;
    ///
    /// while let Some(result) = stream.next().await {
    ///     if let Ok(text) = result {
    ///         print!("{}", text);
    ///     }
    /// }
    ///
    /// let full_response = full_response_rx.await?;
    /// info!("\nFull response: {}", full_response);
    /// ```
    pub async fn chat_stream_with_full(
        &self,
        message: impl Into<String>,
    ) -> LLMResult<(TextStream, tokio::sync::oneshot::Receiver<String>)> {
        let session_id = self.active_session_id.read().await.clone();
        self.chat_stream_with_full_session(&session_id, message)
            .await
    }

    /// 使用指定会话进行流式对话并收集完整响应
    ///
    /// # 参数
    /// - `session_id`: 会话唯一标识
    /// - `message`: 用户消息
    pub async fn chat_stream_with_full_session(
        &self,
        session_id: &str,
        message: impl Into<String>,
    ) -> LLMResult<(TextStream, tokio::sync::oneshot::Receiver<String>)> {
        let message = message.into();

        // 调用 before_chat 钩子
        let processed_message = if let Some(ref handler) = self.event_handler {
            match handler.before_chat(&message).await? {
                Some(msg) => msg,
                None => {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let _ = tx.send(String::new());
                    return Ok((Box::pin(futures::stream::empty()), rx));
                }
            }
        } else {
            message
        };

        // 获取会话
        let session = self.get_session_arc(session_id).await?;

        // 获取当前历史
        let history = {
            let session_guard = session.read().await;
            session_guard.messages().to_vec()
        };

        // 构建请求
        let mut builder = self.client.chat();

        if let Some(ref system) = self.config.system_prompt {
            builder = builder.system(system.clone());
        }

        if let Some(temp) = self.config.temperature {
            builder = builder.temperature(temp);
        }

        if let Some(tokens) = self.config.max_tokens {
            builder = builder.max_tokens(tokens);
        }

        builder = builder.messages(history);
        builder = builder.user(processed_message.clone());

        let chunk_stream = builder.send_stream().await?;

        // 添加用户消息到历史
        {
            let mut session_guard = session.write().await;
            session_guard
                .messages_mut()
                .push(ChatMessage::user(&processed_message));
        }

        // 创建 channel 用于传递完整响应
        let (tx, rx) = tokio::sync::oneshot::channel();

        // 创建收集完整响应的流
        let event_handler = self.event_handler.clone().map(Arc::new);
        let wrapped_stream = Self::create_collecting_stream(chunk_stream, session, tx, event_handler);

        Ok((wrapped_stream, rx))
    }

    // ========================================================================
    // 内部辅助方法
    // ========================================================================

    /// 将 chunk stream 转换为纯文本 stream
    fn chunk_stream_to_text_stream(chunk_stream: ChatStream) -> TextStream {
        use futures::StreamExt;

        let text_stream = chunk_stream.filter_map(|result| async move {
            match result {
                Ok(chunk) => {
                    // 提取文本内容
                    if let Some(choice) = chunk.choices.first()
                        && let Some(ref content) = choice.delta.content
                        && !content.is_empty()
                    {
                        return Some(Ok(content.clone()));
                    }
                    None
                }
                Err(e) => Some(Err(e)),
            }
        });

        Box::pin(text_stream)
    }

    /// 创建更新历史的流
    fn create_history_updating_stream(
        chunk_stream: ChatStream,
        session: Arc<RwLock<ChatSession>>,
        event_handler: Option<Arc<Box<dyn LLMAgentEventHandler>>>,
    ) -> TextStream {
        use futures::StreamExt;

        let collected = Arc::new(tokio::sync::Mutex::new(String::new()));
        let collected_clone = collected.clone();
        let event_handler_clone = event_handler.clone();

        let stream = chunk_stream.filter_map(move |result| {
            let collected = collected.clone();
            let event_handler = event_handler.clone();
            async move {
                match result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            // 检查是否完成
                            if choice.finish_reason.is_some() {
                                return None;
                            }
                            if let Some(ref content) = choice.delta.content
                                && !content.is_empty()
                            {
                                let mut collected = collected.lock().await;
                                collected.push_str(content);
                                return Some(Ok(content.clone()));
                            }
                        }
                        None
                    }
                    Err(e) => {
                        // 调用 on_error 钩子
                        if let Some(handler) = event_handler {
                            let _ = handler.on_error(&e).await;
                        }
                        Some(Err(e))
                    }
                }
            }
        });

        // 在流结束后更新历史并调用 after_chat 钩子
        let stream = stream
            .chain(futures::stream::once(async move {
                let full_response = collected_clone.lock().await.clone();
                if !full_response.is_empty() {
                    let mut session = session.write().await;
                    session
                        .messages_mut()
                        .push(ChatMessage::assistant(&full_response));

                    // 调用 after_chat 钩子
                    if let Some(handler) = event_handler_clone
                        && let Ok(Some(_processed_response)) = handler.after_chat(&full_response).await {
                            // 如果 after_chat 处理了响应，使用处理后的响应
                            // 由于流已经结束，我们无法再输出内容，所以只能忽略
                        }
                }
                // 返回一个空的 Ok 来结束流，但不输出
                Err(LLMError::Other("__stream_end__".to_string()))
            }))
            .filter_map(|result| async move {
                match result {
                    Ok(s) => Some(Ok(s)),
                    Err(e) if e.to_string() == "__stream_end__" => None,
                    Err(e) => Some(Err(e)),
                }
            });

        Box::pin(stream)
    }

    /// 创建收集完整响应的流
    fn create_collecting_stream(
        chunk_stream: ChatStream,
        session: Arc<RwLock<ChatSession>>,
        tx: tokio::sync::oneshot::Sender<String>,
        event_handler: Option<Arc<Box<dyn LLMAgentEventHandler>>>,
    ) -> TextStream {
        use futures::StreamExt;

        let collected = Arc::new(tokio::sync::Mutex::new(String::new()));
        let collected_clone = collected.clone();
        let event_handler_clone = event_handler.clone();

        let stream = chunk_stream.filter_map(move |result| {
            let collected = collected.clone();
            let event_handler = event_handler.clone();
            async move {
                match result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            if choice.finish_reason.is_some() {
                                return None;
                            }
                            if let Some(ref content) = choice.delta.content
                                && !content.is_empty()
                            {
                                let mut collected = collected.lock().await;
                                collected.push_str(content);
                                return Some(Ok(content.clone()));
                            }
                        }
                        None
                    }
                    Err(e) => {
                        // 调用 on_error 钩子
                        if let Some(handler) = event_handler {
                            let _ = handler.on_error(&e).await;
                        }
                        Some(Err(e))
                    }
                }
            }
        });

        // 在流结束后更新历史并发送完整响应
        let stream = stream
            .chain(futures::stream::once(async move {
                let full_response = collected_clone.lock().await.clone();
                let mut processed_response = full_response.clone();

                if !full_response.is_empty() {
                    let mut session = session.write().await;
                    session
                        .messages_mut()
                        .push(ChatMessage::assistant(&processed_response));

                    // 调用 after_chat 钩子
                    if let Some(handler) = event_handler_clone
                        && let Ok(Some(resp)) = handler.after_chat(&processed_response).await {
                            processed_response = resp;
                        }
                }

                let _ = tx.send(processed_response);

                // 返回一个空的 Ok 来结束流，但不输出
                Err(LLMError::Other("__stream_end__".to_string()))
            }))
            .filter_map(|result| async move {
                match result {
                    Ok(s) => Some(Ok(s)),
                    Err(e) if e.to_string() == "__stream_end__" => None,
                    Err(e) => Some(Err(e)),
                }
            });

        Box::pin(stream)
    }
}

/// LLM Agent 构建器
pub struct LLMAgentBuilder {
    agent_id: String,
    name: Option<String>,
    provider: Option<Arc<dyn LLMProvider>>,
    system_prompt: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    tools: Vec<Tool>,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    event_handler: Option<Box<dyn LLMAgentEventHandler>>,
    plugins: Vec<Box<dyn AgentPlugin>>,
    custom_config: HashMap<String, String>,
    prompt_plugin: Option<Box<dyn prompt::PromptTemplatePlugin>>,
}

impl LLMAgentBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            agent_id: uuid::Uuid::now_v7().to_string(),
            name: None,
            provider: None,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            tool_executor: None,
            event_handler: None,
            plugins: Vec::new(),
            custom_config: HashMap::new(),
            prompt_plugin: None,
        }
    }

    /// 设置id
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.agent_id = id.into();
        self
    }

    /// 设置名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置 LLM Provider
    pub fn with_provider(mut self, provider: Arc<dyn LLMProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 设置系统提示词
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置温度
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// 设置最大 token 数
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// 添加工具
    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    /// 设置工具列表
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    /// 设置工具执行器
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// 设置事件处理器
    pub fn with_event_handler(mut self, handler: Box<dyn LLMAgentEventHandler>) -> Self {
        self.event_handler = Some(handler);
        self
    }

    /// 添加插件
    pub fn with_plugin(mut self, plugin: impl AgentPlugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// 添加插件列表
    pub fn with_plugins(mut self, plugins: Vec<Box<dyn AgentPlugin>>) -> Self {
        self.plugins.extend(plugins);
        self
    }

    /// 设置 Prompt 模板插件
    pub fn with_prompt_plugin(mut self, plugin: impl prompt::PromptTemplatePlugin + 'static) -> Self {
        self.prompt_plugin = Some(Box::new(plugin));
        self
    }

    /// 设置支持热重载的 Prompt 模板插件
    pub fn with_hot_reload_prompt_plugin(mut self, plugin: prompt::HotReloadableRhaiPromptPlugin) -> Self {
        self.prompt_plugin = Some(Box::new(plugin));
        self
    }

    /// 添加自定义配置
    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_config.insert(key.into(), value.into());
        self
    }

    /// 构建 LLM Agent
    ///
    /// # Panics
    /// 如果未设置 provider 则 panic
    pub fn build(self) -> LLMAgent {
        let provider = self
            .provider
            .expect("LLM provider must be set before building");

        let config = LLMAgentConfig {
            agent_id: self.agent_id.clone(),
            name: self.name.unwrap_or_else(|| self.agent_id.clone()),
            system_prompt: self.system_prompt,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            custom_config: self.custom_config,
        };

        let mut agent = LLMAgent::new(config, provider);

        // 设置Prompt模板插件
        agent.prompt_plugin = self.prompt_plugin;

        if !self.tools.is_empty()
            && let Some(executor) = self.tool_executor
        {
            agent.set_tools(self.tools, executor);
        }

        if let Some(handler) = self.event_handler {
            agent.set_event_handler(handler);
        }

        // 添加插件
        agent.add_plugins(self.plugins);

        agent
    }

    /// 尝试构建 LLM Agent
    ///
    /// 如果未设置 provider 则返回错误
    pub fn try_build(self) -> LLMResult<LLMAgent> {
        let provider = self
            .provider
            .ok_or_else(|| LLMError::ConfigError("LLM provider not set".to_string()))?;

        let config = LLMAgentConfig {
            agent_id: self.agent_id.clone(),
            name: self.name.unwrap_or_else(|| self.agent_id.clone()),
            system_prompt: self.system_prompt,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            custom_config: self.custom_config,
        };

        let mut agent = LLMAgent::new(config, provider);

        if !self.tools.is_empty()
            && let Some(executor) = self.tool_executor
        {
            agent.set_tools(self.tools, executor);
        }

        if let Some(handler) = self.event_handler {
            agent.set_event_handler(handler);
        }

        // 添加插件
        agent.add_plugins(self.plugins);

        Ok(agent)
    }
}

// ============================================================================
// 从配置文件创建
// ============================================================================


impl LLMAgentBuilder {
    /// 从 agent.yml 配置文件创建 Builder
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use mofa_sdk::llm::LLMAgentBuilder;
    ///
    /// let agent = LLMAgentBuilder::from_config_file("agent.yml")?
    ///     .build();
    /// ```
    pub fn from_config_file(path: impl AsRef<std::path::Path>) -> LLMResult<Self> {
        let config = crate::config::AgentYamlConfig::from_file(path)
            .map_err(|e| LLMError::ConfigError(e.to_string()))?;
        Self::from_yaml_config(config)
    }

    /// 从 YAML 配置创建 Builder
    pub fn from_yaml_config(config: crate::config::AgentYamlConfig) -> LLMResult<Self> {
        let mut builder = Self::new().with_id(&config.agent.id).with_name(&config.agent.name);
        // 配置 LLM provider
        if let Some(llm_config) = config.llm {
            let provider = create_provider_from_config(&llm_config)?;
            builder = builder.with_provider(Arc::new(provider));

            if let Some(temp) = llm_config.temperature {
                builder = builder.with_temperature(temp);
            }
            if let Some(tokens) = llm_config.max_tokens {
                builder = builder.with_max_tokens(tokens);
            }
            if let Some(prompt) = llm_config.system_prompt {
                builder = builder.with_system_prompt(prompt);
            }
        }

        Ok(builder)
    }
}

/// 从配置创建 LLM Provider

fn create_provider_from_config(
    config: &crate::config::LLMYamlConfig,
) -> LLMResult<super::openai::OpenAIProvider> {
    use super::openai::{OpenAIConfig, OpenAIProvider};

    match config.provider.as_str() {
        "openai" => {
            let api_key = config
                .api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .ok_or_else(|| LLMError::ConfigError("OpenAI API key not set".to_string()))?;

            let mut openai_config = OpenAIConfig::new(api_key);

            if let Some(ref model) = config.model {
                openai_config = openai_config.with_model(model);
            }
            if let Some(ref base_url) = config.base_url {
                openai_config = openai_config.with_base_url(base_url);
            }
            if let Some(temp) = config.temperature {
                openai_config = openai_config.with_temperature(temp);
            }
            if let Some(tokens) = config.max_tokens {
                openai_config = openai_config.with_max_tokens(tokens);
            }

            Ok(OpenAIProvider::with_config(openai_config))
        }
        "ollama" => {
            let model = config.model.clone().unwrap_or_else(|| "llama2".to_string());
            Ok(OpenAIProvider::ollama(model))
        }
        "azure" => {
            let endpoint = config.base_url.clone().ok_or_else(|| {
                LLMError::ConfigError("Azure endpoint (base_url) not set".to_string())
            })?;
            let api_key = config
                .api_key
                .clone()
                .or_else(|| std::env::var("AZURE_OPENAI_API_KEY").ok())
                .ok_or_else(|| LLMError::ConfigError("Azure API key not set".to_string()))?;
            let deployment = config
                .deployment
                .clone()
                .or_else(|| config.model.clone())
                .ok_or_else(|| {
                    LLMError::ConfigError("Azure deployment name not set".to_string())
                })?;

            Ok(OpenAIProvider::azure(endpoint, api_key, deployment))
        }
        "compatible" | "local" => {
            let base_url = config.base_url.clone().ok_or_else(|| {
                LLMError::ConfigError("base_url not set for compatible provider".to_string())
            })?;
            let model = config
                .model
                .clone()
                .unwrap_or_else(|| "default".to_string());

            Ok(OpenAIProvider::local(base_url, model))
        }
        other => Err(LLMError::ConfigError(format!(
            "Unknown provider: {}",
            other
        ))),
    }
}

// ============================================================================
// MoFAAgent 实现 - 新的统一微内核架构
// ============================================================================

#[async_trait::async_trait]
impl mofa_kernel::agent::MoFAAgent for LLMAgent {
    fn id(&self) -> &str {
        &self.metadata.id
    }

    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn capabilities(&self) -> &mofa_kernel::agent::AgentCapabilities {
        // 将 metadata 中的 capabilities 转换为 AgentCapabilities
        // 这里需要使用一个静态的 AgentCapabilities 实例
        // 或者在 LLMAgent 中存储一个 AgentCapabilities 字段
        // 为了简化，我们创建一个基于当前 metadata 的实现
        use mofa_kernel::agent::AgentCapabilities;

        // 注意：这里返回的是一个临时引用，实际使用中可能需要调整 LLMAgent 的结构
        // 来存储一个 AgentCapabilities 实例
        // 这里我们使用一个 hack 来返回一个静态实例
        static CAPABILITIES: std::sync::OnceLock<AgentCapabilities> = std::sync::OnceLock::new();

        CAPABILITIES.get_or_init(|| {
            AgentCapabilities::builder()
                .tag("llm")
                .tag("chat")
                .tag("text-generation")
                .input_type(mofa_kernel::agent::InputType::Text)
                .output_type(mofa_kernel::agent::OutputType::Text)
                .supports_streaming(true)
                .supports_tools(true)
                .build()
        })
    }

    async fn initialize(
        &mut self,
        ctx: &mofa_kernel::agent::AgentContext,
    ) -> mofa_kernel::agent::AgentResult<()> {
        // 初始化所有插件
        for plugin in &mut self.plugins {
            plugin.init_plugin().await.map_err(|e| {
                mofa_kernel::agent::AgentError::InitializationFailed(e.to_string())
            })?;
        }
        self.state = mofa_kernel::agent::AgentState::Ready;

        // 将上下文信息保存到 metadata（如果需要）
        let _ = ctx;

        Ok(())
    }

    async fn execute(
        &mut self,
        input: mofa_kernel::agent::AgentInput,
        _ctx: &mofa_kernel::agent::AgentContext,
    ) -> mofa_kernel::agent::AgentResult<mofa_kernel::agent::AgentOutput> {
        use mofa_kernel::agent::{AgentError, AgentInput, AgentOutput};

        // 将 AgentInput 转换为字符串
        let message = match input {
            AgentInput::Text(text) => text,
            AgentInput::Json(json) => json.to_string(),
            _ => {
                return Err(AgentError::ValidationFailed(
                    "Unsupported input type for LLMAgent".to_string(),
                ))
            }
        };

        // 执行 chat
        let response = self.chat(&message).await.map_err(|e| {
            AgentError::ExecutionFailed(format!("LLM chat failed: {}", e))
        })?;

        // 将响应转换为 AgentOutput
        Ok(AgentOutput::text(response))
    }

    async fn shutdown(&mut self) -> mofa_kernel::agent::AgentResult<()> {
        // 销毁所有插件
        for plugin in &mut self.plugins {
            plugin.unload().await.map_err(|e| {
                mofa_kernel::agent::AgentError::ShutdownFailed(e.to_string())
            })?;
        }
        self.state = mofa_kernel::agent::AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> mofa_kernel::agent::AgentState {
        self.state.clone()
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 快速创建简单的 LLM Agent
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_sdk::llm::{simple_llm_agent, openai_from_env};
/// use std::sync::Arc;
///
/// let agent = simple_llm_agent(
///     "my-agent",
///     Arc::new(openai_from_env()),
///     "You are a helpful assistant."
/// );
/// ```
pub fn simple_llm_agent(
    agent_id: impl Into<String>,
    provider: Arc<dyn LLMProvider>,
    system_prompt: impl Into<String>,
) -> LLMAgent {
    LLMAgentBuilder::new()
        .with_id(agent_id)
        .with_provider(provider)
        .with_system_prompt(system_prompt)
        .build()
}

/// 从配置文件创建 LLM Agent
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_sdk::llm::agent_from_config;
///
/// let agent = agent_from_config("agent.yml")?;
/// ```

pub fn agent_from_config(path: impl AsRef<std::path::Path>) -> LLMResult<LLMAgent> {
    LLMAgentBuilder::from_config_file(path)?.try_build()
}
