//! LLM Client - 高级 LLM 交互封装
//! LLM Client - High-level LLM interaction wrapper
//!
//! 提供便捷的 LLM 交互 API，包括消息管理、工具调用循环等
//! Provides convenient LLM interaction APIs, including message management, tool call loops, etc.

use super::provider::{LLMConfig, LLMProvider};
use super::tool_executor::ToolExecutor;
use super::types::*;
use std::sync::Arc;

/// LLM 客户端
/// LLM Client
///
/// 提供高级 LLM 交互功能
/// Provides high-level LLM interaction features
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::{LLMClient, LLMConfig, ChatMessage};
///
/// // 创建客户端
/// // Create client
/// let client = LLMClient::new(provider);
///
/// // 简单对话
/// // Simple chat
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
    /// Create client using a Provider
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            config: LLMConfig::default(),
        }
    }

    /// 使用配置创建客户端
    /// Create client with configuration
    pub fn with_config(provider: Arc<dyn LLMProvider>, config: LLMConfig) -> Self {
        Self { provider, config }
    }

    /// 获取 Provider
    /// Get Provider
    pub fn provider(&self) -> &Arc<dyn LLMProvider> {
        &self.provider
    }

    /// 获取配置
    /// Get configuration
    pub fn config(&self) -> &LLMConfig {
        &self.config
    }

    /// 创建 Chat 请求构建器
    /// Create Chat request builder
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
    /// Create Embedding request
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
    /// Batch Embedding
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
    /// Simple dialogue (Single Q&A)
    pub async fn ask(&self, question: impl Into<String>) -> LLMResult<String> {
        let response = self.chat().user(question).send().await?;

        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }

    /// 带系统提示的简单对话
    /// Simple dialogue with system prompt
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
/// Chat Request Builder
pub struct ChatRequestBuilder {
    provider: Arc<dyn LLMProvider>,
    request: ChatCompletionRequest,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    max_tool_rounds: u32,
    // Retry configuration
    retry_policy: Option<LLMRetryPolicy>,
    retry_enabled: bool,
}

impl ChatRequestBuilder {
    /// 创建新的构建器
    /// Create a new builder
    pub fn new(provider: Arc<dyn LLMProvider>, model: impl Into<String>) -> Self {
        Self {
            provider,
            request: ChatCompletionRequest::new(model),
            tool_executor: None,
            max_tool_rounds: 10,
            retry_policy: None,
            retry_enabled: false,
        }
    }

