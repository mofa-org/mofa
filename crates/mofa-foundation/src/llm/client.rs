//! LLM Client - 高级 LLM 交互封装
//!
//! 提供便捷的 LLM 交互 API，包括消息管理、工具调用循环等

use super::provider::{LLMConfig, LLMProvider};
use super::types::*;
use std::sync::Arc;

/// LLM 客户端
///
/// 提供高级 LLM 交互功能
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::llm::{LLMClient, LLMConfig, ChatMessage};
///
/// // 创建客户端
/// let client = LLMClient::new(provider);
///
/// // 简单对话
/// let response = client
///     .chat()
///     .system("You are a helpful assistant.")
///     .user("Hello!")
///     .send()
///     .await?;
///
/// info!("{}", response.content().unwrap_or_default());
/// ```
pub struct LLMClient {
    provider: Arc<dyn LLMProvider>,
    config: LLMConfig,
}

impl LLMClient {
    /// 使用 Provider 创建客户端
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            config: LLMConfig::default(),
        }
    }

    /// 使用配置创建客户端
    pub fn with_config(provider: Arc<dyn LLMProvider>, config: LLMConfig) -> Self {
        Self { provider, config }
    }

    /// 获取 Provider
    pub fn provider(&self) -> &Arc<dyn LLMProvider> {
        &self.provider
    }

    /// 获取配置
    pub fn config(&self) -> &LLMConfig {
        &self.config
    }

    /// 创建 Chat 请求构建器
    pub fn chat(&self) -> ChatRequestBuilder {
        let model = self
            .config
            .default_model
            .clone()
            .unwrap_or_else(|| self.provider.default_model().to_string());

        let mut builder = ChatRequestBuilder::new(self.provider.clone(), model);

        if let Some(temp) = self.config.default_temperature {
            builder = builder.temperature(temp);
        }
        if let Some(tokens) = self.config.default_max_tokens {
            builder = builder.max_tokens(tokens);
        }

        builder
    }

    /// 创建 Embedding 请求
    pub async fn embed(&self, input: impl Into<String>) -> LLMResult<Vec<f32>> {
        let model = self
            .config
            .default_model
            .clone()
            .unwrap_or_else(|| "text-embedding-ada-002".to_string());

        let request = EmbeddingRequest {
            model,
            input: EmbeddingInput::Single(input.into()),
            encoding_format: None,
            dimensions: None,
            user: None,
        };

        let response = self.provider.embedding(request).await?;
        response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| LLMError::Other("No embedding data returned".to_string()))
    }

    /// 批量 Embedding
    pub async fn embed_batch(&self, inputs: Vec<String>) -> LLMResult<Vec<Vec<f32>>> {
        let model = self
            .config
            .default_model
            .clone()
            .unwrap_or_else(|| "text-embedding-ada-002".to_string());

        let request = EmbeddingRequest {
            model,
            input: EmbeddingInput::Multiple(inputs),
            encoding_format: None,
            dimensions: None,
            user: None,
        };

        let response = self.provider.embedding(request).await?;
        Ok(response.data.into_iter().map(|d| d.embedding).collect())
    }

    /// 简单对话（单次问答）
    pub async fn ask(&self, question: impl Into<String>) -> LLMResult<String> {
        let response = self.chat().user(question).send().await?;

        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }

    /// 带系统提示的简单对话
    pub async fn ask_with_system(
        &self,
        system: impl Into<String>,
        question: impl Into<String>,
    ) -> LLMResult<String> {
        let response = self.chat().system(system).user(question).send().await?;

        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }
}

/// Chat 请求构建器
pub struct ChatRequestBuilder {
    provider: Arc<dyn LLMProvider>,
    request: ChatCompletionRequest,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    max_tool_rounds: u32,
}

impl ChatRequestBuilder {
    /// 创建新的构建器
    pub fn new(provider: Arc<dyn LLMProvider>, model: impl Into<String>) -> Self {
        Self {
            provider,
            request: ChatCompletionRequest::new(model),
            tool_executor: None,
            max_tool_rounds: 10,
        }
    }

