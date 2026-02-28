//! 标准 LLM Agent 实现
//! Standard LLM Agent implementation
//!
//! 框架提供的开箱即用的 LLM Agent，用户只需配置 provider 即可使用
//! Out-of-the-box LLM Agent provided by the framework; users only need to configure the provider.
//!
//! # 示例
//! # Example
//!
//! ```rust,ignore
//! use mofa_sdk::kernel::AgentInput;
//! use mofa_sdk::runtime::run_agents;
//! use mofa_sdk::llm::LLMAgentBuilder;
//!
//! #[tokio::main]
//! async fn main() -> GlobalResult<()> {
//!     let agent = LLMAgentBuilder::from_env()?
//!         .with_id("my-llm-agent")
//!         .with_system_prompt("You are a helpful assistant.")
//!         .build();
//!
//!     let outputs = run_agents(agent, vec![AgentInput::text("Hello")]).await?;
//!     println!("{}", outputs[0].to_text());
//!     Ok(())
//! }
//! ```

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use super::client::{ChatSession, LLMClient};
use super::provider::{ChatStream, LLMProvider};
use super::tool_executor::ToolExecutor;
use super::types::{ChatMessage, LLMError, LLMResult, Tool};
use crate::llm::{
    AnthropicConfig, AnthropicProvider, GeminiConfig, GeminiProvider, OllamaConfig, OllamaProvider,
};
use crate::prompt;
use futures::{Stream, StreamExt};
use mofa_kernel::agent::AgentMetadata;
use mofa_kernel::agent::AgentState;
use mofa_kernel::plugin::{AgentPlugin, PluginType};
use mofa_plugins::tts::TTSPlugin;
use std::collections::HashMap;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

/// Type alias for TTS audio stream - boxed to avoid exposing kokoro-tts types
pub type TtsAudioStream = Pin<Box<dyn Stream<Item = (Vec<f32>, Duration)> + Send>>;

/// Cancellation token for cooperative cancellation
struct CancellationToken {
    cancel: Arc<AtomicBool>,
}

impl CancellationToken {
    fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn clone_token(&self) -> CancellationToken {
        CancellationToken {
            cancel: Arc::clone(&self.cancel),
        }
    }
}

/// 流式文本响应类型
/// Streaming text response type
///
/// 每次 yield 一个文本片段（delta content）
/// Yields a text fragment (delta content) each time
pub type TextStream = Pin<Box<dyn Stream<Item = LLMResult<String>> + Send>>;

/// TTS 流句柄：持有 sink 和消费者任务
/// TTS stream handle: holds sink and consumer task
///
/// 用于实时流式 TTS，允许 incremental 提交文本
/// Used for real-time streaming TTS, allowing incremental text submission
#[cfg(feature = "kokoro")]
struct TTSStreamHandle {
    sink: mofa_plugins::tts::kokoro_wrapper::SynthSink<String>,
    _stream_handle: tokio::task::JoinHandle<()>,
}

/// Active TTS session with cancellation support
struct TTSSession {
    cancellation_token: CancellationToken,
    is_active: Arc<AtomicBool>,
}

impl TTSSession {
    fn new(token: CancellationToken) -> Self {
        let is_active = Arc::new(AtomicBool::new(true));
        TTSSession {
            cancellation_token: token,
            is_active,
        }
    }

    fn cancel(&self) {
        self.cancellation_token.cancel();
        self.is_active.store(false, Ordering::Relaxed);
    }

    fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }
}

/// 句子缓冲区：按标点符号断句（内部实现）
/// Sentence buffer: splits sentences by punctuation (internal implementation)
struct SentenceBuffer {
    buffer: String,
}

impl SentenceBuffer {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// 推入文本块，返回完整句子（如果有）
    /// Pushes text block, returns full sentence (if any)
    fn push(&mut self, text: &str) -> Option<String> {
        for ch in text.chars() {
            self.buffer.push(ch);
            // 句末标点：。！？!?
            // Sentence-ending punctuation: 。！？!?
            if matches!(ch, '。' | '！' | '？' | '!' | '?') {
                let sentence = self.buffer.trim().to_string();
                if !sentence.is_empty() {
                    self.buffer.clear();
                    return Some(sentence);
                }
            }
        }
        None
    }

    /// 刷新剩余内容
    /// Flushes remaining content
    fn flush(&mut self) -> Option<String> {
        if self.buffer.trim().is_empty() {
            None
        } else {
            let remaining = self.buffer.trim().to_string();
            self.buffer.clear();
            Some(remaining)
        }
    }
}

/// 流式响应事件
/// Streaming response events
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 文本片段
    /// Text fragment
    Text(String),
    /// 工具调用开始
    /// Tool call start
    ToolCallStart { id: String, name: String },
    /// 工具调用参数片段
    /// Tool call arguments fragment
    ToolCallDelta { id: String, arguments_delta: String },
    /// 完成原因
    /// Completion reason
    Done(Option<String>),
}

/// LLM Agent 配置
/// LLM Agent configuration
#[derive(Clone)]
pub struct LLMAgentConfig {
    /// Agent ID
    pub agent_id: String,
    /// Agent 名称
    /// Agent name
    pub name: String,
    /// 系统提示词
    /// System prompt
    pub system_prompt: Option<String>,
    /// 默认温度
    /// Default temperature
    pub temperature: Option<f32>,
    /// 默认最大 token 数
    /// Default maximum tokens
    pub max_tokens: Option<u32>,
    /// 自定义配置
    /// Custom configuration
    pub custom_config: HashMap<String, String>,
    /// 用户 ID，用于数据库持久化和多用户场景
    /// User ID, for database persistence and multi-user scenarios
    pub user_id: Option<String>,
    /// 租户 ID，用于多租户支持
    /// Tenant ID, for multi-tenant support
    pub tenant_id: Option<String>,
    /// 上下文窗口大小，用于滑动窗口消息 management（单位：轮数/rounds）
    /// Context window size, for sliding window message management (unit: rounds)
    ///
    /// 注意：单位是**轮数**（rounds），不是 token 数量
    /// Note: The unit is **rounds**, not token counts
    /// 每轮对话 ≈ 1 个用户消息 + 1 个助手响应
    /// Each round ≈ 1 user message + 1 assistant response
    pub context_window_size: Option<usize>,
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
            user_id: None,
            tenant_id: None,
            context_window_size: None,
        }
    }
}

/// 标准 LLM Agent
/// Standard LLM Agent
///
/// 框架提供的开箱即用的 LLM Agent 实现
/// Out-of-the-box LLM Agent implementation provided by the framework
///
/// # 多会话支持
/// # Multi-session support
///
/// LLMAgent 支持多会话管理，每个会话有唯一的 session_id：
/// LLMAgent supports multi-session management, each session having a unique session_id:
///
/// ```rust,ignore
/// // 创建新会话
/// // Create new session
/// let session_id = agent.create_session().await;
///
/// // 使用指定会话对话
/// // Chat with specified session
/// agent.chat_with_session(&session_id, "Hello").await?;
///
/// // 切换默认会话
/// // Switch default session
/// agent.switch_session(&session_id).await?;
///
/// // 获取所有会话ID
/// // Get all session IDs
/// let sessions = agent.list_sessions().await;
/// ```
///
/// # TTS 支持
/// # TTS support
///
/// LLMAgent 支持通过统一的插件系统配置 TTS：
/// LLMAgent supports configuring TTS via a unified plugin system:
///
/// ```rust,ignore
/// // 创建 TTS 插件（引擎 + 可选音色）
/// // Create TTS plugin (engine + optional voice)
/// let tts_plugin = TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_090"));
///
/// // 通过插件系统添加
/// // Add via plugin system
/// let agent = LLMAgentBuilder::new()
///     .with_id("my-agent")
///     .with_provider(Arc::new(openai_from_env()?))
///     .with_plugin(tts_plugin)
///     .build();
///
/// // 直接使用 TTS
/// // Use TTS directly
/// agent.tts_speak("Hello world").await?;
///
/// // 高级用法：自定义配置
/// // Advanced usage: custom configuration
/// let tts_plugin = TTSPlugin::with_engine("tts", kokoro_engine, Some("zf_090"))
///     .with_config(TTSPluginConfig {
///         streaming_chunk_size: 8192,
///         ..Default::default()
///     });
/// ```
pub struct LLMAgent {
    config: LLMAgentConfig,
    /// 智能体元数据
    /// Agent metadata
    metadata: AgentMetadata,
    client: LLMClient,
    /// 多会话存储 (session_id -> ChatSession)
    /// Multi-session storage (session_id -> ChatSession)
    sessions: Arc<RwLock<HashMap<String, Arc<RwLock<ChatSession>>>>>,
    /// 当前活动会话ID
    /// Current active session ID
    active_session_id: Arc<RwLock<String>>,
    tools: Vec<Tool>,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    event_handler: Option<Box<dyn LLMAgentEventHandler>>,
    /// 插件列表
    /// Plugin list
    plugins: Vec<Box<dyn AgentPlugin>>,
    /// 当前智能体状态
    /// Current agent state
    state: AgentState,
    /// 保存 provider 用于创建新会话
    /// Save provider for creating new sessions
    provider: Arc<dyn LLMProvider>,
    /// Prompt 模板插件
    /// Prompt template plugin
    prompt_plugin: Option<Box<dyn prompt::PromptTemplatePlugin>>,
    /// TTS 插件（通过 builder 配置）
    /// TTS plugin (configured via builder)
    tts_plugin: Option<Arc<Mutex<TTSPlugin>>>,
    /// 缓存的 Kokoro TTS 引擎（只需初始化一次，后续可复用）
    /// Cached Kokoro TTS engine (initialize once, reuse later)
    #[cfg(feature = "kokoro")]
    cached_kokoro_engine: Arc<Mutex<Option<Arc<mofa_plugins::tts::kokoro_wrapper::KokoroTTS>>>>,
    /// Active TTS session for cancellation
    active_tts_session: Arc<Mutex<Option<TTSSession>>>,
    /// 持久化存储（可选，用于从数据库加载历史会话）
    /// Persistent storage (optional, for loading session history from database)
    message_store: Option<Arc<dyn crate::persistence::MessageStore + Send + Sync>>,
    session_store: Option<Arc<dyn crate::persistence::SessionStore + Send + Sync>>,
    /// 用户 ID（用于从数据库加载会话）
    /// User ID (for loading sessions from database)
    persistence_user_id: Option<uuid::Uuid>,
    /// Agent ID（用于从数据库加载会话）
    /// Agent ID (for loading sessions from database)
    persistence_agent_id: Option<uuid::Uuid>,
}

/// LLM Agent 事件处理器
/// LLM Agent event handler
///
/// 允许用户自定义事件处理逻辑
/// Allows users to customize event processing logic
#[async_trait::async_trait]
pub trait LLMAgentEventHandler: Send + Sync {
    /// Clone this handler trait object
    fn clone_box(&self) -> Box<dyn LLMAgentEventHandler>;

    /// 获取 Any 类型用于 downcasting
    /// Get Any type for downcasting
    fn as_any(&self) -> &dyn std::any::Any;

    /// 处理用户消息前的钩子
    /// Hook before processing user message
    async fn before_chat(&self, message: &str) -> LLMResult<Option<String>> {
        Ok(Some(message.to_string()))
    }

    /// 处理用户消息前的钩子（带模型名称）
    /// Hook before processing user message (with model name)
    ///
    /// 默认实现调用 `before_chat`。
    /// Default implementation calls `before_chat`.
    /// 如果需要知道使用的模型名称（例如用于持久化），请实现此方法。
    /// If you need to know the model name (e.g., for persistence), implement this method.
    async fn before_chat_with_model(
        &self,
        message: &str,
        _model: &str,
    ) -> LLMResult<Option<String>> {
        self.before_chat(message).await
    }