    /// 添加系统消息
    /// Add system message
    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(ChatMessage::system(content));
        self
    }

    /// 添加用户消息
    /// Add user message
    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(ChatMessage::user(content));
        self
    }

    /// 添加用户消息（结构化内容）
    /// Add user message (structured content)
    pub fn user_with_content(mut self, content: MessageContent) -> Self {
        self.request
            .messages
            .push(ChatMessage::user_with_content(content));
        self
    }

    /// 添加用户消息（多部分内容）
    /// Add user message (multi-part content)
    pub fn user_with_parts(mut self, parts: Vec<ContentPart>) -> Self {
        self.request
            .messages
            .push(ChatMessage::user_with_parts(parts));
        self
    }

    /// 添加助手消息
    /// Add assistant message
    pub fn assistant(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(ChatMessage::assistant(content));
        self
    }

    /// 添加消息
    /// Add message
    pub fn message(mut self, message: ChatMessage) -> Self {
        self.request.messages.push(message);
        self
    }

    /// 添加消息列表
    /// Add message list
    pub fn messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.request.messages.extend(messages);
        self
    }

    /// 设置温度
    /// Set temperature
    pub fn temperature(mut self, temp: f32) -> Self {
        self.request.temperature = Some(temp);
        self
    }

    /// 设置最大 token 数
    /// Set maximum tokens
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.request.max_tokens = Some(tokens);
        self
    }

    /// 添加工具
    /// Add tool
    pub fn tool(mut self, tool: Tool) -> Self {
        self.request.tools.get_or_insert_with(Vec::new).push(tool);
        self
    }

    /// 设置工具列表
    /// Set tool list
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.request.tools = Some(tools);
        self
    }

    /// 设置工具执行器
    /// Set tool executor
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// 设置最大工具调用轮数
    /// Set maximum tool call rounds
    pub fn max_tool_rounds(mut self, rounds: u32) -> Self {
        self.max_tool_rounds = rounds;
        self
    }

    /// 设置响应格式为 JSON
    /// Set response format to JSON
    pub fn json_mode(mut self) -> Self {
        self.request.response_format = Some(ResponseFormat::json());
        self
    }

    /// 设置停止序列
    /// Set stop sequences
    pub fn stop(mut self, sequences: Vec<String>) -> Self {
        self.request.stop = Some(sequences);
        self
    }

    // ========================================================================
    // Retry Configuration
    // ========================================================================

    /// Enable retry with default policy
    ///
    /// Uses PromptRetry strategy for serialization errors (best for JSON mode)
    /// and DirectRetry for network/transient errors.
    ///
    /// # Example
    /// ```rust,ignore
    /// let response = client.chat()
    ///     .json_mode()
    ///     .with_retry()
    ///     .send()
    ///     .await?;
    /// ```
    pub fn with_retry(mut self) -> Self {
        self.retry_enabled = true;
        self.retry_policy = Some(LLMRetryPolicy::default());
        self
    }

    /// Enable retry with custom policy
    ///
    /// # Example
    /// ```rust,ignore
    /// use mofa_foundation::llm::{LLMRetryPolicy, BackoffStrategy};
    ///
    /// let custom_policy = LLMRetryPolicy {
    ///     max_attempts: 5,
    ///     backoff: BackoffStrategy::ExponentialWithJitter {
    ///         initial_delay_ms: 500,
    ///         max_delay_ms: 60000,
    ///         jitter_ms: 250,
    ///     },
    ///     ..Default::default()
    /// };
    ///
    /// let response = client.chat()
    ///     .with_retry_policy(custom_policy)
    ///     .send()
    ///     .await?;
    /// ```
    pub fn with_retry_policy(mut self, policy: LLMRetryPolicy) -> Self {
        self.retry_enabled = true;
        self.retry_policy = Some(policy);
        self
    }

    /// Disable retry (explicit)
    ///
    /// This is the default behavior, but can be used to override
    /// any previously set retry configuration.
    pub fn without_retry(mut self) -> Self {
        self.retry_enabled = false;
        self.retry_policy = None;
        self
    }

    /// Set max retry attempts (convenience method)
    ///
    /// Shortcut for setting max_attempts in the default retry policy.
    /// Equivalent to `.with_retry_policy(LLMRetryPolicy::with_max_attempts(n))`
    ///
    /// # Example
    /// ```rust,ignore
    /// let response = client.chat()
    ///     .json_mode()
    ///     .max_retries(3)
    ///     .send()
    ///     .await?;
    /// ```
    pub fn max_retries(mut self, max: u32) -> Self {
        if self.retry_policy.is_none() {
            self.retry_policy = Some(LLMRetryPolicy::default());
        }
        if let Some(ref mut policy) = self.retry_policy {
            policy.max_attempts = max;
        }
        self.retry_enabled = true;
        self
    }

    /// 发送请求
    /// Send request
    pub async fn send(self) -> LLMResult<ChatCompletionResponse> {
        if self.retry_enabled {
            let policy = self.retry_policy.unwrap_or_default();
            let executor = crate::llm::retry::RetryExecutor::new(self.provider, policy);
            executor.chat(self.request).await
        } else {
            self.provider.chat(self.request).await
        }
    }

    /// 发送流式请求
    /// Send streaming request
    pub async fn send_stream(mut self) -> LLMResult<super::provider::ChatStream> {
        self.request.stream = Some(true);
        self.provider.chat_stream(self.request).await
    }

    /// 发送请求并自动执行工具调用
    /// Send request and automatically execute tool calls
    ///
    /// 当 LLM 返回工具调用时，自动执行工具并继续对话，
    /// 直到 LLM 返回最终响应或达到最大轮数
    /// When LLM returns tool calls, execute them and continue the chat
    /// until final response or max rounds are reached
    pub async fn send_with_tools(mut self) -> LLMResult<ChatCompletionResponse> {
        let executor = self
            .tool_executor
            .take()
            .ok_or_else(|| LLMError::ConfigError("Tool executor not set".to_string()))?;

        if self
            .request
            .tools
            .as_ref()
            .map(|tools| tools.is_empty())
            .unwrap_or(true)
        {
            let tools = executor.available_tools().await?;
            if !tools.is_empty() {
                self.request.tools = Some(tools);
            }
        }

        let max_rounds = self.max_tool_rounds;
        let mut round = 0;

        loop {
            let response = self.provider.chat(self.request.clone()).await?;

            // 检查是否有工具调用
            // Check for tool calls
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
            // Add assistant message (containing tool calls)
            if let Some(choice) = response.choices.first() {
                self.request.messages.push(choice.message.clone());
            }

            // 执行工具调用
            // Execute tool calls
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
                    // Add tool result message
                    self.request
                        .messages
                        .push(ChatMessage::tool_result(&tool_call.id, result_str));
                }
            }
        }
    }
}