    /// 添加系统消息
    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(ChatMessage::system(content));
        self
    }

    /// 添加用户消息
    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(ChatMessage::user(content));
        self
    }

    /// 添加助手消息
    pub fn assistant(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(ChatMessage::assistant(content));
        self
    }

    /// 添加消息
    pub fn message(mut self, message: ChatMessage) -> Self {
        self.request.messages.push(message);
        self
    }

    /// 添加消息列表
    pub fn messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.request.messages.extend(messages);
        self
    }

    /// 设置温度
    pub fn temperature(mut self, temp: f32) -> Self {
        self.request.temperature = Some(temp);
        self
    }

    /// 设置最大 token 数
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.request.max_tokens = Some(tokens);
        self
    }

    /// 添加工具
    pub fn tool(mut self, tool: Tool) -> Self {
        self.request.tools.get_or_insert_with(Vec::new).push(tool);
        self
    }

    /// 设置工具列表
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.request.tools = Some(tools);
        self
    }

    /// 设置工具执行器
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// 设置最大工具调用轮数
    pub fn max_tool_rounds(mut self, rounds: u32) -> Self {
        self.max_tool_rounds = rounds;
        self
    }

    /// 设置响应格式为 JSON
    pub fn json_mode(mut self) -> Self {
        self.request.response_format = Some(ResponseFormat::json());
        self
    }

    /// 设置停止序列
    pub fn stop(mut self, sequences: Vec<String>) -> Self {
        self.request.stop = Some(sequences);
        self
    }

    /// 发送请求
    pub async fn send(self) -> LLMResult<ChatCompletionResponse> {
        self.provider.chat(self.request).await
    }

    /// 发送流式请求
    pub async fn send_stream(mut self) -> LLMResult<super::provider::ChatStream> {
        self.request.stream = Some(true);
        self.provider.chat_stream(self.request).await
    }

    /// 发送请求并自动执行工具调用
    ///
    /// 当 LLM 返回工具调用时，自动执行工具并继续对话，
    /// 直到 LLM 返回最终响应或达到最大轮数
    pub async fn send_with_tools(mut self) -> LLMResult<ChatCompletionResponse> {
        let executor = self
            .tool_executor
            .take()
            .ok_or_else(|| LLMError::ConfigError("Tool executor not set".to_string()))?;

        let max_rounds = self.max_tool_rounds;
        let mut round = 0;

        loop {
            let response = self.provider.chat(self.request.clone()).await?;

            // 检查是否有工具调用
            if !response.has_tool_calls() {
                return Ok(response);
            }

            round += 1;
            if round >= max_rounds {
                return Err(LLMError::Other(format!(
                    "Max tool rounds ({}) exceeded",
                    max_rounds
                )));
            }

            // 添加助手消息（包含工具调用）
            if let Some(choice) = response.choices.first() {
                self.request.messages.push(choice.message.clone());
            }

            // 执行工具调用
            if let Some(tool_calls) = response.tool_calls() {
                for tool_call in tool_calls {
                    let result = executor
                        .execute(&tool_call.function.name, &tool_call.function.arguments)
                        .await;

                    let result_str = match result {
                        Ok(r) => r,
                        Err(e) => format!("Error: {}", e),
                    };

                    // 添加工具结果消息
                    self.request
                        .messages
                        .push(ChatMessage::tool_result(&tool_call.id, result_str));
                }
            }
        }
    }
}

/// 工具执行器 trait
///
/// 实现此 trait 以支持自动工具调用
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// 执行工具
    ///
    /// # 参数
    /// - `name`: 工具名称
    /// - `arguments`: JSON 格式的参数
    ///
    /// # 返回
    /// 工具执行结果（JSON 格式）
    async fn execute(&self, name: &str, arguments: &str) -> LLMResult<String>;

    /// 获取可用工具列表
    fn available_tools(&self) -> Vec<Tool>;
}

// ============================================================================
// 会话管理
// ============================================================================

/// 对话会话
///
/// 管理多轮对话的消息历史
pub struct ChatSession {
    /// 会话唯一标识
    session_id: uuid::Uuid,
    /// 用户 ID
    user_id: uuid::Uuid,
    /// Agent ID
    agent_id: uuid::Uuid,
    /// LLM 客户端
    client: LLMClient,
    /// 消息历史
    messages: Vec<ChatMessage>,
    /// 系统提示词
    system_prompt: Option<String>,
    /// 工具列表
    tools: Vec<Tool>,
    /// 工具执行器
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    /// 会话创建时间
    created_at: std::time::Instant,
    /// 会话元数据
    metadata: std::collections::HashMap<String, String>,
    /// 消息存储
    message_store: Arc<dyn crate::persistence::MessageStore>,
    /// 会话存储
    session_store: Arc<dyn crate::persistence::SessionStore>,
}

impl ChatSession {
    /// 创建新会话（自动生成 ID）
    pub fn new(
        client: LLMClient,
    ) -> Self {
        // 默认使用内存存储
        let store = Arc::new(crate::persistence::InMemoryStore::new());
        Self::with_id_and_stores(
            Self::generate_session_id(),
            client,
            uuid::Uuid::now_v7(), // 自动生成 user_id
            uuid::Uuid::now_v7(), // 自动生成 agent_id
            store.clone(),
            store.clone(),
        )
    }