    /// 处理 LLM 响应后的钩子
    /// Hook after processing LLM response
    async fn after_chat(&self, response: &str) -> LLMResult<Option<String>> {
        Ok(Some(response.to_string()))
    }

    /// 处理 LLM 响应后的钩子（带元数据）
    /// Hook after processing LLM response (with metadata)
    ///
    /// 默认实现调用 after_chat。
    /// Default implementation calls after_chat.
    /// 如果需要访问响应元数据（如 response_id, model, token counts），请实现此方法。
    /// If you need to access response metadata (e.g., response_id, model, token counts), implement this method.
    async fn after_chat_with_metadata(
        &self,
        response: &str,
        _metadata: &super::types::LLMResponseMetadata,
    ) -> LLMResult<Option<String>> {
        self.after_chat(response).await
    }

    /// 处理工具调用
    /// Handle tool calls
    async fn on_tool_call(&self, name: &str, arguments: &str) -> LLMResult<Option<String>> {
        let _ = (name, arguments);
        Ok(None)
    }

    /// 处理错误
    /// Handle errors
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
    /// Create new LLM Agent
    pub fn new(config: LLMAgentConfig, provider: Arc<dyn LLMProvider>) -> Self {
        Self::with_initial_session(config, provider, None)
    }

    /// 创建新的 LLM Agent，并指定初始会话 ID
    /// Create new LLM Agent and specify initial session ID
    ///
    /// # 参数
    /// # Parameters
    /// - `config`: Agent 配置
    /// - `config`: Agent configuration
    /// - `provider`: LLM Provider
    /// - `initial_session_id`: 初始会话 ID，如果为 None 则使用自动生成的 ID
    /// - `initial_session_id`: Initial session ID; if None, an auto-generated ID is used
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = LLMAgent::with_initial_session(
    ///     config,
    ///     provider,
    ///     Some("user-session-001".to_string())
    /// );
    /// ```
    pub fn with_initial_session(
        config: LLMAgentConfig,
        provider: Arc<dyn LLMProvider>,
        initial_session_id: Option<String>,
    ) -> Self {
        let client = LLMClient::new(provider.clone());

        let mut session = if let Some(sid) = initial_session_id {
            ChatSession::with_id_str(&sid, LLMClient::new(provider.clone()))
        } else {
            ChatSession::new(LLMClient::new(provider.clone()))
        };

        // 设置系统提示
        // Set system prompt
        if let Some(ref prompt) = config.system_prompt {
            session = session.with_system(prompt.clone());
        }

        // 设置上下文窗口大小
        // Set context window size
        session = session.with_context_window_size(config.context_window_size);

        let session_id = session.session_id().to_string();
        let session_arc = Arc::new(RwLock::new(session));

        // 初始化会话存储
        // Initialize session storage
        let mut sessions = HashMap::new();
        sessions.insert(session_id.clone(), session_arc);

        // Clone fields needed for metadata before moving config
        let agent_id = config.agent_id.clone();
        let name = config.name.clone();

        // 创建 AgentCapabilities
        // Create AgentCapabilities
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
            tts_plugin: None,
            #[cfg(feature = "kokoro")]
            cached_kokoro_engine: Arc::new(Mutex::new(None)),
            active_tts_session: Arc::new(Mutex::new(None)),
            message_store: None,
            session_store: None,
            persistence_user_id: None,
            persistence_agent_id: None,
        }
    }

    /// 创建新的 LLM Agent，并尝试从数据库加载初始会话（异步版本）
    /// Create new LLM Agent and try to load initial session from database (async version)
    ///
    /// 如果提供了 persistence stores 且 session_id 存在于数据库中，
    /// If persistence stores are provided and session_id exists in database,
    /// 会自动加载历史消息并应用滑动窗口。
    /// historical messages will be loaded and sliding window applied automatically.
    ///
    /// # 参数
    /// # Parameters
    /// - `config`: Agent 配置
    /// - `config`: Agent configuration
    /// - `provider`: LLM Provider
    /// - `initial_session_id`: 初始会话 ID，如果为 None 则使用自动生成的 ID
    /// - `initial_session_id`: Initial session ID; if None, an auto-generated ID is used
    /// - `message_store`: 消息存储（可选，用于从数据库加载历史）
    /// - `message_store`: Message store (optional, for loading history from database)
    /// - `session_store`: 会话存储（可选，用于从数据库加载历史）
    /// - `session_store`: Session store (optional, for loading history from database)
    /// - `persistence_user_id`: 用户 ID（用于从数据库加载会话）
    /// - `persistence_user_id`: User ID (for loading session from database)
    /// - `persistence_agent_id`: Agent ID（用于从数据库加载会话）
    /// - `persistence_agent_id`: Agent ID (for loading session from database)
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = LLMAgent::with_initial_session_async(
    ///     config,
    ///     provider,
    ///     Some("user-session-001".to_string()),
    ///     Some(message_store),
    ///     Some(session_store),
    ///     Some(user_id),
    ///     Some(agent_id),
    /// ).await?;
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub async fn with_initial_session_async(
        config: LLMAgentConfig,
        provider: Arc<dyn LLMProvider>,
        initial_session_id: Option<String>,
        message_store: Option<Arc<dyn crate::persistence::MessageStore + Send + Sync>>,
        session_store: Option<Arc<dyn crate::persistence::SessionStore + Send + Sync>>,
        persistence_user_id: Option<uuid::Uuid>,
        persistence_tenant_id: Option<uuid::Uuid>,
        persistence_agent_id: Option<uuid::Uuid>,
    ) -> Self {
        let client = LLMClient::new(provider.clone());

        // Clone initial_session_id to avoid move issues
        let initial_session_id_clone = initial_session_id.clone();

        // 1. 尝试从数据库加载会话（如果有 stores 且指定了 session_id）
        // 1. Try to load session from database (if stores are present and session_id specified)
        let session = if let (
            Some(sid),
            Some(msg_store),
            Some(sess_store),
            Some(user_id),
            Some(tenant_id),
            Some(agent_id),
        ) = (
            initial_session_id_clone,
            message_store.clone(),
            session_store.clone(),
            persistence_user_id,
            persistence_tenant_id,
            persistence_agent_id,
        ) {
            // Clone stores before moving them into ChatSession::load
            let msg_store_clone = msg_store.clone();
            let sess_store_clone = sess_store.clone();

            let session_uuid = uuid::Uuid::parse_str(&sid).unwrap_or_else(|_| {
                tracing::warn!(
                    "⚠️ Invalid session_id format '{}', generating a new UUID",
                    sid
                );
                // ⚠️ Invalid session_id format '{}', will generate new UUID
                uuid::Uuid::now_v7()
            });

            // 尝试从数据库加载
            // Try loading from database
            match ChatSession::load(
                session_uuid,
                LLMClient::new(provider.clone()),
                user_id,
                agent_id,
                tenant_id,
                msg_store,
                sess_store,
                config.context_window_size,
            )
            .await
            {
                Ok(loaded_session) => {
                    tracing::info!(
                        "✅ Session loaded from database: {} ({} messages)",
                        // ✅ Session loaded from database: {} ({} messages)
                        sid,
                        loaded_session.messages().len()
                    );
                    loaded_session
                }
                Err(e) => {
                    // 会话不存在，创建新会话（使用用户指定的ID和从persistence获取的user_id/agent_id）
                    // Session not found; create new session (using specified ID and user_id/agent_id from persistence)
                    tracing::info!(
                        "📝 Creating new session and persisting: {} (not found in DB: {})",
                        sid,
                        e
                    );
                    // 📝 Creating new session and persisting: {} (doesn't exist in DB: {})

                    // Clone stores again for the fallback case
                    let msg_store_clone2 = msg_store_clone.clone();
                    let sess_store_clone2 = sess_store_clone.clone();

                    // 使用正确的 user_id 和 agent_id 创建会话，并持久化到数据库
                    // Create session with correct user_id and agent_id, and persist to database
                    match ChatSession::with_id_and_stores_and_persist(
                        session_uuid,
                        LLMClient::new(provider.clone()),
                        user_id,
                        agent_id,
                        tenant_id,
                        msg_store_clone,
                        sess_store_clone,
                        config.context_window_size,
                    )
                    .await
                    {
                        Ok(mut new_session) => {
                            if let Some(ref prompt) = config.system_prompt {
                                new_session = new_session.with_system(prompt.clone());
                            }
                            new_session
                        }
                        Err(persist_err) => {
                            tracing::error!(
                                "❌ Failed to persist session: {}, falling back to in-memory session",
                                persist_err
                            );
                            // ❌ Persisting session failed: {}, falling back to in-memory session
                            // 降级：如果持久化失败，创建内存会话
                            // Fallback: If persistence fails, create in-memory session
                            let new_session = ChatSession::with_id_and_stores(
                                session_uuid,
                                LLMClient::new(provider.clone()),
                                user_id,
                                agent_id,
                                tenant_id,
                                msg_store_clone2,
                                sess_store_clone2,
                                config.context_window_size,
                            );
                            if let Some(ref prompt) = config.system_prompt {
                                new_session.with_system(prompt.clone())
                            } else {
                                new_session
                            }
                        }
                    }
                }
            }
        } else {
            // 没有 persistence stores，创建普通会话
            // No persistence stores; creating standard session
            let mut session = if let Some(sid) = initial_session_id {
                ChatSession::with_id_str(&sid, LLMClient::new(provider.clone()))
            } else {
                ChatSession::new(LLMClient::new(provider.clone()))
            };
            if let Some(ref prompt) = config.system_prompt {
                session = session.with_system(prompt.clone());
            }
            session.with_context_window_size(config.context_window_size)
        };

        let session_id = session.session_id().to_string();
        let session_arc = Arc::new(RwLock::new(session));

        // 初始化会话存储
        // Initialize session storage
        let mut sessions = HashMap::new();
        sessions.insert(session_id.clone(), session_arc);

        // Clone fields needed for metadata before moving config
        let agent_id = config.agent_id.clone();
        let name = config.name.clone();

        // 创建 AgentCapabilities
        // Create AgentCapabilities
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
            tts_plugin: None,
            #[cfg(feature = "kokoro")]
            cached_kokoro_engine: Arc::new(Mutex::new(None)),
            active_tts_session: Arc::new(Mutex::new(None)),
            message_store,
            session_store,
            persistence_user_id,
            persistence_agent_id,
        }
    }

    /// 获取配置
    /// Get configuration
    pub fn config(&self) -> &LLMAgentConfig {
        &self.config
    }

    /// 获取 LLM Client
    /// Get LLM Client
    pub fn client(&self) -> &LLMClient {
        &self.client
    }

    // ========================================================================
    // 会话管理方法
    // Session management methods
    // ========================================================================

    /// 获取当前活动会话ID
    /// Get current active session ID
    pub async fn current_session_id(&self) -> String {
        self.active_session_id.read().await.clone()
    }

    /// 创建新会话
    /// Create new session
    ///
    /// 返回新会话的 session_id
    /// Returns the session_id of the new session
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let session_id = agent.create_session().await;
    /// agent.chat_with_session(&session_id, "Hello").await?;
    /// ```
    pub async fn create_session(&self) -> String {
        let mut session = ChatSession::new(LLMClient::new(self.provider.clone()));

        // 使用动态 Prompt 模板（如果可用）
        // Use dynamic Prompt template (if available)
        let mut system_prompt = self.config.system_prompt.clone();

        if let Some(ref plugin) = self.prompt_plugin
            && let Some(template) = plugin.get_current_template().await
        {
            // 渲染默认模板
            // Render default template
            system_prompt = match template.render(&[]) {
                Ok(prompt) => Some(prompt),
                Err(_) => self.config.system_prompt.clone(),
            };
        }

        if let Some(ref prompt) = system_prompt {
            session = session.with_system(prompt.clone());
        }

        // 设置上下文窗口大小
        // Set context window size
        session = session.with_context_window_size(self.config.context_window_size);

        let session_id = session.session_id().to_string();
        let session_arc = Arc::new(RwLock::new(session));

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session_arc);