// ============================================================================
// 会话管理
// Session Management
// ============================================================================

/// 对话会话
/// Dialogue Session
///
/// 管理多轮对话的消息历史
/// Manages message history for multi-turn conversations
pub struct ChatSession {
    /// 会话唯一标识
    /// Unique session identifier
    session_id: uuid::Uuid,
    /// 用户 ID
    /// User ID
    user_id: uuid::Uuid,
    /// Agent ID
    /// Agent ID
    agent_id: uuid::Uuid,
    /// 租户 ID
    /// Tenant ID
    tenant_id: uuid::Uuid,
    /// LLM 客户端
    /// LLM Client
    client: LLMClient,
    /// 消息历史
    /// Message history
    messages: Vec<ChatMessage>,
    /// 系统提示词
    /// System prompt words
    system_prompt: Option<String>,
    /// 工具列表
    /// Tool list
    tools: Vec<Tool>,
    /// 工具执行器
    /// Tool executor
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    /// 会话创建时间
    /// Session creation time
    created_at: chrono::DateTime<chrono::Utc>,
    /// 会话元数据
    /// Session metadata
    metadata: std::collections::HashMap<String, String>,
    /// 消息存储
    /// Message storage
    message_store: Arc<dyn crate::persistence::MessageStore>,
    /// 会话存储
    /// Session storage
    session_store: Arc<dyn crate::persistence::SessionStore>,
    /// 上下文窗口大小（滑动窗口，限制对话轮数）
    /// Context window size (sliding window, limits conversation rounds)
    context_window_size: Option<usize>,
    /// 最后一次 LLM 响应的元数据
    /// Metadata of the last LLM response
    last_response_metadata: Option<super::types::LLMResponseMetadata>,
}

impl ChatSession {
    /// 创建新会话（自动生成 ID）
    /// Create new session (auto-generate ID)
    pub fn new(client: LLMClient) -> Self {
        // 默认使用内存存储
        // Uses in-memory storage by default
        let store = Arc::new(crate::persistence::InMemoryStore::new());
        Self::with_id_and_stores(
            Self::generate_session_id(),
            client,
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
            store.clone(),
            store.clone(),
            None,
        )
    }

    /// 创建新会话并指定存储实现
    /// Create new session with specified storage implementations
    pub fn new_with_stores(
        client: LLMClient,
        user_id: uuid::Uuid,
        tenant_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        message_store: Arc<dyn crate::persistence::MessageStore>,
        session_store: Arc<dyn crate::persistence::SessionStore>,
    ) -> Self {
        Self::with_id_and_stores(
            Self::generate_session_id(),
            client,
            user_id,
            tenant_id,
            agent_id,
            message_store,
            session_store,
            None,
        )
    }

    /// 使用指定 UUID 创建会话
    /// Create session with specified UUID
    pub fn with_id(session_id: uuid::Uuid, client: LLMClient) -> Self {
        // 默认使用内存存储
        // Uses in-memory storage by default
        let store = Arc::new(crate::persistence::InMemoryStore::new());
        Self {
            session_id,
            user_id: uuid::Uuid::now_v7(),
            agent_id: uuid::Uuid::now_v7(),
            tenant_id: uuid::Uuid::now_v7(),
            client,
            messages: Vec::new(),
            system_prompt: None,
            tools: Vec::new(),
            tool_executor: None,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
            message_store: store.clone(),
            session_store: store.clone(),
            context_window_size: None,
            last_response_metadata: None,
        }
    }