    /// 创建新会话并指定存储实现
    pub fn new_with_stores(
        client: LLMClient,
        user_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        message_store: Arc<dyn crate::persistence::MessageStore>,
        session_store: Arc<dyn crate::persistence::SessionStore>,
    ) -> Self {
        Self::with_id_and_stores(
            Self::generate_session_id(),
            client,
            user_id,
            agent_id,
            message_store,
            session_store,
        )
    }

    /// 使用指定 UUID 创建会话
    pub fn with_id(
        session_id: uuid::Uuid,
        client: LLMClient,
    ) -> Self {
        // 默认使用内存存储
        let store = Arc::new(crate::persistence::InMemoryStore::new());
        Self {
            session_id,
            user_id: uuid::Uuid::now_v7(), // 自动生成 user_id
            agent_id: uuid::Uuid::now_v7(), // 自动生成 agent_id
            client,
            messages: Vec::new(),
            system_prompt: None,
            tools: Vec::new(),
            tool_executor: None,
            created_at: std::time::Instant::now(),
            metadata: std::collections::HashMap::new(),
            message_store: store.clone(),
            session_store: store.clone(),
        }
    }

    /// 使用指定字符串 ID 创建会话
    pub fn with_id_str(
        session_id: &str,
        client: LLMClient,
    ) -> Self {
        // 尝试将字符串解析为 UUID，如果失败则生成新的 UUID
        let session_id = uuid::Uuid::parse_str(session_id)
            .unwrap_or_else(|_| uuid::Uuid::now_v7());
        Self::with_id(session_id, client)
    }

    /// 使用指定 ID 和存储实现创建会话
    pub fn with_id_and_stores(
        session_id: uuid::Uuid,
        client: LLMClient,
        user_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        message_store: Arc<dyn crate::persistence::MessageStore>,
        session_store: Arc<dyn crate::persistence::SessionStore>,
    ) -> Self {
        Self {
            session_id,
            user_id,
            agent_id,
            client,
            messages: Vec::new(),
            system_prompt: None,
            tools: Vec::new(),
            tool_executor: None,
            created_at: std::time::Instant::now(),
            metadata: std::collections::HashMap::new(),
            message_store,
            session_store,
        }
    }