        session_id
    }

    /// 使用指定ID创建新会话
    /// Create new session with specified ID
    ///
    /// 如果 session_id 已存在，返回错误
    /// Returns error if session_id already exists
    ///
    /// # 示例
    /// # Example
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

        let mut session =
            ChatSession::with_id_str(&session_id, LLMClient::new(self.provider.clone()));

        // 使用动态 Prompt 模板（如果可用）
        // Use dynamic Prompt template (if available)
        let mut system_prompt = self.config.system_prompt.clone();

        if let Some(ref plugin) = self.prompt_plugin
            && let Some(template) = plugin.get_current_template().await
        {
            // 渲染默认模板
            // Render default template
            system_prompt = match template.render(&[]) {
                Ok(prompt) => Some(prompt),
                Err(_) => self.config.system_prompt.clone(),
            };
        }

        if let Some(ref prompt) = system_prompt {
            session = session.with_system(prompt.clone());
        }

        // 设置上下文窗口大小
        // Set context window size
        session = session.with_context_window_size(self.config.context_window_size);

        let session_arc = Arc::new(RwLock::new(session));

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session_arc);

        Ok(session_id)
    }

    /// 切换当前活动会话
    /// Switch current active session
    ///
    /// # 错误
    /// # Error
    /// 如果 session_id 不存在则返回错误
    /// Returns error if session_id does not exist
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
    /// Get or create session
    ///
    /// 如果 session_id 存在则返回它，否则使用该 ID 创建新会话
    /// Returns session_id if it exists, otherwise creates a new session with that ID
    pub async fn get_or_create_session(&self, session_id: impl Into<String>) -> String {
        let session_id = session_id.into();

        {
            let sessions = self.sessions.read().await;
            if sessions.contains_key(&session_id) {
                return session_id;
            }
        }

        // 会话不存在，创建新的
        // Session not found, creating new one
        let _ = self.create_session_with_id(&session_id).await;
        session_id
    }

    /// 删除会话
    /// Remove session
    ///
    /// # 注意
    /// # Note
    /// 不能删除当前活动会话，需要先切换到其他会话
    /// Cannot remove active session; switch to another session first
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
    /// List all session IDs
    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// 获取会话数量
    /// Get session count
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// 检查会话是否存在
    /// Check if session exists
    pub async fn has_session(&self, session_id: &str) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(session_id)
    }

    // ========================================================================
    // TTS 便捷方法
    // TTS convenience methods
    // ========================================================================

    /// 使用 TTS 合成并播放文本
    /// Synthesize and play text using TTS
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// agent.tts_speak("Hello world").await?;
    /// ```
    pub async fn tts_speak(&self, text: &str) -> LLMResult<()> {
        let tts = self
            .tts_plugin
            .as_ref()
            .ok_or_else(|| LLMError::Other("TTS plugin not configured".to_string()))?;

        let mut tts_guard = tts.lock().await;
        tts_guard
            .synthesize_and_play(text)
            .await
            .map_err(|e| LLMError::Other(format!("TTS synthesis failed: {}", e)))
    }

    /// 使用 TTS 流式合成文本
    /// Synthesize text in a stream using TTS
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// agent.tts_speak_streaming("Hello world", Box::new(|audio| {
    ///     println!("Got {} bytes of audio", audio.len());
    /// })).await?;
    /// ```
    pub async fn tts_speak_streaming(
        &self,
        text: &str,
        callback: Box<dyn Fn(Vec<u8>) + Send + Sync>,
    ) -> LLMResult<()> {
        let tts = self
            .tts_plugin
            .as_ref()
            .ok_or_else(|| LLMError::Other("TTS plugin not configured".to_string()))?;

        let mut tts_guard = tts.lock().await;
        tts_guard
            .synthesize_streaming(text, callback)
            .await
            .map_err(|e| LLMError::Other(format!("TTS streaming failed: {}", e)))
    }

    /// 使用 TTS 流式合成文本（f32 native format，更高效）
    /// Stream synthesize text using TTS (f32 native format, more efficient)
    ///
    /// This method is more efficient for KokoroTTS as it uses the native f32 format
    /// without the overhead of f32 -> i16 -> u8 conversion.
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use rodio::buffer::SamplesBuffer;
    ///
    /// agent.tts_speak_f32_stream("Hello world", Box::new(|audio_f32| {
    ///     // audio_f32 is Vec<f32> with values in [-1.0, 1.0]
    ///     sink.append(SamplesBuffer::new(1, 24000, audio_f32));
    /// })).await?;
    /// ```
    pub async fn tts_speak_f32_stream(
        &self,
        text: &str,
        callback: Box<dyn Fn(Vec<f32>) + Send + Sync>,
    ) -> LLMResult<()> {
        let tts = self
            .tts_plugin
            .as_ref()
            .ok_or_else(|| LLMError::Other("TTS plugin not configured".to_string()))?;

        let mut tts_guard = tts.lock().await;
        tts_guard
            .synthesize_streaming_f32(text, callback)
            .await
            .map_err(|e| LLMError::Other(format!("TTS f32 streaming failed: {}", e)))
    }

    /// 获取 TTS 音频流（仅支持 Kokoro TTS）
    /// Get TTS audio stream (only Kokoro TTS supported)
    ///
    /// Returns a direct stream of (audio_f32, duration) tuples from KokoroTTS.
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    /// use rodio::buffer::SamplesBuffer;
    ///
    /// if let Ok(mut stream) = agent.tts_create_stream("Hello world").await {
    ///     while let Some((audio, took)) = stream.next().await {
    ///         // audio is Vec<f32> with values in [-1.0, 1.0]
    ///         sink.append(SamplesBuffer::new(1, 24000, audio));
    ///     }
    /// }
    /// ```
    pub async fn tts_create_stream(&self, text: &str) -> LLMResult<TtsAudioStream> {
        #[cfg(feature = "kokoro")]
        {
            use mofa_plugins::tts::kokoro_wrapper::KokoroTTS;

            // 首先检查是否有缓存的引擎（只需初始化一次）
            // First check if there's a cached engine (initializes only once)
            let cached_engine = {
                let cache_guard = self.cached_kokoro_engine.lock().await;
                cache_guard.clone()
            };

            let kokoro = if let Some(engine) = cached_engine {
                // 使用缓存的引擎（无需再次获取 tts_plugin 的锁）
                // Use cached engine (no need to re-acquire tts_plugin lock)
                engine
            } else {
                // 首次调用：获取 tts_plugin 的锁，downcast 并缓存
                // First call: acquire tts_plugin lock, downcast, and cache
                let tts = self
                    .tts_plugin
                    .as_ref()
                    .ok_or_else(|| LLMError::Other("TTS plugin not configured".to_string()))?;

                let tts_guard = tts.lock().await;

                let engine = tts_guard
                    .engine()
                    .ok_or_else(|| LLMError::Other("TTS engine not initialized".to_string()))?;

                if let Some(kokoro_ref) = engine.as_any().downcast_ref::<KokoroTTS>() {
                    // 克隆 KokoroTTS（内部使用 Arc，克隆只是增加引用计数）
                    // Clone KokoroTTS (uses Arc internally, cloning just increases ref count)
                    let cloned = kokoro_ref.clone();
                    let cloned_arc = Arc::new(cloned);

                    // 获取 voice 配置
                    // Get voice configuration
                    let voice = tts_guard
                        .stats()
                        .get("default_voice")
                        .and_then(|v| v.as_str())
                        .unwrap_or("default");

                    // 缓存克隆的引擎
                    // Cache the cloned engine
                    {
                        let mut cache_guard = self.cached_kokoro_engine.lock().await;
                        *cache_guard = Some(cloned_arc.clone());
                    }

                    cloned_arc
                } else {
                    return Err(LLMError::Other("TTS engine is not KokoroTTS".to_string()));
                }
            };

            // 使用缓存的引擎创建 stream（无需再次获取 tts_plugin 的锁）
            // Create stream using cached engine (no need to re-acquire tts_plugin lock)
            let voice = "default"; // 可以从配置中获取
            // voice = "default"; // Can be retrieved from configuration
            let (mut sink, stream) = kokoro
                .create_stream(voice)
                .await
                .map_err(|e| LLMError::Other(format!("Failed to create TTS stream: {}", e)))?;

            // Submit text for synthesis
            sink.synth(text.to_string()).await.map_err(|e| {
                LLMError::Other(format!("Failed to submit text for synthesis: {}", e))
            })?;

            // Box the stream to hide the concrete type
            Ok(Box::pin(stream))
        }

        #[cfg(not(feature = "kokoro"))]
        {
            Err(LLMError::Other("Kokoro feature not enabled".to_string()))
        }
    }

    /// Stream multiple sentences through a single TTS stream
    ///
    /// This is more efficient than calling tts_speak_f32_stream multiple times
    /// because it reuses the same stream for all sentences, following the kokoro-tts
    /// streaming pattern: ONE stream, multiple synth calls, continuous audio output.
    ///
    /// # Arguments
    /// - `sentences`: Vector of text sentences to synthesize
    /// - `callback`: Function to call with each audio chunk (Vec<f32>)
    ///
    /// # Example
    /// ```rust,ignore
    /// use rodio::buffer::SamplesBuffer;
    ///
    /// let sentences = vec!["Hello".to_string(), "World".to_string()];
    /// agent.tts_speak_f32_stream_batch(
    ///     sentences,
    ///     Box::new(|audio_f32| {
    ///         sink.append(SamplesBuffer::new(1, 24000, audio_f32));
    ///     }),
    /// ).await?;
    /// ```
    pub async fn tts_speak_f32_stream_batch(
        &self,
        sentences: Vec<String>,
        callback: Box<dyn Fn(Vec<f32>) + Send + Sync>,
    ) -> LLMResult<()> {
        let tts = self
            .tts_plugin
            .as_ref()
            .ok_or_else(|| LLMError::Other("TTS plugin not configured".to_string()))?;

        let tts_guard = tts.lock().await;

        #[cfg(feature = "kokoro")]
        {
            use mofa_plugins::tts::kokoro_wrapper::KokoroTTS;

            let engine = tts_guard
                .engine()
                .ok_or_else(|| LLMError::Other("TTS engine not initialized".to_string()))?;

            if let Some(kokoro) = engine.as_any().downcast_ref::<KokoroTTS>() {
                let voice = tts_guard
                    .stats()
                    .get("default_voice")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                // Create ONE stream for all sentences
                let (mut sink, mut stream) = kokoro
                    .create_stream(&voice)
                    .await
                    .map_err(|e| LLMError::Other(format!("Failed to create TTS stream: {}", e)))?;

                // Spawn a task to consume the stream continuously
                tokio::spawn(async move {
                    while let Some((audio, _took)) = stream.next().await {
                        callback(audio);
                    }
                });

                // Submit all sentences to the same sink
                for sentence in sentences {
                    sink.synth(sentence)
                        .await
                        .map_err(|e| LLMError::Other(format!("Failed to submit text: {}", e)))?;
                }

                return Ok(());
            }

            Err(LLMError::Other("TTS engine is not KokoroTTS".to_string()))
        }

        #[cfg(not(feature = "kokoro"))]
        {
            Err(LLMError::Other("Kokoro feature not enabled".to_string()))
        }
    }

    /// 检查是否配置了 TTS 插件
    /// Check if the TTS plugin is configured
    pub fn has_tts(&self) -> bool {
        self.tts_plugin.is_some()
    }

    /// Interrupt currently playing TTS audio
    ///
    /// Stops current audio playback and cancels any ongoing TTS synthesis.
    /// Call this before starting a new TTS request for clean transition.
    ///
    /// # Example
    /// ```rust,ignore
    /// // User enters new input while audio is playing
    /// agent.interrupt_tts().await?;
    /// agent.chat_with_tts(&session_id, new_input).await?;
    /// ```
    pub async fn interrupt_tts(&self) -> LLMResult<()> {
        let mut session_guard = self.active_tts_session.lock().await;
        if let Some(session) = session_guard.take() {
            session.cancel();
        }
        Ok(())
    }

    // ========================================================================
    // LLM + TTS 流式对话方法
    // LLM + TTS Streaming Dialogue Methods
    // ========================================================================

    /// 流式聊天并自动 TTS 播放（最简版本）
    /// Streaming chat with automatic TTS playback (simplest version)
    ///
    /// 自动处理：
    /// Automatic processing:
    /// - 流式 LLM 输出
    /// - Streaming LLM output
    /// - 按标点断句
    /// - Sentence segmenting by punctuation
    /// - 批量 TTS 播放
    /// - Batch TTS playback
    ///
    /// # 示例
    /// # Example
    /// ```rust,ignore
    /// agent.chat_with_tts(&session_id, "你好").await?;
    /// ```
    pub async fn chat_with_tts(
        &self,
        session_id: &str,
        message: impl Into<String>,
    ) -> LLMResult<()> {
        self.chat_with_tts_internal(session_id, message, None).await
    }

    /// 流式聊天并自动 TTS 播放（自定义音频处理）
    /// Streaming chat with automatic TTS playback (custom audio processing)
    ///
    /// # 示例
    /// # Example
    /// ```rust,ignore
    /// use rodio::buffer::SamplesBuffer;
    ///
    /// agent.chat_with_tts_callback(&session_id, "你好", |audio| {
    ///     sink.append(SamplesBuffer::new(1, 24000, audio));
    /// }).await?;
    /// ```
    pub async fn chat_with_tts_callback(
        &self,
        session_id: &str,
        message: impl Into<String>,
        callback: impl Fn(Vec<f32>) + Send + Sync + 'static,
    ) -> LLMResult<()> {
        self.chat_with_tts_internal(session_id, message, Some(Box::new(callback)))
            .await
    }

    /// 创建实时 TTS 流
    /// Create a real-time TTS stream
    ///
    /// 返回的 handle 允许 incremental 提交文本，实现真正的实时流式 TTS。
    /// The returned handle allows incremental text submission for true streaming TTS.
    ///
    /// # 核心机制
    /// # Core Mechanism
    /// 1. 创建 TTS stream（仅一次）
    /// 1. Create TTS stream (only once)
    /// 2. 启动消费者任务（持续接收音频块）
    /// 2. Start consumer task (continuously receiving audio chunks)
    /// 3. 返回的 sink 支持多次 `synth()` 调用
    /// 3. The returned sink supports multiple `synth()` calls
    #[cfg(feature = "kokoro")]
    async fn create_tts_stream_handle(
        &self,
        callback: Box<dyn Fn(Vec<f32>) + Send + Sync>,
        cancellation_token: Option<CancellationToken>,
    ) -> LLMResult<TTSStreamHandle> {
        use mofa_plugins::tts::kokoro_wrapper::KokoroTTS;

        let tts = self
            .tts_plugin
            .as_ref()
            .ok_or_else(|| LLMError::Other("TTS plugin not configured".to_string()))?;

        let tts_guard = tts.lock().await;
        let engine = tts_guard
            .engine()
            .ok_or_else(|| LLMError::Other("TTS engine not initialized".to_string()))?;

        let kokoro = engine
            .as_any()
            .downcast_ref::<KokoroTTS>()
            .ok_or_else(|| LLMError::Other("TTS engine is not KokoroTTS".to_string()))?;

        let voice = tts_guard
            .stats()
            .get("default_voice")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        // 创建 TTS stream（只创建一次）
        // Create TTS stream (only created once)
        let (sink, mut stream) = kokoro
            .create_stream(&voice)
            .await
            .map_err(|e| LLMError::Other(format!("Failed to create TTS stream: {}", e)))?;

        // Clone cancellation token for the spawned task
        let token_clone = cancellation_token.as_ref().map(|t| t.clone_token());

        // 启动消费者任务（持续接收音频块，支持取消检查）
        // Start consumer task (receiving audio chunks with cancellation support)
        let stream_handle = tokio::spawn(async move {
            while let Some((audio, _took)) = stream.next().await {
                // 检查取消信号
                // Check cancellation signal
                if let Some(ref token) = token_clone
                    && token.is_cancelled()
                {
                    break; // 退出循环，停止音频处理
                    // Exit loop, stop audio processing
                }
                callback(audio);
            }
        });

        Ok(TTSStreamHandle {
            sink,
            _stream_handle: stream_handle,
        })
    }

    /// 内部实现：LLM + TTS 实时流式对话
    /// Internal implementation: LLM + TTS real-time streaming dialogue
    ///
    /// # 核心机制
    /// # Core Mechanism
    /// 1. 在 LLM 流式输出**之前**创建 TTS stream
    /// 1. Create TTS stream BEFORE LLM streaming output
    /// 2. 检测到完整句子时立即提交到 TTS
    /// 2. Submit to TTS immediately when a full sentence is detected
    /// 3. LLM 流和 TTS 流并行运行
    /// 3. LLM stream and TTS stream run in parallel
    async fn chat_with_tts_internal(
        &self,
        session_id: &str,
        message: impl Into<String>,
        callback: Option<Box<dyn Fn(Vec<f32>) + Send + Sync>>,
    ) -> LLMResult<()> {
        #[cfg(feature = "kokoro")]
        {
            use mofa_plugins::tts::kokoro_wrapper::KokoroTTS;

            let callback = match callback {
                Some(cb) => cb,
                None => {
                    // 无 TTS 请求，仅流式输出文本
                    // No TTS request, only stream text output
                    let mut text_stream =
                        self.chat_stream_with_session(session_id, message).await?;
                    while let Some(result) = text_stream.next().await {
                        match result {
                            Ok(text_chunk) => {
                                print!("{}", text_chunk);
                                std::io::stdout().flush().map_err(|e| {
                                    LLMError::Other(format!("Failed to flush stdout: {}", e))
                                })?;
                            }
                            Err(e) if e.to_string().contains("__stream_end__") => break,
                            Err(e) => return Err(e),
                        }
                    }
                    println!();
                    return Ok(());
                }
            };

            // Step 0: 取消任何现有的 TTS 会话
            // Step 0: Cancel any existing TTS sessions
            self.interrupt_tts().await?;

            // Step 1: 创建 cancellation token
            // Step 1: Create cancellation token
            let cancellation_token = CancellationToken::new();

            // Step 2: 在 LLM 流式输出之前创建 TTS stream（传入 cancellation token）
            // Step 2: Create TTS stream before LLM output (passing cancellation token)
            let mut tts_handle = self
                .create_tts_stream_handle(callback, Some(cancellation_token.clone_token()))
                .await?;

            // Step 3: 创建并跟踪新的 TTS session
            // Step 3: Create and track a new TTS session
            let session = TTSSession::new(cancellation_token);

            {
                let mut active_session = self.active_tts_session.lock().await;
                *active_session = Some(session);
            }

            let mut buffer = SentenceBuffer::new();

            // Step 4: 流式处理 LLM 响应，实时提交句子到 TTS
            // Step 4: Stream LLM response, submitting sentences to TTS in real-time
            let mut text_stream = self.chat_stream_with_session(session_id, message).await?;

            while let Some(result) = text_stream.next().await {
                match result {
                    Ok(text_chunk) => {
                        // 检查是否已被取消
                        // Check if it has been cancelled
                        {
                            let active_session = self.active_tts_session.lock().await;
                            if let Some(ref session) = *active_session
                                && !session.is_active()
                            {
                                return Ok(()); // 优雅退出
                                // Graceful exit
                            }
                        }

                        // 实时显示文本
                        // Display text in real-time
                        print!("{}", text_chunk);
                        std::io::stdout().flush().map_err(|e| {
                            LLMError::Other(format!("Failed to flush stdout: {}", e))
                        })?;

                        // 检测句子并立即提交到 TTS
                        // Detect sentence and submit to TTS immediately
                        if let Some(sentence) = buffer.push(&text_chunk)
                            && let Err(e) = tts_handle.sink.synth(sentence).await
                        {
                            eprintln!("[TTS Error] Failed to submit sentence: {}", e);
                            // 继续流式处理，即使 TTS 失败
                            // Continue streaming even if TTS fails
                        }
                    }
                    Err(e) if e.to_string().contains("__stream_end__") => break,
                    Err(e) => return Err(e),
                }
            }

            // Step 5: 提交剩余文本
            // Step 5: Submit remaining text
            if let Some(remaining) = buffer.flush()
                && let Err(e) = tts_handle.sink.synth(remaining).await
            {
                eprintln!("[TTS Error] Failed to submit final sentence: {}", e);
            }

            // Step 6: 清理会话
            // Step 6: Clean up the session
            {
                let mut active_session = self.active_tts_session.lock().await;
                *active_session = None;
            }

            // Step 7: 等待 TTS 流完成（所有音频块处理完毕）
            // Step 7: Wait for TTS stream completion (all audio blocks processed)
            let _ = tokio::time::timeout(
                tokio::time::Duration::from_secs(30),
                tts_handle._stream_handle,
            )
            .await
            .map_err(|_| LLMError::Other("TTS stream processing timeout".to_string()))
            .and_then(|r| r.map_err(|e| LLMError::Other(format!("TTS stream task failed: {}", e))));

            Ok(())
        }

        #[cfg(not(feature = "kokoro"))]
        {
            // 当 kokoro feature 未启用时，使用批量处理模式
            // When kokoro feature is disabled, use batch processing mode
            let mut text_stream = self.chat_stream_with_session(session_id, message).await?;
            let mut buffer = SentenceBuffer::new();
            let mut sentences = Vec::new();

            // 收集所有句子
            // Collect all sentences
            while let Some(result) = text_stream.next().await {
                match result {
                    Ok(text_chunk) => {
                        print!("{}", text_chunk);
                        std::io::stdout().flush().map_err(|e| {
                            LLMError::Other(format!("Failed to flush stdout: {}", e))
                        })?;

                        if let Some(sentence) = buffer.push(&text_chunk) {
                            sentences.push(sentence);
                        }
                    }
                    Err(e) if e.to_string().contains("__stream_end__") => break,
                    Err(e) => return Err(e),
                }
            }

            // 添加剩余内容
            // Add remaining content
            if let Some(remaining) = buffer.flush() {
                sentences.push(remaining);
            }

            // 批量播放 TTS（如果有回调）
            // Batch play TTS (if callback is provided)
            if !sentences.is_empty()
                && let Some(cb) = callback
            {
                for sentence in &sentences {
                    println!("\n[TTS] {}", sentence);
                }
                // 注意：非 kokoro 环境下无法调用此方法
                // Note: This method cannot be called in non-kokoro environments
                // 这里需要根据实际情况处理
                // Needs to be handled according to actual situation here
                let _ = cb;
            }

            Ok(())
        }
    }

    /// 内部方法：获取会话 Arc
    /// Internal method: Get session Arc
    async fn get_session_arc(&self, session_id: &str) -> LLMResult<Arc<RwLock<ChatSession>>> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| LLMError::Other(format!("Session '{}' not found", session_id)))
    }

    // ========================================================================
    // 对话方法
    // Dialogue Methods
    // ========================================================================

    /// 发送消息并获取响应（使用当前活动会话）
    /// Send message and get response (using current active session)
    pub async fn chat(&self, message: impl Into<String>) -> LLMResult<String> {
        let session_id = self.active_session_id.read().await.clone();
        self.chat_with_session(&session_id, message).await
    }

    /// 使用指定会话发送消息并获取响应
    /// Send message and get response using specified session
    ///
    /// # 参数
    /// # Parameters
    /// - `session_id`: 会话唯一标识
    /// - `session_id`: Unique session identifier
    /// - `message`: 用户消息
    /// - `message`: User message
    ///
    /// # 示例
    /// # Example
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

        // 获取模型名称
        // Get model name
        let model = self.provider.default_model();

        // 调用 before_chat 钩子（带模型名称）
        // Call before_chat hook (with model name)
        let processed_message = if let Some(ref handler) = self.event_handler {
            match handler.before_chat_with_model(&message, model).await? {
                Some(msg) => msg,
                None => return Ok(String::new()),
            }
        } else {
            message
        };

        // 获取会话
        // Get session
        let session = self.get_session_arc(session_id).await?;

        // 发送消息
        // Send message
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

        // 调用 after_chat 钩子（带元数据）
        // Call after_chat hook (with metadata)
        let final_response = if let Some(ref handler) = self.event_handler {
            // 从会话中获取响应元数据
            // Get response metadata from the session
            let metadata = session_guard.last_response_metadata();
            if let Some(meta) = metadata {
                match handler.after_chat_with_metadata(&response, meta).await? {
                    Some(resp) => resp,
                    None => response,
                }
            } else {
                // 回退到旧方法（没有元数据）
                // Fall back to old method (no metadata)
                match handler.after_chat(&response).await? {
                    Some(resp) => resp,
                    None => response,
                }
            }
        } else {
            response
        };

        Ok(final_response)
    }

    /// 简单问答（不保留上下文）
    /// Simple Q&A (no context retained)
    pub async fn ask(&self, question: impl Into<String>) -> LLMResult<String> {
        let question = question.into();

        let mut builder = self.client.chat();

        // 使用动态 Prompt 模板（如果可用）
        // Use dynamic prompt template (if available)
        let mut system_prompt = self.config.system_prompt.clone();

        if let Some(ref plugin) = self.prompt_plugin
            && let Some(template) = plugin.get_current_template().await
        {
            // 渲染默认模板（可以根据需要添加变量）
            // Render default template (variables can be added as needed)
            match template.render(&[]) {
                Ok(prompt) => system_prompt = Some(prompt),
                Err(_) => {
                    // 如果渲染失败，使用回退的系统提示词
                    // If rendering fails, use fallback system prompt
                    system_prompt = self.config.system_prompt.clone();
                }
            }
        }

        // 设置系统提示词
        // Set system prompt
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
            let response = builder.send_with_tools().await?;
            return response
                .content()
                .map(|s| s.to_string())
                .ok_or_else(|| LLMError::Other("No content in response".to_string()));
        }

        let response = builder.send().await?;
        response
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| LLMError::Other("No content in response".to_string()))
    }

    /// 设置 Prompt 场景
    /// Set prompt scenario
    pub async fn set_prompt_scenario(&self, scenario: impl Into<String>) {
        let scenario = scenario.into();

        if let Some(ref plugin) = self.prompt_plugin {
            plugin.set_active_scenario(&scenario).await;
        }
    }

    /// 清空对话历史（当前活动会话）
    /// Clear conversation history (for the current active session)
    pub async fn clear_history(&self) {
        let session_id = self.active_session_id.read().await.clone();
        let _ = self.clear_session_history(&session_id).await;
    }

    /// 清空指定会话的对话历史
    /// Clear the conversation history of a specified session
    pub async fn clear_session_history(&self, session_id: &str) -> LLMResult<()> {
        let session = self.get_session_arc(session_id).await?;
        let mut session_guard = session.write().await;
        session_guard.clear();
        Ok(())
    }

    /// 获取对话历史（当前活动会话）
    /// Retrieve conversation history (for the current active session)
    pub async fn history(&self) -> Vec<ChatMessage> {
        let session_id = self.active_session_id.read().await.clone();
        self.get_session_history(&session_id)
            .await
            .unwrap_or_default()
    }

    /// 获取指定会话的对话历史
    /// Retrieve the conversation history of a specified session
    pub async fn get_session_history(&self, session_id: &str) -> LLMResult<Vec<ChatMessage>> {
        let session = self.get_session_arc(session_id).await?;
        let session_guard = session.read().await;
        Ok(session_guard.messages().to_vec())
    }

    /// 设置工具
    /// Set up tools
    pub fn set_tools(&mut self, tools: Vec<Tool>, executor: Arc<dyn ToolExecutor>) {
        self.tools = tools;
        self.tool_executor = Some(executor);

        // 更新 session 中的工具
        // Update the tools within the session
        // 注意：这需要重新创建 session，因为 with_tools 消耗 self
        // Note: This requires session recreation as with_tools consumes self
    }

    /// 设置事件处理器
    /// Set up the event handler
    pub fn set_event_handler(&mut self, handler: Box<dyn LLMAgentEventHandler>) {
        self.event_handler = Some(handler);
    }

    /// 向智能体添加插件
    /// Add a plugin to the agent
    pub fn add_plugin<P: AgentPlugin + 'static>(&mut self, plugin: P) {
        self.plugins.push(Box::new(plugin));
    }

    /// 向智能体添加插件列表
    /// Add a list of plugins to the agent
    pub fn add_plugins(&mut self, plugins: Vec<Box<dyn AgentPlugin>>) {
        self.plugins.extend(plugins);
    }

    // ========================================================================
    // 流式对话方法
    // Streaming Dialogue Methods
    // ========================================================================

    /// 流式问答（不保留上下文）
    /// Streaming Q&A (without context retention)
    ///
    /// 返回一个 Stream，每次 yield 一个文本片段
    /// Returns a Stream that yields a text fragment each time
    ///
    /// # 示例
    /// # Example
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
        // Send a streaming request
        let chunk_stream = builder.send_stream().await?;

        // 转换为纯文本流
        // Convert to a plain text stream
        Ok(Self::chunk_stream_to_text_stream(chunk_stream))
    }

    /// 流式多轮对话（保留上下文）
    /// Streaming multi-turn dialogue (with context retention)
    ///
    /// 注意：流式对话会在收到完整响应后更新历史记录
    /// Note: Streaming dialogue updates history after receiving full response
    ///
    /// # 示例
    /// # Example
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
    /// Use a specified session for streaming multi-turn dialogue
    ///
    /// # 参数
    /// # Parameters
    /// - `session_id`: 会话唯一标识
    /// - `session_id`: Unique identifier for the session
    /// - `message`: 用户消息
    /// - `message`: User message
    pub async fn chat_stream_with_session(
        &self,
        session_id: &str,
        message: impl Into<String>,
    ) -> LLMResult<TextStream> {
        let message = message.into();

        // 获取模型名称
        // Retrieve the model name
        let model = self.provider.default_model();

        // 调用 before_chat 钩子（带模型名称）
        // Invoke before_chat hook (with model name)
        let processed_message = if let Some(ref handler) = self.event_handler {
            match handler.before_chat_with_model(&message, model).await? {
                Some(msg) => msg,
                None => return Ok(Box::pin(futures::stream::empty())),
            }
        } else {
            message
        };

        // 获取会话
        // Retrieve the session
        let session = self.get_session_arc(session_id).await?;

        // 获取当前历史
        // Retrieve current history
        let history = {
            let session_guard = session.read().await;
            session_guard.messages().to_vec()
        };

        // 构建请求
        // Construct the request
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
        // Add history messages
        builder = builder.messages(history);
        builder = builder.user(processed_message.clone());

        // 发送流式请求
        // Send a streaming request
        let chunk_stream = builder.send_stream().await?;

        // 在流式处理前，先添加用户消息到历史
        // Add user message to history before stream processing
        {
            let mut session_guard = session.write().await;
            session_guard
                .messages_mut()
                .push(ChatMessage::user(&processed_message));
        }

        // 创建一个包装流，在完成时更新历史并调用事件处理
        // Create a wrapped stream to update history and call events on completion
        let event_handler = self.event_handler.clone().map(Arc::new);
        let wrapped_stream =
            Self::create_history_updating_stream(chunk_stream, session, event_handler);

        Ok(wrapped_stream)
    }

    /// 获取原始流式响应块（包含完整信息）
    /// Retrieve raw streaming response chunks (including full info)
    ///
    /// 如果需要访问工具调用等详细信息，使用此方法
    /// Use this method if detailed info like tool calls is required
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
    /// Stream dialogue and collect full response (using active session)
    ///
    /// 同时返回流和一个 channel 用于获取完整响应
    /// Returns both the stream and a channel to retrieve the full response
    ///
    /// # 示例
    /// # Example
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
    /// Use a specified session for streaming and full response collection
    ///
    /// # 参数
    /// # Parameters
    /// - `session_id`: 会话唯一标识
    /// - `session_id`: Unique identifier for the session
    /// - `message`: 用户消息
    /// - `message`: User message
    pub async fn chat_stream_with_full_session(
        &self,
        session_id: &str,
        message: impl Into<String>,
    ) -> LLMResult<(TextStream, tokio::sync::oneshot::Receiver<String>)> {
        let message = message.into();

        // 获取模型名称
        // Retrieve the model name
        let model = self.provider.default_model();

        // 调用 before_chat 钩子（带模型名称）
        // Invoke before_chat hook (with model name)
        let processed_message = if let Some(ref handler) = self.event_handler {
            match handler.before_chat_with_model(&message, model).await? {
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
        // Retrieve the session
        let session = self.get_session_arc(session_id).await?;

        // 获取当前历史
        // Retrieve current history
        let history = {
            let session_guard = session.read().await;
            session_guard.messages().to_vec()
        };

        // 构建请求
        // Construct the request
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
        // Add user message to history
        {
            let mut session_guard = session.write().await;
            session_guard
                .messages_mut()
                .push(ChatMessage::user(&processed_message));
        }

        // 创建 channel 用于传递完整响应
        // Create a channel to pass the full response
        let (tx, rx) = tokio::sync::oneshot::channel();

        // 创建收集完整响应的流
        // Create a stream that collects the full response
        let event_handler = self.event_handler.clone().map(Arc::new);
        let wrapped_stream =
            Self::create_collecting_stream(chunk_stream, session, tx, event_handler);

        Ok((wrapped_stream, rx))
    }

    // ========================================================================
    // 内部辅助方法
    // Internal Helper Methods
    // ========================================================================

    /// 将 chunk stream 转换为纯文本 stream
    /// Convert chunk stream into a plain text stream
    fn chunk_stream_to_text_stream(chunk_stream: ChatStream) -> TextStream {
        use futures::StreamExt;

        let text_stream = chunk_stream.filter_map(|result| async move {
            match result {
                Ok(chunk) => {
                    // 提取文本内容
                    // Extract text content
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
    /// Create a stream that updates conversation history
    fn create_history_updating_stream(
        chunk_stream: ChatStream,
        session: Arc<RwLock<ChatSession>>,
        event_handler: Option<Arc<Box<dyn LLMAgentEventHandler>>>,
    ) -> TextStream {
        use super::types::LLMResponseMetadata;

        let collected = Arc::new(tokio::sync::Mutex::new(String::new()));
        let collected_clone = collected.clone();
        let event_handler_clone = event_handler.clone();
        let metadata_collected = Arc::new(tokio::sync::Mutex::new(None::<LLMResponseMetadata>));
        let metadata_collected_clone = metadata_collected.clone();

        let stream = chunk_stream.filter_map(move |result| {
            let collected = collected.clone();
            let event_handler = event_handler.clone();
            let metadata_collected = metadata_collected.clone();
            async move {
                match result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            if choice.finish_reason.is_some() {
                                // 最后一个块包含 usage 数据，保存元数据
                                // The last block contains usage data; save the metadata
                                let metadata = LLMResponseMetadata::from(&chunk);
                                *metadata_collected.lock().await = Some(metadata);
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
                        if let Some(handler) = event_handler {
                            let _ = handler.on_error(&e).await;
                        }
                        Some(Err(e))
                    }
                }
            }
        });

        let stream = stream
            .chain(futures::stream::once(async move {
                let full_response = collected_clone.lock().await.clone();
                let metadata = metadata_collected_clone.lock().await.clone();
                if !full_response.is_empty() {
                    let mut session = session.write().await;
                    session
                        .messages_mut()
                        .push(ChatMessage::assistant(&full_response));

                    // 滑动窗口：裁剪历史消息以保持固定大小
                    // Sliding window: trim historical messages to maintain a fixed size
                    let window_size = session.context_window_size();
                    if window_size.is_some() {
                        let current_messages = session.messages().to_vec();
                        *session.messages_mut() = ChatSession::apply_sliding_window_static(
                            &current_messages,
                            window_size,
                        );
                    }

                    if let Some(handler) = event_handler_clone {
                        if let Some(meta) = &metadata {
                            let _ = handler.after_chat_with_metadata(&full_response, meta).await;
                        } else {
                            let _ = handler.after_chat(&full_response).await;
                        }
                    }
                }
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
    /// Create a stream to collect the full response
    fn create_collecting_stream(
        chunk_stream: ChatStream,
        session: Arc<RwLock<ChatSession>>,
        tx: tokio::sync::oneshot::Sender<String>,
        event_handler: Option<Arc<Box<dyn LLMAgentEventHandler>>>,
    ) -> TextStream {
        use super::types::LLMResponseMetadata;
        use futures::StreamExt;

        let collected = Arc::new(tokio::sync::Mutex::new(String::new()));
        let collected_clone = collected.clone();
        let event_handler_clone = event_handler.clone();
        let metadata_collected = Arc::new(tokio::sync::Mutex::new(None::<LLMResponseMetadata>));
        let metadata_collected_clone = metadata_collected.clone();

        let stream = chunk_stream.filter_map(move |result| {
            let collected = collected.clone();
            let event_handler = event_handler.clone();
            let metadata_collected = metadata_collected.clone();
            async move {
                match result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            if choice.finish_reason.is_some() {
                                // 最后一个块包含 usage 数据，保存元数据
                                // The last block contains usage data; save the metadata
                                let metadata = LLMResponseMetadata::from(&chunk);
                                *metadata_collected.lock().await = Some(metadata);
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
                        if let Some(handler) = event_handler {
                            let _ = handler.on_error(&e).await;
                        }
                        Some(Err(e))
                    }
                }
            }
        });

        // 在流结束后更新历史并发送完整响应
        // Update history and send full response after stream ends
        let stream = stream
            .chain(futures::stream::once(async move {
                let full_response = collected_clone.lock().await.clone();
                let mut processed_response = full_response.clone();
                let metadata = metadata_collected_clone.lock().await.clone();

                if !full_response.is_empty() {
                    let mut session = session.write().await;
                    session
                        .messages_mut()
                        .push(ChatMessage::assistant(&processed_response));

                    // 滑动窗口：裁剪历史消息以保持固定大小
                    // Sliding window: trim historical messages to maintain a fixed size
                    let window_size = session.context_window_size();
                    if window_size.is_some() {
                        let current_messages = session.messages().to_vec();
                        *session.messages_mut() = ChatSession::apply_sliding_window_static(
                            &current_messages,
                            window_size,
                        );
                    }

                    // 调用 after_chat 钩子（带元数据）
                    // Invoke after_chat hook (with metadata)
                    if let Some(handler) = event_handler_clone {
                        if let Some(meta) = &metadata {
                            if let Ok(Some(resp)) = handler
                                .after_chat_with_metadata(&processed_response, meta)
                                .await
                            {
                                processed_response = resp;
                            }
                        } else if let Ok(Some(resp)) = handler.after_chat(&processed_response).await
                        {
                            processed_response = resp;
                        }
                    }
                }

                let _ = tx.send(processed_response);

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
/// LLM Agent Builder
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
    session_id: Option<String>,
    user_id: Option<String>,
    tenant_id: Option<String>,
    context_window_size: Option<usize>,
    /// 持久化存储（用于从数据库加载历史会话）
    /// Persistent storage (used for loading historical sessions from database)
    message_store: Option<Arc<dyn crate::persistence::MessageStore + Send + Sync>>,
    session_store: Option<Arc<dyn crate::persistence::SessionStore + Send + Sync>>,
    persistence_user_id: Option<uuid::Uuid>,
    persistence_tenant_id: Option<uuid::Uuid>,
    persistence_agent_id: Option<uuid::Uuid>,
}

impl Default for LLMAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LLMAgentBuilder {
    /// 创建新的构建器
    /// Create a new builder
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
            session_id: None,
            user_id: None,
            tenant_id: None,
            context_window_size: None,
            message_store: None,
            session_store: None,
            persistence_user_id: None,
            persistence_tenant_id: None,
            persistence_agent_id: None,
        }
    }

    /// 设置id
    /// Set the ID
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.agent_id = id.into();
        self
    }

    /// 设置名称
    /// Set the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置 LLM Provider
    /// Set the LLM Provider
    pub fn with_provider(mut self, provider: Arc<dyn LLMProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 设置系统提示词
    /// Set the system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置温度
    /// Set the temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// 设置最大 token 数
    /// Set the maximum number of tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// 添加工具
    /// Add a tool
    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    /// 设置工具列表
    /// Set the tool list
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    /// 设置工具执行器
    /// Set the tool executor
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// 设置事件处理器
    /// Set the event handler
    pub fn with_event_handler(mut self, handler: Box<dyn LLMAgentEventHandler>) -> Self {
        self.event_handler = Some(handler);
        self
    }

    /// 添加插件
    /// Add a plugin
    pub fn with_plugin(mut self, plugin: impl AgentPlugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// 添加插件列表
    /// Add a list of plugins
    pub fn with_plugins(mut self, plugins: Vec<Box<dyn AgentPlugin>>) -> Self {
        self.plugins.extend(plugins);
        self
    }

    /// 添加持久化插件（便捷方法）
    /// Add persistence plugin (convenience method)
    ///
    /// 持久化插件实现了 AgentPlugin trait，同时也是一个 LLMAgentEventHandler，
    /// The persistence plugin implements AgentPlugin and is also an LLMAgentEventHandler,
    /// 会自动注册到 agent 的插件列表和事件处理器中。
    /// automatically registering into the agent's plugin list and event handler.
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_sdk::persistence::{PersistencePlugin, PostgresStore};
    /// use mofa_sdk::llm::LLMAgentBuilder;
    /// use std::sync::Arc;
    /// use uuid::Uuid;
    ///
    /// # async fn example() -> GlobalResult<()> {
    /// let store = Arc::new(PostgresStore::connect("postgres://localhost/mofa").await?);
    /// let user_id = Uuid::now_v7();
    /// let tenant_id = Uuid::now_v7();
    /// let agent_id = Uuid::now_v7();
    /// let session_id = Uuid::now_v7();
    ///
    /// let plugin = PersistencePlugin::new(
    ///     "persistence-plugin",
    ///     store,
    ///     user_id,
    ///     tenant_id,
    ///     agent_id,
    ///     session_id,
    /// );
    ///
    /// let agent = LLMAgentBuilder::new()
    ///     .with_id("my-agent")
    ///     .with_persistence_plugin(plugin)
    ///     .build_async()
    ///     .await;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_persistence_plugin(
        mut self,
        plugin: crate::persistence::PersistencePlugin,
    ) -> Self {
        self.message_store = Some(plugin.message_store());
        self.session_store = plugin.session_store();
        self.persistence_user_id = Some(plugin.user_id());
        self.persistence_tenant_id = Some(plugin.tenant_id());
        self.persistence_agent_id = Some(plugin.agent_id());

        // 将持久化插件添加到插件列表
        // Add the persistence plugin to the plugin list
        // 同时作为事件处理器
        // Also serves as an event handler
        let plugin_box: Box<dyn AgentPlugin> = Box::new(plugin.clone());
        let event_handler: Box<dyn LLMAgentEventHandler> = Box::new(plugin);
        self.plugins.push(plugin_box);
        self.event_handler = Some(event_handler);
        self
    }

    /// 设置 Prompt 模板插件
    /// Set the Prompt template plugin
    pub fn with_prompt_plugin(
        mut self,
        plugin: impl prompt::PromptTemplatePlugin + 'static,
    ) -> Self {
        self.prompt_plugin = Some(Box::new(plugin));
        self
    }

    /// 设置支持热重载的 Prompt 模板插件
    /// Set a hot-reloadable Prompt template plugin
    pub fn with_hot_reload_prompt_plugin(
        mut self,
        plugin: prompt::HotReloadableRhaiPromptPlugin,
    ) -> Self {
        self.prompt_plugin = Some(Box::new(plugin));
        self
    }

    /// 添加自定义配置
    /// Add custom configuration
    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_config.insert(key.into(), value.into());
        self
    }

    /// 设置初始会话 ID
    /// Set the initial session ID
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = LLMAgentBuilder::new()
    ///     .with_id("my-agent")
    ///     .with_initial_session_id("user-session-001")
    ///     .build();
    /// ```
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// 设置用户 ID
    /// Set the user ID
    ///
    /// 用于数据库持久化和多用户场景的消息隔离。
    /// Used for database persistence and message isolation in multi-user scenarios.
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = LLMAgentBuilder::new()
    ///     .with_id("my-agent")
    ///     .with_user("user-123")
    ///     .build();
    /// ```
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// 设置租户 ID
    /// Set the tenant ID
    ///
    /// 用于多租户支持，实现不同租户的数据隔离。
    /// Used for multi-tenant support to achieve data isolation between tenants.
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = LLMAgentBuilder::new()
    ///     .with_id("my-agent")
    ///     .with_tenant("tenant-abc")
    ///     .build();
    /// ```
    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// 设置上下文窗口大小（滑动窗口）
    /// Set context window size (sliding window)
    ///
    /// 用于滑动窗口消息管理，指定保留的最大对话轮数。
    /// Used for sliding window management, specifying the max conversation rounds to keep.
    /// 当消息历史超过此大小时，会自动裁剪较早的消息。
    /// Older messages are automatically trimmed when history exceeds this size.
    ///
    /// # 参数
    /// # Parameters
    /// - `size`: 上下文窗口大小（单位：轮数，rounds）
    /// - `size`: Context window size (unit: rounds)
    ///
    /// # 注意
    /// # Note
    /// - 单位是**轮数**（rounds），不是 token 数量
    /// - The unit is **rounds**, not token count
    /// - 每轮对话 ≈ 1 个用户消息 + 1 个助手响应
    /// - Each round ≈ 1 user message + 1 assistant response
    /// - 系统消息始终保留，不计入轮数限制
    /// - System messages are always kept and do not count toward the round limit
    /// - 从数据库加载消息时也会应用此限制
    /// - This limit is also applied when loading messages from the database
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = LLMAgentBuilder::new()
    ///     .with_id("my-agent")
    ///     .with_sliding_window(10)  // 只保留最近 10 轮对话
    ///     .build();
    /// ```
    pub fn with_sliding_window(mut self, size: usize) -> Self {
        self.context_window_size = Some(size);
        self
    }

    /// 从环境变量创建基础配置
    /// Create basic configuration from environment variables
    ///
    /// 自动配置：
    /// Automatic configuration:
    /// - OpenAI Provider（从 OPENAI_API_KEY）
    /// - OpenAI Provider (via OPENAI_API_KEY)
    /// - 默认 temperature (0.7) 和 max_tokens (4096)
    /// - Default temperature (0.7) and max_tokens (4096)
    ///
    /// # 环境变量
    /// # Environment Variables
    /// - OPENAI_API_KEY: OpenAI API 密钥（必需）
    /// - OPENAI_API_KEY: OpenAI API key (required)
    /// - OPENAI_BASE_URL: 可选的 API 基础 URL
    /// - OPENAI_BASE_URL: Optional API base URL
    /// - OPENAI_MODEL: 可选的默认模型
    /// - OPENAI_MODEL: Optional default model
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_sdk::llm::LLMAgentBuilder;
    ///
    /// let agent = LLMAgentBuilder::from_env()?
    ///     .with_system_prompt("You are a helpful assistant.")
    ///     .build();
    /// ```
    pub fn from_env() -> LLMResult<Self> {
        use super::openai::{OpenAIConfig, OpenAIProvider};

        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            LLMError::ConfigError("OPENAI_API_KEY environment variable not set".to_string())
        })?;

        let mut config = OpenAIConfig::new(api_key);

        if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
            config = config.with_base_url(&base_url);
        }

        if let Ok(model) = std::env::var("OPENAI_MODEL") {
            config = config.with_model(&model);
        }

        Ok(Self::new()
            .with_provider(Arc::new(OpenAIProvider::with_config(config)))
            .with_temperature(0.7)
            .with_max_tokens(4096))
    }

    /// 构建 LLM Agent
    /// Build the LLM Agent
    ///
    /// # Panics
    /// 如果未设置 provider 则 panic
    /// Panics if the provider is not set
    #[must_use]
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
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            context_window_size: self.context_window_size,
        };

        let mut agent = LLMAgent::with_initial_session(config, provider, self.session_id);

        // 设置Prompt模板插件
        // Set Prompt template plugin
        agent.prompt_plugin = self.prompt_plugin;

        if let Some(executor) = self.tool_executor {
            agent.set_tools(self.tools, executor);
        }

        if let Some(handler) = self.event_handler {
            agent.set_event_handler(handler);
        }

        // 处理插件列表：提取 TTS 插件
        // Process plugin list: extract the TTS plugin
        let mut plugins = self.plugins;
        let mut tts_plugin = None;

        // 查找并提取 TTS 插件
        // Find and extract the TTS plugin
        for i in (0..plugins.len()).rev() {
            if plugins[i].as_any().is::<mofa_plugins::tts::TTSPlugin>() {
                // 使用 Any::downcast_ref 检查类型
                // Check type using Any::downcast_ref
                // 由于我们需要获取所有权，这里使用 is 检查后移除
                // Since ownership is needed, remove after checking with 'is'
                let plugin = plugins.remove(i);
                // 尝试 downcast
                // Attempt downcast
                if let Ok(tts) = plugin.into_any().downcast::<mofa_plugins::tts::TTSPlugin>() {
                    tts_plugin = Some(Arc::new(Mutex::new(*tts)));
                }
            }
        }

        // 添加剩余插件
        // Add remaining plugins
        agent.add_plugins(plugins);

        // 设置 TTS 插件
        // Set TTS plugin
        agent.tts_plugin = tts_plugin;

        agent
    }

    /// 尝试构建 LLM Agent
    /// Attempt to build the LLM Agent
    ///
    /// 如果未设置 provider 则返回错误
    /// Returns an error if the provider is not set
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
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            context_window_size: self.context_window_size,
        };

        let mut agent = LLMAgent::with_initial_session(config, provider, self.session_id);

        if let Some(executor) = self.tool_executor {
            agent.set_tools(self.tools, executor);
        }

        if let Some(handler) = self.event_handler {
            agent.set_event_handler(handler);
        }

        // 处理插件列表：提取 TTS 插件
        // Process plugin list: extract the TTS plugin
        let mut plugins = self.plugins;
        let mut tts_plugin = None;

        // 查找并提取 TTS 插件
        // Find and extract the TTS plugin
        for i in (0..plugins.len()).rev() {
            if plugins[i].as_any().is::<mofa_plugins::tts::TTSPlugin>() {
                // 使用 Any::downcast_ref 检查类型
                // Check type using Any::downcast_ref
                // 由于我们需要获取所有权，这里使用 is 检查后移除
                // Since ownership is needed, remove after checking with 'is'
                let plugin = plugins.remove(i);
                // 尝试 downcast
                // Attempt downcast
                if let Ok(tts) = plugin.into_any().downcast::<mofa_plugins::tts::TTSPlugin>() {
                    tts_plugin = Some(Arc::new(Mutex::new(*tts)));
                }
            }
        }

        // 添加剩余插件
        // Add remaining plugins
        agent.add_plugins(plugins);

        // 设置 TTS 插件
        // Set TTS plugin
        agent.tts_plugin = tts_plugin;

        Ok(agent)
    }

    /// 异步构建 LLM Agent（支持从数据库加载会话）
    /// Asynchronously build LLM Agent (supports loading sessions from DB)
    ///
    /// 使用持久化插件加载会话历史。
    /// Use the persistence plugin to load conversation history.
    ///
    /// # 示例（使用持久化插件）
    /// # Example (using persistence plugin)
    ///
    /// ```rust,ignore
    /// use mofa_sdk::persistence::{PersistencePlugin, PostgresStore};
    ///
    /// let store = PostgresStore::connect("postgres://localhost/mofa").await?;
    /// let user_id = Uuid::now_v7();
    /// let tenant_id = Uuid::now_v7();
    /// let agent_id = Uuid::now_v7();
    /// let session_id = Uuid::now_v7();
    ///
    /// let plugin = PersistencePlugin::new(
    ///     "persistence-plugin",
    ///     Arc::new(store),
    ///     user_id,
    ///     tenant_id,
    ///     agent_id,
    ///     session_id,
    /// );
    ///
    /// let agent = LLMAgentBuilder::from_env()?
    ///     .with_system_prompt("You are helpful.")
    ///     .with_persistence_plugin(plugin)
    ///     .build_async()
    ///     .await;
    /// ```
    pub async fn build_async(mut self) -> LLMAgent {
        let provider = self
            .provider
            .expect("LLM provider must be set before building");

        // Clone tenant_id for potential fallback use before moving into config
        // Clone tenant_id for potential fallback use before moving into config
        let tenant_id_for_persistence = self.tenant_id.clone();

        let config = LLMAgentConfig {
            agent_id: self.agent_id.clone(),
            name: self.name.unwrap_or_else(|| self.agent_id.clone()),
            system_prompt: self.system_prompt,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            custom_config: self.custom_config,
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            context_window_size: self.context_window_size,
        };

        // Fallback: If stores are set but persistence_tenant_id is None, use tenant_id
        // Fallback: If stores are set but persistence_tenant_id is None, use tenant_id
        let persistence_tenant_id = if self.session_store.is_some()
            && self.persistence_tenant_id.is_none()
            && let Some(ref tenant_id) = tenant_id_for_persistence
        {
            uuid::Uuid::parse_str(tenant_id).ok()
        } else {
            self.persistence_tenant_id
        };

        // 使用异步方法，支持从数据库加载
        // Use asynchronous method, supporting loading from database
        let mut agent = LLMAgent::with_initial_session_async(
            config,
            provider,
            self.session_id,
            self.message_store,
            self.session_store,
            self.persistence_user_id,
            persistence_tenant_id,
            self.persistence_agent_id,
        )
        .await;

        // 设置Prompt模板插件
        // Set Prompt template plugin
        agent.prompt_plugin = self.prompt_plugin;

        if self.tools.is_empty()
            && let Some(executor) = self.tool_executor.as_ref()
            && let Ok(tools) = executor.available_tools().await
        {
            self.tools = tools;
        }

        if let Some(executor) = self.tool_executor {
            agent.set_tools(self.tools, executor);
        }

        // 处理插件列表：
        // Process plugin list:
        // 1. 从持久化插件加载历史（新方式）
        // 1. Load history from persistence plugin (new way)
        // 2. 提取 TTS 插件
        // 2. Extract TTS plugin
        let mut plugins = self.plugins;
        let mut tts_plugin = None;
        let history_loaded_from_plugin = false;

        // 查找并提取 TTS 插件
        // Find and extract the TTS plugin
        for i in (0..plugins.len()).rev() {
            if plugins[i].as_any().is::<mofa_plugins::tts::TTSPlugin>() {
                // 使用 Any::downcast_ref 检查类型
                // Check type using Any::downcast_ref
                // 由于我们需要获取所有权，这里使用 is 检查后移除
                // Since ownership is needed, remove after checking with 'is'
                let plugin = plugins.remove(i);
                // 尝试 downcast
                // Attempt downcast
                if let Ok(tts) = plugin.into_any().downcast::<mofa_plugins::tts::TTSPlugin>() {
                    tts_plugin = Some(Arc::new(Mutex::new(*tts)));
                }
            }
        }

        // 从持久化插件加载历史（新方式）
        // Load history from persistence plugin (new way)
        if !history_loaded_from_plugin {
            for plugin in &plugins {
                // 通过 metadata 识别持久化插件
                // Identify persistence plugin via metadata
                if plugin.metadata().plugin_type == PluginType::Storage
                    && plugin
                        .metadata()
                        .capabilities
                        .contains(&"message_persistence".to_string())
                {
                    // 这里我们无法直接调用泛型 PersistencePlugin 的 load_history
                    // We cannot directly call the generic PersistencePlugin's load_history
                    // 因为 trait object 无法访问泛型方法
                    // because trait objects cannot access generic methods
                    // 历史加载将由 LLMAgent 在首次运行时通过 store 完成
                    // History loading will be handled by LLMAgent via store on first run
                    tracing::info!("📦 检测到持久化插件，将在 agent 初始化后加载历史");
                    tracing::info!(
                        "📦 Persistence plugin detected; history will load after agent init"
                    );
                    break;
                }
            }
        }

        // 添加剩余插件
        // Add remaining plugins
        agent.add_plugins(plugins);

        // 设置 TTS 插件
        // Set TTS plugin
        agent.tts_plugin = tts_plugin;

        // 设置事件处理器
        // Set event handler
        if let Some(handler) = self.event_handler {
            agent.set_event_handler(handler);
        }

        agent
    }
}

// ============================================================================
// 从配置文件创建
// Create from configuration file
// ============================================================================

impl LLMAgentBuilder {
    /// 从 agent.yml 配置文件创建 Builder
    /// Create Builder from agent.yml configuration file
    ///
    /// # 示例
    /// # Example
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
    /// Create Builder from YAML configuration
    pub fn from_yaml_config(config: crate::config::AgentYamlConfig) -> LLMResult<Self> {
        let mut builder = Self::new()
            .with_id(&config.agent.id)
            .with_name(&config.agent.name);
        // 配置 LLM provider
        // Configure LLM provider
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

    // ========================================================================
    // 数据库加载方法
    // Database loading methods
    // ========================================================================

    /// 从数据库加载 agent 配置（全局查找）
    /// Load agent configuration from the database (global lookup).
    ///
    /// 根据 agent_code 从数据库加载 agent 配置及其关联的 provider。
    /// Loads agent configuration and its associated provider from the database based on agent_code.
    ///
    /// # 参数
    /// # Parameters
    /// - `store`: 实现了 AgentStore 的持久化存储
    /// - `store`: Persistent storage implementing AgentStore
    /// - `agent_code`: Agent 代码（唯一标识）
    /// - `agent_code`: Agent code (unique identifier)
    ///
    /// # 错误
    /// # Errors
    /// - 如果 agent 不存在
    /// - If the agent does not exist
    /// - 如果 agent 被禁用 (agent_status = false)
    /// - If the agent is disabled (agent_status = false)
    /// - 如果 provider 被禁用 (enabled = false)
    /// - If the provider is disabled (enabled = false)
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_sdk::{llm::LLMAgentBuilder, persistence::PostgresStore};
    ///
    /// let store = PostgresStore::from_env().await?;
    /// let agent = LLMAgentBuilder::from_database(&store, "my-agent").await?.build();
    /// ```
    #[cfg(feature = "persistence-postgres")]
    pub async fn from_database<S>(store: &S, agent_code: &str) -> LLMResult<Self>
    where
        S: crate::persistence::AgentStore + Send + Sync,
    {
        let config = store
            .get_agent_by_code_with_provider(agent_code)
            .await
            .map_err(|e| LLMError::Other(format!("Failed to load agent from database: {}", e)))?
            .ok_or_else(|| {
                LLMError::Other(format!(
                    "Agent with code '{}' not found in database",
                    agent_code
                ))
            })?;

        Self::from_agent_config(&config)
    }

    /// 从数据库加载 agent 配置（租户隔离）
    /// Load agent configuration from the database (tenant isolated).
    ///
    /// 根据 tenant_id 和 agent_code 从数据库加载 agent 配置及其关联的 provider。
    /// Loads agent configuration and associated provider from the database using tenant_id and agent_code.
    ///
    /// # 参数
    /// # Parameters
    /// - `store`: 实现了 AgentStore 的持久化存储
    /// - `store`: Persistent storage implementing AgentStore
    /// - `tenant_id`: 租户 ID
    /// - `tenant_id`: Tenant ID
    /// - `agent_code`: Agent 代码
    /// - `agent_code`: Agent code
    ///
    /// # 错误
    /// # Errors
    /// - 如果 agent 不存在
    /// - If the agent does not exist
    /// - 如果 agent 被禁用 (agent_status = false)
    /// - If the agent is disabled (agent_status = false)
    /// - 如果 provider 被禁用 (enabled = false)
    /// - If the provider is disabled (enabled = false)
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_sdk::{llm::LLMAgentBuilder, persistence::PostgresStore};
    /// use uuid::Uuid;
    ///
    /// let store = PostgresStore::from_env().await?;
    /// let tenant_id = Uuid::parse_str("xxx-xxx-xxx")?;
    /// let agent = LLMAgentBuilder::from_database_with_tenant(&store, tenant_id, "my-agent").await?.build();
    /// ```
    #[cfg(feature = "persistence-postgres")]
    pub async fn from_database_with_tenant<S>(
        store: &S,
        tenant_id: uuid::Uuid,
        agent_code: &str,
    ) -> LLMResult<Self>
    where
        S: crate::persistence::AgentStore + Send + Sync,
    {
        let config = store
            .get_agent_by_code_and_tenant_with_provider(tenant_id, agent_code)
            .await
            .map_err(|e| LLMError::Other(format!("Failed to load agent from database: {}", e)))?
            .ok_or_else(|| {
                LLMError::Other(format!(
                    "Agent with code '{}' not found for tenant {}",
                    agent_code, tenant_id
                ))
            })?;

        Self::from_agent_config(&config)
    }

    /// 使用数据库 agent 配置，但允许进一步定制
    /// Use database agent config while allowing further customization.
    ///
    /// 加载数据库配置后，可以继续使用 builder 方法进行定制。
    /// After loading DB config, you can continue customizing using builder methods.
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// let agent = LLMAgentBuilder::with_database_agent(&store, "my-agent")
    ///     .await?
    ///     .with_temperature(0.8)  // 覆盖数据库中的温度设置
    ///     .with_system_prompt("Custom prompt")  // 覆盖系统提示词
    ///     .build();
    /// ```
    #[cfg(feature = "persistence-postgres")]
    pub async fn with_database_agent<S>(store: &S, agent_code: &str) -> LLMResult<Self>
    where
        S: crate::persistence::AgentStore + Send + Sync,
    {
        Self::from_database(store, agent_code).await
    }

    /// 从 AgentConfig 创建 Builder（内部辅助方法）
    /// Create Builder from AgentConfig (internal helper method).
    #[cfg(feature = "persistence-postgres")]
    pub fn from_agent_config(config: &crate::persistence::AgentConfig) -> LLMResult<Self> {
        use super::openai::{OpenAIConfig, OpenAIProvider};

        let agent = &config.agent;
        let provider = &config.provider;

        // 检查 agent 是否启用
        // Check if the agent is enabled.
        if !agent.agent_status {
            return Err(LLMError::Other(format!(
                "Agent '{}' is disabled (agent_status = false)",
                agent.agent_code
            )));
        }

        // 检查 provider 是否启用
        // Check if the provider is enabled.
        if !provider.enabled {
            return Err(LLMError::Other(format!(
                "Provider '{}' is disabled (enabled = false)",
                provider.provider_name
            )));
        }

        // 根据 provider_type 创建 LLM Provider
        // Create LLM Provider based on provider_type.
        let llm_provider: Arc<dyn super::LLMProvider> = match provider.provider_type.as_str() {
            "openai" | "azure" | "compatible" | "local" => {
                let mut openai_config = OpenAIConfig::new(provider.api_key.clone());
                openai_config = openai_config.with_base_url(&provider.api_base);
                openai_config = openai_config.with_model(&agent.model_name);

                if let Some(temp) = agent.temperature {
                    openai_config = openai_config.with_temperature(temp);
                }

                if let Some(max_tokens) = agent.max_completion_tokens {
                    openai_config = openai_config.with_max_tokens(max_tokens as u32);
                }

                Arc::new(OpenAIProvider::with_config(openai_config))
            }
            "anthropic" => {
                let mut cfg = AnthropicConfig::new(provider.api_key.clone());
                cfg = cfg.with_base_url(&provider.api_base);
                cfg = cfg.with_model(&agent.model_name);

                if let Some(temp) = agent.temperature {
                    cfg = cfg.with_temperature(temp);
                }
                if let Some(tokens) = agent.max_completion_tokens {
                    cfg = cfg.with_max_tokens(tokens as u32);
                }

                Arc::new(AnthropicProvider::with_config(cfg))
            }
            "gemini" => {
                let mut cfg = GeminiConfig::new(provider.api_key.clone());
                cfg = cfg.with_base_url(&provider.api_base);
                cfg = cfg.with_model(&agent.model_name);

                if let Some(temp) = agent.temperature {
                    cfg = cfg.with_temperature(temp);
                }
                if let Some(tokens) = agent.max_completion_tokens {
                    cfg = cfg.with_max_tokens(tokens as u32);
                }

                Arc::new(GeminiProvider::with_config(cfg))
            }
            "ollama" => {
                let mut ollama_config = OllamaConfig::new();
                ollama_config = ollama_config.with_base_url(&provider.api_base);
                ollama_config = ollama_config.with_model(&agent.model_name);

                if let Some(temp) = agent.temperature {
                    ollama_config = ollama_config.with_temperature(temp);
                }

                if let Some(max_tokens) = agent.max_completion_tokens {
                    ollama_config = ollama_config.with_max_tokens(max_tokens as u32);
                }

                Arc::new(OllamaProvider::with_config(ollama_config))
            }
            other => {
                return Err(LLMError::Other(format!(
                    "Unsupported provider type: {}",
                    other
                )));
            }
        };

        // 创建基础 builder
        // Create base builder.
        let mut builder = Self::new()
            .with_id(agent.id)
            .with_name(agent.agent_name.clone())
            .with_provider(llm_provider)
            .with_system_prompt(agent.system_prompt.clone())
            .with_tenant(agent.tenant_id.to_string());

        // 设置可选参数
        // Set optional parameters.
        if let Some(temp) = agent.temperature {
            builder = builder.with_temperature(temp);
        }
        if let Some(tokens) = agent.max_completion_tokens {
            builder = builder.with_max_tokens(tokens as u32);
        }
        if let Some(limit) = agent.context_limit {
            builder = builder.with_sliding_window(limit as usize);
        }

        // 处理 custom_params (JSONB) - 将每个 key-value 添加到 custom_config
        // Process custom_params (JSONB) - Add each key-value to custom_config.
        if let Some(ref params) = agent.custom_params
            && let Some(obj) = params.as_object()
        {
            for (key, value) in obj.iter() {
                let value_str: String = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => value.to_string(),
                };
                builder = builder.with_config(key.as_str(), value_str);
            }
        }

        // 处理 response_format
        // Process response_format.
        if let Some(ref format) = agent.response_format {
            builder = builder.with_config("response_format", format);
        }

        // 处理 stream
        // Process stream.
        if let Some(stream) = agent.stream {
            builder = builder.with_config("stream", if stream { "true" } else { "false" });
        }

        Ok(builder)
    }
}

/// 从配置创建 LLM Provider
/// Create LLM Provider from configuration.
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
// MoFAAgent Implementation - New unified microkernel architecture.
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
        // Convert capabilities in metadata to AgentCapabilities.
        // 这里需要使用一个静态的 AgentCapabilities 实例
        // A static AgentCapabilities instance is required here.
        // 或者在 LLMAgent 中存储一个 AgentCapabilities 字段
        // Or store an AgentCapabilities field within LLMAgent.
        // 为了简化，我们创建一个基于当前 metadata 的实现
        // For simplicity, we create an implementation based on current metadata.
        use mofa_kernel::agent::AgentCapabilities;

        // 注意：这里返回的是一个临时引用，实际使用中可能需要调整 LLMAgent 的结构
        // Note: This returns a temporary reference; LLMAgent structure might need adjustment.
        // 来存储一个 AgentCapabilities 实例
        // To store an AgentCapabilities instance.
        // 这里我们使用一个 hack 来返回一个静态实例
        // Here we use a hack to return a static instance.
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
        // 初始化所有插件（load -> init）
        // Initialize all plugins (load -> init).
        let mut plugin_config = mofa_kernel::plugin::PluginConfig::new();
        for (k, v) in &self.config.custom_config {
            plugin_config.set(k, v);
        }
        if let Some(user_id) = &self.config.user_id {
            plugin_config.set("user_id", user_id);
        }
        if let Some(tenant_id) = &self.config.tenant_id {
            plugin_config.set("tenant_id", tenant_id);
        }
        let session_id = self.active_session_id.read().await.clone();
        plugin_config.set("session_id", session_id);

        let plugin_ctx =
            mofa_kernel::plugin::PluginContext::new(self.id()).with_config(plugin_config);

        for plugin in &mut self.plugins {
            plugin
                .load(&plugin_ctx)
                .await
                .map_err(|e| mofa_kernel::agent::AgentError::InitializationFailed(e.to_string()))?;
            plugin
                .init_plugin()
                .await
                .map_err(|e| mofa_kernel::agent::AgentError::InitializationFailed(e.to_string()))?;
        }
        self.state = mofa_kernel::agent::AgentState::Ready;

        // 将上下文信息保存到 metadata（如果需要）
        // Save context information to metadata (if needed).
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
        // Convert AgentInput to string.
        let message = match input {
            AgentInput::Text(text) => text,
            AgentInput::Json(json) => json.to_string(),
            _ => {
                return Err(AgentError::ValidationFailed(
                    "Unsupported input type for LLMAgent".to_string(),
                ));
            }
        };

        // 执行 chat
        // Execute chat.
        let response = self
            .chat(&message)
            .await
            .map_err(|e| AgentError::ExecutionFailed(format!("LLM chat failed: {}", e)))?;

        // 将响应转换为 AgentOutput
        // Convert response to AgentOutput.
        Ok(AgentOutput::text(response))
    }

    async fn shutdown(&mut self) -> mofa_kernel::agent::AgentResult<()> {
        // 销毁所有插件
        // Destroy all plugins.
        for plugin in &mut self.plugins {
            plugin
                .unload()
                .await
                .map_err(|e| mofa_kernel::agent::AgentError::ShutdownFailed(e.to_string()))?;
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
// Convenience Functions.
// ============================================================================

/// 快速创建简单的 LLM Agent
/// Quickly create a simple LLM Agent.
///
/// # 示例
/// # Example
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
/// Create LLM Agent from a configuration file.
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_sdk::llm::agent_from_config;
///
/// let agent = agent_from_config("agent.yml")?;
/// ```
pub fn agent_from_config(path: impl AsRef<std::path::Path>) -> LLMResult<LLMAgent> {
    LLMAgentBuilder::from_config_file(path)?.try_build()
}