    /// 使用指定字符串 ID 创建会话
    /// Create session with specified string ID
    pub fn with_id_str(session_id: &str, client: LLMClient) -> Self {
        // 尝试将字符串解析为 UUID，如果失败则生成新的 UUID
        // Try to parse string as UUID, generate a new one if it fails
        let session_id = uuid::Uuid::parse_str(session_id).unwrap_or_else(|_| uuid::Uuid::now_v7());
        Self::with_id(session_id, client)
    }

    /// 使用指定 ID 和存储实现创建会话
    /// Create session with specified ID and storage implementations
    #[allow(clippy::too_many_arguments)]
    pub fn with_id_and_stores(
        session_id: uuid::Uuid,
        client: LLMClient,
        user_id: uuid::Uuid,
        tenant_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        message_store: Arc<dyn crate::persistence::MessageStore>,
        session_store: Arc<dyn crate::persistence::SessionStore>,
        context_window_size: Option<usize>,
    ) -> Self {
        Self {
            session_id,
            user_id,
            tenant_id,
            agent_id,
            client,
            messages: Vec::new(),
            system_prompt: None,
            tools: Vec::new(),
            tool_executor: None,
            created_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
            message_store,
            session_store,
            context_window_size,
            last_response_metadata: None,
        }
    }

    /// 使用指定 ID 和存储实现创建会话，并立即持久化到数据库
    /// Create session with ID and stores, and persist to database immediately
    ///
    /// 这个方法会将会话记录保存到数据库，确保会话在创建时就被持久化。
    /// This method saves session records to DB, ensuring persistence upon creation.
    /// 这对于需要将会话 ID 用作外键的场景很重要（例如保存消息时）。
    /// Important for scenarios using session ID as a foreign key (e.g., saving messages).
    ///
    /// # 参数
    /// # Parameters
    /// - `session_id`: 会话 ID
    /// - `session_id`: Session ID
    /// - `client`: LLM 客户端
    /// - `client`: LLM Client
    /// - `user_id`: 用户 ID
    /// - `user_id`: User ID
    /// - `agent_id`: Agent ID
    /// - `agent_id`: Agent ID
    /// - `message_store`: 消息存储
    /// - `message_store`: Message storage
    /// - `session_store`: 会话存储
    /// - `session_store`: Session storage
    /// - `context_window_size`: 可选的上下文窗口大小
    /// - `context_window_size`: Optional context window size
    ///
    /// # 返回
    /// # Returns
    /// 返回创建并持久化后的会话
    /// Returns the created and persisted session
    ///
    /// # 错误
    /// # Error
    /// 如果数据库操作失败，返回错误
    /// Returns error if database operation fails
    #[allow(clippy::too_many_arguments)]
    pub async fn with_id_and_stores_and_persist(
        session_id: uuid::Uuid,
        client: LLMClient,
        user_id: uuid::Uuid,
        tenant_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        message_store: Arc<dyn crate::persistence::MessageStore>,
        session_store: Arc<dyn crate::persistence::SessionStore>,
        context_window_size: Option<usize>,
    ) -> crate::persistence::PersistenceResult<Self> {
        // 创建内存会话
        // Create in-memory session
        let session = Self::with_id_and_stores(
            session_id,
            client,
            user_id,
            tenant_id,
            agent_id,
            message_store,
            session_store.clone(),
            context_window_size,
        );

        // 持久化会话记录到数据库
        // Persist session record to the database
        let db_session =
            crate::persistence::ChatSession::new(user_id, agent_id).with_id(session_id);
        session_store.create_session(&db_session).await?;

        Ok(session)
    }

    /// 生成唯一会话 ID
    /// Generate unique session ID
    fn generate_session_id() -> uuid::Uuid {
        uuid::Uuid::now_v7()
    }

    /// 获取会话 ID
    /// Get session ID
    pub fn session_id(&self) -> uuid::Uuid {
        self.session_id
    }