    /// 生成唯一会话 ID
    fn generate_session_id() -> uuid::Uuid {
        uuid::Uuid::now_v7()
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> uuid::Uuid {
        self.session_id
    }

    /// 获取会话 ID 字符串
    pub fn session_id_str(&self) -> String {
        self.session_id.to_string()
    }

    /// 获取会话创建时间
    pub fn created_at(&self) -> std::time::Instant {
        self.created_at
    }

    /// 从数据库加载会话
    ///
    /// 创建一个新的 ChatSession 实例，加载指定 ID 的会话和消息
    pub async fn load(
        session_id: uuid::Uuid,
        client: LLMClient,
        user_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        message_store: Arc<dyn crate::persistence::MessageStore>,
        session_store: Arc<dyn crate::persistence::SessionStore>,
    ) -> crate::persistence::PersistenceResult<Self> {
        // Load session from database
        let _db_session = session_store
            .get_session(session_id)
            .await?
            .ok_or_else(|| crate::persistence::PersistenceError::NotFound("Session not found".to_string()))?;

        // Load messages from database
        let db_messages = message_store.get_session_messages(session_id).await?;

        // Convert messages to domain format
        let mut messages = Vec::new();
        for db_msg in db_messages {
            // Convert MessageRole to Role
            let domain_role = match db_msg.role {
                crate::persistence::MessageRole::System => crate::llm::types::Role::System,
                crate::persistence::MessageRole::User => crate::llm::types::Role::User,
                crate::persistence::MessageRole::Assistant => crate::llm::types::Role::Assistant,
                crate::persistence::MessageRole::Tool => crate::llm::types::Role::Tool,
            };

            // Convert MessageContent to domain format
            let domain_content = db_msg.content.text.map(crate::llm::types::MessageContent::Text);

            // Create domain message
            let domain_msg = crate::llm::types::ChatMessage {
                role: domain_role,
                content: domain_content,
                name: None,
                tool_calls: None,
                tool_call_id: None,
            };

            // TODO: Handle tool_calls and tool_result from db_msg.content

            messages.push(domain_msg);
        }

        // Create and return ChatSession
        Ok(Self {
            session_id,
            user_id,
            agent_id,
            client,
            messages,
            system_prompt: None, // System prompt is not stored in messages
            tools: Vec::new(),   // Tools are not persisted yet
            tool_executor: None, // Tool executor is not persisted
            created_at: std::time::Instant::now(), // TODO: Convert from db_session.create_time
            metadata: std::collections::HashMap::new(), // TODO: Convert from db_session.metadata
            message_store,
            session_store,
        })
    }

    /// 获取会话存活时长
    pub fn elapsed(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// 获取元数据
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// 获取所有元数据
    pub fn metadata(&self) -> &std::collections::HashMap<String, String> {
        &self.metadata
    }

    /// 设置系统提示
    pub fn with_system(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置工具
    pub fn with_tools(mut self, tools: Vec<Tool>, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tools = tools;
        self.tool_executor = Some(executor);
        self
    }

    /// 发送消息
    pub async fn send(&mut self, content: impl Into<String>) -> LLMResult<String> {
        // 添加用户消息
        self.messages.push(ChatMessage::user(content));

        // 构建请求
        let mut builder = self.client.chat();

        // 添加系统提示
        if let Some(ref system) = self.system_prompt {
            builder = builder.system(system.clone());
        }

        // 添加历史消息
        builder = builder.messages(self.messages.clone());

        // 添加工具
        if !self.tools.is_empty() {
            builder = builder.tools(self.tools.clone());
            if let Some(ref executor) = self.tool_executor {
                builder = builder.with_tool_executor(executor.clone());
            }
        }

        // 发送请求
        let response = if self.tool_executor.is_some() {
            builder.send_with_tools().await?
        } else {
            builder.send().await?
        };

        // 提取响应内容
        let content = response
            .content()
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))?
            .to_string();

        // 添加助手消息到历史
        self.messages.push(ChatMessage::assistant(&content));

        Ok(content)
    }

    /// 获取消息历史
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// 获取消息历史（可变引用）
    pub fn messages_mut(&mut self) -> &mut Vec<ChatMessage> {
        &mut self.messages
    }

    /// 清空消息历史
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// 获取消息数量
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// 保存会话和消息到数据库
    pub async fn save(&self) -> crate::persistence::PersistenceResult<()> {
        // Convert ChatSession to persistence entity
        let db_session = crate::persistence::ChatSession::new(
            self.user_id,
            self.agent_id,
        )
        .with_id(self.session_id)
        .with_metadata("client_version", serde_json::json!("0.1.0"));

        // Save session
        self.session_store.create_session(&db_session).await?;

        // Convert and save messages
        for msg in self.messages.iter() {
            // Convert Role to MessageRole
            let persistence_role = match msg.role {
                crate::llm::types::Role::System => crate::persistence::MessageRole::System,
                crate::llm::types::Role::User => crate::persistence::MessageRole::User,
                crate::llm::types::Role::Assistant => crate::persistence::MessageRole::Assistant,
                crate::llm::types::Role::Tool => crate::persistence::MessageRole::Tool,
                crate::llm::types::Role::Function => crate::persistence::MessageRole::Tool, // Map Function to Tool role
            };

            // Convert MessageContent to persistence format
            let persistence_content = match &msg.content {
                Some(crate::llm::types::MessageContent::Text(text)) => {
                    crate::persistence::MessageContent::text(text)
                }
                Some(crate::llm::types::MessageContent::Parts(parts)) => {
                    // For now, only handle text parts
                    let text = parts.iter()
                        .filter_map(|part| {
                            if let crate::llm::types::ContentPart::Text { text } = part {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    crate::persistence::MessageContent::text(text)
                }
                None => crate::persistence::MessageContent::text(""),
            };

            let llm_message = crate::persistence::LLMMessage::new(
                self.session_id,
                self.agent_id,
                self.user_id,
                persistence_role,
                persistence_content,
            );

            // Save message
            self.message_store.save_message(&llm_message).await?;
        }

        Ok(())
    }

    /// 从数据库删除会话和消息
    pub async fn delete(&self) -> crate::persistence::PersistenceResult<()> {
        // Delete all messages for the session
        self.message_store
            .delete_session_messages(self.session_id)
            .await?;

        // Delete the session itself
        self.session_store
            .delete_session(self.session_id)
            .await?;

        Ok(())
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 快速创建函数工具定义
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::llm::function_tool;
/// use serde_json::json;
///
/// let tool = function_tool(
///     "get_weather",
///     "Get the current weather for a location",
///     json!({
///         "type": "object",
///         "properties": {
///             "location": {
///                 "type": "string",
///                 "description": "City name"
///             }
///         },
///         "required": ["location"]
///     })
/// );
/// ```
pub fn function_tool(
    name: impl Into<String>,
    description: impl Into<String>,
    parameters: serde_json::Value,
) -> Tool {
    Tool::function(name, description, parameters)
}