    /// 获取会话 ID 字符串
    /// Get session ID string
    pub fn session_id_str(&self) -> String {
        self.session_id.to_string()
    }

    /// 获取会话创建时间
    /// Get session creation time
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }

    /// 从数据库加载会话
    /// Load session from database
    ///
    /// 创建一个新的 ChatSession 实例，加载指定 ID 的会话和消息。
    /// Creates a new ChatSession instance, loading session and messages for the ID.
    ///
    /// # 参数
    /// # Parameters
    /// - `session_id`: 会话 ID
    /// - `session_id`: Session ID
    /// - `client`: LLM 客户端
    /// - `client`: LLM Client
    /// - `user_id`: 用户 ID
    /// - `user_id`: User ID
    /// - `agent_id`: Agent ID
    /// - `agent_id`: Agent ID
    /// - `message_store`: 消息存储
    /// - `message_store`: Message storage
    /// - `session_store`: 会话存储
    /// - `session_store`: Session storage
    /// - `context_window_size`: 可选的上下文窗口大小（轮数），如果指定则只加载最近的 N 轮对话
    /// - `context_window_size`: Optional context window (rounds); if set, loads only recent N rounds
    ///
    /// # 注意
    /// # Note
    /// 当指定 `context_window_size` 时，只会加载最近的 N 轮对话到内存中。
    /// When `context_window_size` is specified, only recent N rounds are loaded to memory.
    /// 这对于长期对话很有用，可以避免加载大量历史消息。
    /// Useful for long conversations to avoid loading massive history.
    #[allow(clippy::too_many_arguments)]
    pub async fn load(
        session_id: uuid::Uuid,
        client: LLMClient,
        user_id: uuid::Uuid,
        tenant_id: uuid::Uuid,
        agent_id: uuid::Uuid,
        message_store: Arc<dyn crate::persistence::MessageStore>,
        session_store: Arc<dyn crate::persistence::SessionStore>,
        context_window_size: Option<usize>,
    ) -> crate::persistence::PersistenceResult<Self> {
        // Load session from database
        let db_session = session_store
            .get_session(session_id)
            .await?
            .ok_or_else(|| {
                crate::persistence::PersistenceError::NotFound("Session not found".to_string())
            })?;

        // Load messages from database
        // Use pagination when context_window_size is set to avoid loading all messages
        let db_messages = if context_window_size.is_some() {
            // Use pagination to avoid loading all messages for long-running sessions
            let total_count = message_store.count_session_messages(session_id).await?;

            // Calculate fetch limit: rounds * 2 (user+assistant per round) + buffer for system messages
            let rounds = context_window_size.unwrap_or(0);
            let limit = (rounds * 2 + 20) as i64; // 20 message buffer for system messages at beginning

            // Calculate offset to get the most recent messages
            let offset = std::cmp::max(0, total_count - limit);

            message_store
                .get_session_messages_paginated(session_id, offset, limit)
                .await?
        } else {
            // No window size specified, fetch all messages (current behavior)
            message_store.get_session_messages(session_id).await?
        };

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
            let domain_content = db_msg
                .content
                .text
                .map(crate::llm::types::MessageContent::Text);

            // Create domain message
            let domain_msg = ChatMessage {
                role: domain_role,
                content: domain_content,
                name: None,
                tool_calls: None,
                tool_call_id: None,
            };
            messages.push(domain_msg);
        }

        // 应用滑动窗口逻辑（如果指定了 context_window_size）
        // Apply sliding window logic (if context_window_size is specified)
        let messages = Self::apply_sliding_window_static(&messages, context_window_size);

        // Create and return ChatSession
        Ok(Self {
            session_id,
            user_id,
            tenant_id,
            agent_id,
            client,
            messages,
            system_prompt: None, // System prompt is not stored in messages
            tools: Vec::new(),   // Tools are not persisted yet
            tool_executor: None, // Tool executor is not persisted
            created_at: db_session.create_time,
            metadata: db_session
                .metadata
                .into_iter()
                .map(|(k, v)| (k, v.to_string()))
                .collect(),
            message_store,
            session_store,
            context_window_size,
            last_response_metadata: None,
        })
    }

    /// 获取会话存活时长
    /// Get session duration
    pub fn elapsed(&self) -> std::time::Duration {
        (chrono::Utc::now() - self.created_at)
            .to_std()
            .unwrap_or_default()
    }

    /// 设置元数据
    /// Set metadata
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// 获取元数据
    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// 获取所有元数据
    /// Get all metadata
    pub fn metadata(&self) -> &std::collections::HashMap<String, String> {
        &self.metadata
    }

    /// 设置系统提示
    /// Set system prompt
    pub fn with_system(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置上下文窗口大小（滑动窗口）
    /// Set context window size (sliding window)
    ///
    /// # 参数
    /// # Parameters
    /// - `size`: 保留的最大对话轮数（None 表示不限制）
    /// - `size`: Max conversation rounds to keep (None means unlimited)
    ///
    /// # 注意
    /// # Note
    /// - 单位是**轮数**（rounds），不是 token 数量
    /// - Unit is rounds, not token count
    /// - 每轮对话 ≈ 1 个用户消息 + 1 个助手响应
    /// - Each round ≈ 1 user message + 1 assistant response
    /// - 系统消息始终保留，不计入轮数限制
    /// - System messages are always kept, not counted in round limits
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let session = ChatSession::new(client)
    ///     .with_context_window_size(Some(10)); // 只保留最近 10 轮对话
    ///     // Keep only the most recent 10 conversation rounds
    /// ```
    pub fn with_context_window_size(mut self, size: Option<usize>) -> Self {
        self.context_window_size = size;
        self
    }

    /// 设置工具
    /// Set tools
    pub fn with_tools(mut self, tools: Vec<Tool>, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tools = tools;
        self.tool_executor = Some(executor);
        self
    }

    /// 仅设置工具执行器（工具列表自动发现）
    /// Set tool executor only (auto-discovery of tools)
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// 发送消息
    /// Send message
    pub async fn send(&mut self, content: impl Into<String>) -> LLMResult<String> {
        // 添加用户消息
        // Add user message
        self.messages.push(ChatMessage::user(content));

        // 构建请求
        // Build request
        let mut builder = self.client.chat();

        // 添加系统提示
        // Add system prompt
        if let Some(ref system) = self.system_prompt {
            builder = builder.system(system.clone());
        }

        // 添加历史消息（应用滑动窗口）
        // Add historical messages (apply sliding window)
        let messages_for_context = self.apply_sliding_window();
        builder = builder.messages(messages_for_context);

        // 添加工具
        // Add tools
        if let Some(ref executor) = self.tool_executor {
            let tools = if self.tools.is_empty() {
                executor.available_tools().await?
            } else {
                self.tools.clone()
            };

            if !tools.is_empty() {
                builder = builder.tools(tools);
            }

            builder = builder.with_tool_executor(executor.clone());
        }

        // 发送请求
        // Send request
        let response = if self.tool_executor.is_some() {
            builder.send_with_tools().await?
        } else {
            builder.send().await?
        };

        // 存储响应元数据
        // Store response metadata
        self.last_response_metadata = Some(super::types::LLMResponseMetadata::from(&response));

        // 提取响应内容
        // Extract response content
        let content = response
            .content()
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))?
            .to_string();

        // 添加助手消息到历史
        // Add assistant message to history
        self.messages.push(ChatMessage::assistant(&content));

        // 滑动窗口：裁剪历史消息以保持固定大小
        // Sliding window: trim history messages to maintain fixed size
        if self.context_window_size.is_some() {
            self.messages =
                Self::apply_sliding_window_static(&self.messages, self.context_window_size);
        }

        Ok(content)
    }

    /// 发送结构化消息（支持多模态）
    /// Send structured message (supports multi-modal)
    pub async fn send_with_content(&mut self, content: MessageContent) -> LLMResult<String> {
        self.messages.push(ChatMessage::user_with_content(content));

        // 构建请求
        // Build request
        let mut builder = self.client.chat();

        // 添加系统提示
        // Add system prompt
        if let Some(ref system) = self.system_prompt {
            builder = builder.system(system.clone());
        }

        // 添加历史消息（应用滑动窗口）
        // Add historical messages (apply sliding window)
        let messages_for_context = self.apply_sliding_window();
        builder = builder.messages(messages_for_context);

        // 添加工具
        // Add tools
        if let Some(ref executor) = self.tool_executor {
            let tools = if self.tools.is_empty() {
                executor.available_tools().await?
            } else {
                self.tools.clone()
            };

            if !tools.is_empty() {
                builder = builder.tools(tools);
            }

            builder = builder.with_tool_executor(executor.clone());
        }

        // 发送请求
        // Send request
        let response = if self.tool_executor.is_some() {
            builder.send_with_tools().await?
        } else {
            builder.send().await?
        };

        // 存储响应元数据
        // Store response metadata
        self.last_response_metadata = Some(super::types::LLMResponseMetadata::from(&response));

        // 提取响应内容
        // Extract response content
        let content = response
            .content()
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))?
            .to_string();

        // 添加助手消息到历史
        // Add assistant message to history
        self.messages.push(ChatMessage::assistant(&content));

        // 滑动窗口：裁剪历史消息以保持固定大小
        // Sliding window: trim history messages to maintain fixed size
        if self.context_window_size.is_some() {
            self.messages =
                Self::apply_sliding_window_static(&self.messages, self.context_window_size);
        }

        Ok(content)
    }

    /// 获取消息历史
    /// Get message history
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// 获取消息历史（可变引用）
    /// Get message history (mutable reference)
    pub fn messages_mut(&mut self) -> &mut Vec<ChatMessage> {
        &mut self.messages
    }

    /// 清空消息历史
    /// Clear message history
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// 获取消息数量
    /// Get message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// 是否为空
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// 设置上下文窗口大小（滑动窗口）
    /// Set context window size (sliding window)
    ///
    /// # 参数
    /// # Parameters
    /// - `size`: 保留的最大对话轮数（None 表示不限制）
    /// - `size`: Max conversation rounds to keep (None means unlimited)
    ///
    /// # 注意
    /// # Note
    /// - 单位是**轮数**（rounds），不是 token 数量
    /// - Unit is rounds, not token count
    /// - 每轮对话 ≈ 1 个用户消息 + 1 个助手响应（可能包含工具调用）
    /// - Each round ≈ 1 user message + 1 assistant response (may include tool calls)
    /// - 系统消息始终保留，不计入轮数限制
    /// - System messages are always kept, not counted in round limits
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// session.set_context_window_size(Some(10)); // 只保留最近 10 轮对话
    /// // Keep only most recent 10 conversation rounds
    /// session.set_context_window_size(None);     // 不限制对话轮数
    /// // No limit on conversation rounds
    /// ```
    pub fn set_context_window_size(&mut self, size: Option<usize>) {
        self.context_window_size = size;
    }

    /// 获取上下文窗口大小（轮数）
    /// Get context window size (rounds)
    pub fn context_window_size(&self) -> Option<usize> {
        self.context_window_size
    }

    /// 获取最后一次 LLM 响应的元数据
    /// Get metadata of the last LLM response
    pub fn last_response_metadata(&self) -> Option<&super::types::LLMResponseMetadata> {
        self.last_response_metadata.as_ref()
    }

    /// 应用滑动窗口，返回限定后的消息列表
    /// Apply sliding window, returns the limited message list
    ///
    /// 保留最近的 N 轮对话（每轮包括用户消息和助手响应）。
    /// Keeps recent N rounds (each round includes user msg and assistant response).
    /// 系统消息始终保留。
    /// System messages are always kept.
    ///
    /// # 参数
    /// # Parameters
    /// - `messages`: 要过滤的消息列表
    /// - `messages`: List of messages to filter
    /// - `window_size`: 保留的最大对话轮数（None 表示不限制）
    /// - `window_size`: Max rounds to keep (None means unlimited)
    ///
    /// # 算法说明
    /// # Algorithm Description
    /// - 1. 分离系统消息和对话消息
    /// - 1. Separate system and conversation messages
    /// - 2. 从对话消息中保留最近的 N 轮
    /// - 2. Keep the most recent N rounds from conversation messages
    /// - 3. 合并系统消息和裁剪后的对话消息
    /// - 3. Merge system messages with trimmed conversation messages
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// // 假设有 20 条消息（约 10 轮对话）
    /// // Assuming 20 messages (approx. 10 rounds)
    /// // 设置 window_size = Some(3)
    /// // Setting window_size = Some(3)
    /// // 结果：保留最近 3 轮对话（约 6 条消息）+ 所有系统消息
    /// // Result: keep recent 3 rounds (approx. 6 messages) + all system messages
    /// let limited = ChatSession::apply_sliding_window_static(messages, Some(3));
    /// ```
    fn apply_sliding_window(&self) -> Vec<ChatMessage> {
        Self::apply_sliding_window_static(&self.messages, self.context_window_size)
    }

    /// 静态方法：应用滑动窗口到消息列表
    /// Static method: apply sliding window to message list
    ///
    /// 这个方法可以在不拥有 ChatSession 实例的情况下使用，
    /// This method can be used without owning a ChatSession instance,
    /// 例如在从数据库加载消息时。
    /// such as when loading messages from the database.
    ///
    /// # 参数
    /// # Parameters
    /// - `messages`: 要过滤的消息列表
    /// - `messages`: Message list to filter
    /// - `window_size`: 保留的最大对话轮数（None 表示不限制）
    /// - `window_size`: Max rounds to keep (None means unlimited)
    pub fn apply_sliding_window_static(
        messages: &[ChatMessage],
        window_size: Option<usize>,
    ) -> Vec<ChatMessage> {
        let max_rounds = match window_size {
            Some(size) if size > 0 => size,
            _ => return messages.to_vec(), // 无限制，返回所有消息
                                           // No limit, return all messages
        };

        // 分离系统消息和对话消息
        // Separate system messages and conversation messages
        let mut system_messages = Vec::new();
        let mut conversation_messages = Vec::new();

        for msg in messages {
            if msg.role == Role::System {
                system_messages.push(msg.clone());
            } else {
                conversation_messages.push(msg.clone());
            }
        }

        // 计算需要保留的最大消息数（每轮大约2条：用户+助手）
        // Calculate max messages to keep (approx 2 per round: user + assistant)
        let max_messages = max_rounds * 2;

        if conversation_messages.len() <= max_messages {
            // 对话消息数量在限制内，返回所有消息
            // Conversation count within limit, return all messages
            return messages.to_vec();
        }

        // 保留最后的 N 条对话消息
        // Keep the last N conversation messages
        let start_index = conversation_messages.len() - max_messages;
        let limited_conversation: Vec<ChatMessage> = conversation_messages
            .into_iter()
            .skip(start_index)
            .collect();

        // 合并系统消息和裁剪后的对话消息
        // Merge system messages and trimmed conversation messages
        let mut result = system_messages;
        result.extend(limited_conversation);

        result
    }

    /// 保存会话和消息到数据库
    /// Save session and messages to the database
    pub async fn save(&self) -> crate::persistence::PersistenceResult<()> {
        // Convert ChatSession to persistence entity
        let db_session = crate::persistence::ChatSession::new(self.user_id, self.agent_id)
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
            };

            // Convert MessageContent to persistence format
            let persistence_content = match &msg.content {
                Some(crate::llm::types::MessageContent::Text(text)) => {
                    crate::persistence::MessageContent::text(text)
                }
                Some(crate::llm::types::MessageContent::Parts(parts)) => {
                    // For now, only handle text parts
                    let text = parts
                        .iter()
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
                self.tenant_id,
                persistence_role,
                persistence_content,
            );

            // Save message
            self.message_store.save_message(&llm_message).await?;
        }

        Ok(())
    }

    /// 从数据库删除会话和消息
    /// Delete session and messages from database
    pub async fn delete(&self) -> crate::persistence::PersistenceResult<()> {
        // Delete all messages for the session
        self.message_store
            .delete_session_messages(self.session_id)
            .await?;

        // Delete the session itself
        self.session_store.delete_session(self.session_id).await?;

        Ok(())
    }
}

// ============================================================================
// 便捷函数
// Helper Functions
// ============================================================================

/// 快速创建函数工具定义
/// Quickly create function tool definition
///
/// # 示例
/// # Example
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
