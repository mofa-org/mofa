//! 持久化插件
//! Persistence Plugin
//!
//! 提供与 LLMAgent 集成的持久化功能
//! Provides persistence functionality integrated with LLMAgent

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use super::entities::*;
use super::traits::*;
use crate::llm::types::LLMResponseMetadata;
use crate::llm::{LLMError, LLMResult};
use mofa_kernel::plugin::{
    AgentPlugin, PluginContext, PluginMetadata, PluginResult, PluginState, PluginType,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// 持久化上下文
/// Persistence Context
///
/// 提供对持久化功能的便捷访问
/// Provides convenient access to persistence features
pub struct PersistenceContext<S>
where
    S: MessageStore + ApiCallStore + SessionStore + Send + Sync + 'static,
{
    store: Arc<S>,
    user_id: Uuid,
    agent_id: Uuid,
    tenant_id: Uuid,
    session_id: Uuid,
}

impl<S> PersistenceContext<S>
where
    S: MessageStore + ApiCallStore + SessionStore + Send + Sync + 'static,
{
    /// 创建新的持久化上下文
    /// Create a new persistence context
    pub async fn new(
        store: Arc<S>,
        user_id: Uuid,
        tenant_id: Uuid,
        agent_id: Uuid,
    ) -> LLMResult<Self> {
        let session = ChatSession::new(user_id, agent_id);
        store
            .create_session(&session)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(Self {
            store,
            user_id,
            agent_id,
            tenant_id,
            session_id: session.id,
        })
    }

    /// 从现有会话创建上下文
    /// Create context from an existing session
    pub fn from_session(
        store: Arc<S>,
        user_id: Uuid,
        agent_id: Uuid,
        tenant_id: Uuid,
        session_id: Uuid,
    ) -> Self {
        Self {
            store,
            user_id,
            agent_id,
            tenant_id,
            session_id,
        }
    }

    /// 获取会话 ID
    /// Get the session ID
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// 保存用户消息
    /// Save a user message
    pub async fn save_user_message(&self, content: impl Into<String>) -> LLMResult<Uuid> {
        let message = LLMMessage::new(
            self.session_id,
            self.agent_id,
            self.user_id,
            self.tenant_id,
            MessageRole::User,
            MessageContent::text(content),
        );
        let id = message.id;

        self.store
            .save_message(&message)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(id)
    }

    /// 保存助手消息
    /// Save an assistant message
    pub async fn save_assistant_message(&self, content: impl Into<String>) -> LLMResult<Uuid> {
        let message = LLMMessage::new(
            self.session_id,
            self.agent_id,
            self.user_id,
            self.tenant_id,
            MessageRole::Assistant,
            MessageContent::text(content),
        );
        let id = message.id;

        self.store
            .save_message(&message)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(id)
    }

    /// 获取会话消息历史
    /// Get session message history
    pub async fn get_history(&self) -> LLMResult<Vec<LLMMessage>> {
        self.store
            .get_session_messages(self.session_id)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))
    }

    /// 获取使用统计
    /// Get usage statistics
    pub async fn get_usage_stats(&self) -> LLMResult<UsageStatistics> {
        let filter = QueryFilter::new().session(self.session_id);
        self.store
            .get_statistics(&filter)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))
    }

    /// 创建新会话
    /// Create a new session
    pub async fn new_session(&mut self) -> LLMResult<Uuid> {
        let session = ChatSession::new(self.user_id, self.agent_id);
        self.store
            .create_session(&session)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        self.session_id = session.id;
        Ok(session.id)
    }

    /// 获取存储引用
    /// Get the storage reference
    pub fn store(&self) -> Arc<S> {
        self.store.clone()
    }
}

// ============================================================================
// PersistencePlugin - 实现 AgentPlugin trait
// PersistencePlugin - Implements AgentPlugin trait
// ============================================================================

/// 持久化插件
/// Persistence Plugin
///
/// 实现 AgentPlugin trait，提供完整的持久化能力：
/// Implements AgentPlugin trait, providing full persistence capabilities:
/// - 从数据库加载会话历史
/// - Load session history from the database
/// - 自动记录用户消息、助手消息、API 调用
/// - Auto-log user messages, assistant messages, and API calls
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::persistence::{PersistencePlugin, PostgresStore};
/// use mofa_sdk::llm::LLMAgentBuilder;
/// use uuid::Uuid;
///
/// # async fn example() -> GlobalResult<()> {
/// let store = PostgresStore::connect("postgres://localhost/mofa").await?;
/// let user_id = Uuid::now_v7();
/// let tenant_id = Uuid::now_v7();
/// let agent_id = Uuid::now_v7();
/// let session_id = Uuid::now_v7();
///
/// let plugin = PersistencePlugin::from_store(
///     "persistence-plugin",
///     store,
///     user_id,
///     tenant_id,
///     agent_id,
///     session_id,
/// );
///
/// let agent = LLMAgentBuilder::new()
///     .with_plugin(plugin)
///     .build_async()
///     .await;
/// # Ok(())
/// # }
/// ```
pub struct PersistencePlugin {
    metadata: PluginMetadata,
    state: PluginState,
    message_store: Arc<dyn MessageStore + Send + Sync>,
    api_call_store: Arc<dyn ApiCallStore + Send + Sync>,
    session_store: Option<Arc<dyn SessionStore + Send + Sync>>,
    user_id: Uuid,
    tenant_id: Uuid,
    agent_id: Uuid,
    session_id: Arc<RwLock<Uuid>>,
    current_user_msg_id: Arc<RwLock<Option<Uuid>>>,
    request_start_time: Arc<RwLock<Option<std::time::Instant>>>,
    response_id: Arc<RwLock<Option<String>>>,
    current_model: Arc<RwLock<Option<String>>>,
}

impl PersistencePlugin {
    /// 创建持久化插件
    /// Create a persistence plugin
    ///
    /// # 参数
    /// # Parameters
    /// - `plugin_id`: 插件唯一标识
    /// - `plugin_id`: Unique plugin identifier
    /// - `message_store`: 消息存储后端
    /// - `message_store`: Message storage backend
    /// - `api_call_store`: API 调用存储后端
    /// - `api_call_store`: API call storage backend
    /// - `user_id`: 用户 ID
    /// - `user_id`: User ID
    /// - `tenant_id`: 租户 ID
    /// - `tenant_id`: Tenant ID
    /// - `agent_id`: Agent ID
    /// - `agent_id`: Agent ID
    /// - `session_id`: 会话 ID
    /// - `session_id`: Session ID
    pub fn new(
        plugin_id: &str,
        message_store: Arc<dyn MessageStore + Send + Sync>,
        api_call_store: Arc<dyn ApiCallStore + Send + Sync>,
        user_id: Uuid,
        tenant_id: Uuid,
        agent_id: Uuid,
        session_id: Uuid,
    ) -> Self {
        let metadata = PluginMetadata::new(plugin_id, "Persistence Plugin", PluginType::Storage)
            .with_description("Message and API call persistence plugin")
            .with_capability("message_persistence")
            .with_capability("api_call_logging")
            .with_capability("session_history");

        Self {
            metadata,
            state: PluginState::Loaded,
            message_store,
            api_call_store,
            session_store: None,
            user_id,
            tenant_id,
            agent_id,
            session_id: Arc::new(RwLock::new(session_id)),
            current_user_msg_id: Arc::new(RwLock::new(None)),
            request_start_time: Arc::new(RwLock::new(None)),
            response_id: Arc::new(RwLock::new(None)),
            current_model: Arc::new(RwLock::new(None)),
        }
    }

    /// 创建持久化插件（便捷方法，使用单个存储后端）
    /// Create persistence plugin (convenience method using single storage backend)
    ///
    /// # 参数
    /// # Parameters
    /// - `plugin_id`: 插件唯一标识
    /// - `plugin_id`: Unique plugin identifier
    /// - `store`: 持久化存储后端（需要同时实现 MessageStore、ApiCallStore、SessionStore）
    /// - `store`: Persistence backend (must implement MessageStore, ApiCallStore, SessionStore)
    /// - `user_id`: 用户 ID
    /// - `user_id`: User ID
    /// - `tenant_id`: 租户 ID
    /// - `tenant_id`: Tenant ID
    /// - `agent_id`: Agent ID
    /// - `agent_id`: Agent ID
    /// - `session_id`: 会话 ID
    /// - `session_id`: Session ID
    pub fn from_store<S>(
        plugin_id: &str,
        store: S,
        user_id: Uuid,
        tenant_id: Uuid,
        agent_id: Uuid,
        session_id: Uuid,
    ) -> Self
    where
        S: MessageStore + ApiCallStore + SessionStore + Send + Sync + 'static,
    {
        let store_arc = Arc::new(store);
        let session_store: Arc<dyn SessionStore + Send + Sync> = store_arc.clone();
        let mut plugin = Self::new(
            plugin_id,
            store_arc.clone(),
            store_arc,
            user_id,
            tenant_id,
            agent_id,
            session_id,
        );
        plugin.session_store = Some(session_store);
        plugin
    }

    /// 更新会话 ID
    /// Update the session ID
    pub async fn with_session_id(&self, session_id: Uuid) {
        *self.session_id.write().await = session_id;
    }

    /// 获取当前会话 ID
    /// Get the current session ID
    pub async fn session_id(&self) -> Uuid {
        *self.session_id.read().await
    }

    /// 获取历史消息（用于 build_async）
    /// Get historical messages (used for build_async)
    pub async fn load_history(&self) -> PersistenceResult<Vec<LLMMessage>> {
        self.message_store
            .get_session_messages(*self.session_id.read().await)
            .await
    }

    /// 获取消息存储引用
    /// Get the message store reference
    pub fn message_store(&self) -> Arc<dyn MessageStore + Send + Sync> {
        self.message_store.clone()
    }

    /// 获取 API 调用存储引用
    /// Get the API call store reference
    pub fn api_call_store(&self) -> Arc<dyn ApiCallStore + Send + Sync> {
        self.api_call_store.clone()
    }

    /// 获取会话存储引用
    /// Get the session store reference
    pub fn session_store(&self) -> Option<Arc<dyn SessionStore + Send + Sync>> {
        self.session_store.clone()
    }

    /// 获取用户 ID
    /// Get the user ID
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// 获取租户 ID
    /// Get the tenant ID
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    /// 获取 Agent ID
    /// Get the agent ID
    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    /// 保存消息（内部方法）
    /// Save message (internal method)
    async fn save_message_internal(&self, role: MessageRole, content: &str) -> LLMResult<Uuid> {
        let session_id = *self.session_id.read().await;
        let message = LLMMessage::new(
            session_id,
            self.agent_id,
            self.user_id,
            self.tenant_id,
            role,
            MessageContent::text(content),
        );
        let id = message.id;

        self.message_store
            .save_message(&message)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(id)
    }

    /// 保存用户消息
    /// Save a user message
    pub async fn save_user_message(&self, content: &str) -> LLMResult<Uuid> {
        self.save_message_internal(MessageRole::User, content).await
    }

    /// 保存助手消息
    /// Save an assistant message
    pub async fn save_assistant_message(&self, content: &str) -> LLMResult<Uuid> {
        self.save_message_internal(MessageRole::Assistant, content)
            .await
    }
}

impl Clone for PersistencePlugin {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            state: self.state.clone(),
            message_store: self.message_store.clone(),
            api_call_store: self.api_call_store.clone(),
            session_store: self.session_store.clone(),
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            agent_id: self.agent_id,
            session_id: self.session_id.clone(),
            current_user_msg_id: self.current_user_msg_id.clone(),
            request_start_time: self.request_start_time.clone(),
            response_id: self.response_id.clone(),
            current_model: self.current_model.clone(),
        }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for PersistencePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    async fn load(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn init_plugin(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        self.state = PluginState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded;
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded;
        Ok(())
    }

    async fn execute(&mut self, _input: String) -> PluginResult<String> {
        Ok("persistence plugin".to_string())
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        stats.insert(
            "plugin_type".to_string(),
            serde_json::Value::String("persistence".to_string()),
        );
        stats.insert(
            "user_id".to_string(),
            serde_json::Value::String(self.user_id.to_string()),
        );
        stats.insert(
            "tenant_id".to_string(),
            serde_json::Value::String(self.tenant_id.to_string()),
        );
        stats.insert(
            "agent_id".to_string(),
            serde_json::Value::String(self.agent_id.to_string()),
        );
        stats
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

// 实现 LLMAgentEventHandler trait
// Implements LLMAgentEventHandler trait
#[async_trait::async_trait]
impl crate::llm::agent::LLMAgentEventHandler for PersistencePlugin {
    fn clone_box(&self) -> Box<dyn crate::llm::agent::LLMAgentEventHandler> {
        // 由于 PersistencePlugin 需要 Arc<S>，我们创建一个新的克隆实例
        // As PersistencePlugin needs Arc<S>, we create a new clone instance
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// 在发送用户消息前调用 - 记录用户消息
    /// Called before sending user message - records user message
    async fn before_chat(&self, message: &str) -> LLMResult<Option<String>> {
        // 记录请求开始时间
        // Record request start time
        *self.request_start_time.write().await = Some(std::time::Instant::now());

        // 保存用户消息
        // Save user message
        let user_msg_id = self.save_user_message(message).await?;
        info!(
            "✅ [Persistence Plugin] User message saved: ID = {}",
            user_msg_id
        );
        // ✅ [Persistence Plugin] User message saved: ID = {}

        // 存储当前用户消息 ID，用于后续关联 API 调用
        // Store current user message ID to associate with subsequent API calls
        *self.current_user_msg_id.write().await = Some(user_msg_id);

        Ok(Some(message.to_string()))
    }

    /// 在发送用户消息前调用（带模型名称）- 记录用户消息和模型
    /// Called before sending user message (with model name) - records message and model
    async fn before_chat_with_model(
        &self,
        message: &str,
        model: &str,
    ) -> LLMResult<Option<String>> {
        // 存储模型名称，用于后续的 after_chat 和 on_error
        // Store model name for subsequent after_chat and on_error
        *self.current_model.write().await = Some(model.to_string());

        // 调用原有的 before_chat 逻辑
        // Call existing before_chat logic
        self.before_chat(message).await
    }

    /// 在收到 LLM 响应后调用 - 记录助手消息和 API 调用
    /// Called after receiving LLM response - records assistant message and API call
    async fn after_chat(&self, response: &str) -> LLMResult<Option<String>> {
        // 保存助手消息
        // Save assistant message
        let assistant_msg_id = self.save_assistant_message(response).await?;
        info!(
            "✅ [Persistence Plugin] Assistant message saved: ID = {}",
            assistant_msg_id
        );
        // ✅ [Persistence Plugin] Assistant message saved: ID = {}

        // 计算请求延迟
        // Calculate request latency
        let latency = match *self.request_start_time.read().await {
            Some(start) => start.elapsed().as_millis() as i32,
            None => 0,
        };

        // 获取存储的模型名称，或使用默认值
        // Get stored model name, or use default
        let model = self.current_model.read().await;
        let model_name = model.as_ref().map(|s| s.as_str()).unwrap_or("unknown");

        // 记录 API 调用
        // Record API call
        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let session_id = *self.session_id.read().await;
            let now = chrono::Utc::now();
            let request_time = now - chrono::Duration::milliseconds(latency as i64);

            let api_call = LLMApiCall::success(
                session_id,
                self.agent_id,
                self.user_id,
                self.tenant_id,
                user_msg_id,
                assistant_msg_id,
                model_name,
                0, // 未知（没有元数据时无法获取真实值）
                // Unknown (cannot get real value without metadata)
                response.len() as i32 / 4, // 简单估算 completion_tokens (每4字符一个token)
                // Simple estimation of completion_tokens (1 token per 4 chars)
                request_time,
                now,
            );

            let _ = self
                .api_call_store
                .save_api_call(&api_call)
                .await
                .map_err(|e| LLMError::Other(e.to_string()));
            info!(
                "✅ [Persistence Plugin] API call record saved: model={}, latency={}ms",
                model_name, latency
            );
            // ✅ [Persistence Plugin] API call record saved: model={}, latency={}ms
        }

        // 清理状态
        // Clear state
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.current_model.write().await = None;

        Ok(Some(response.to_string()))
    }

    /// 在收到 LLM 响应后调用 - 记录助手消息和 API 调用（带元数据）
    /// Called after receiving LLM response - records assistant message and API call (with metadata)
    async fn after_chat_with_metadata(
        &self,
        response: &str,
        metadata: &LLMResponseMetadata,
    ) -> LLMResult<Option<String>> {
        // 保存 response_id
        // Save response_id
        *self.response_id.write().await = Some(metadata.id.clone());

        // 保存助手消息
        // Save assistant message
        let assistant_msg_id = self.save_assistant_message(response).await?;
        info!(
            "✅ [Persistence Plugin] Assistant message saved: ID = {}",
            assistant_msg_id
        );
        // ✅ [Persistence Plugin] Assistant message saved: ID = {}

        // 计算请求延迟
        // Calculate request latency
        let latency = match *self.request_start_time.read().await {
            Some(start) => start.elapsed().as_millis() as i32,
            None => 0,
        };

        // 记录 API 调用
        // Record API call
        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let session_id = *self.session_id.read().await;
            let now = chrono::Utc::now();
            let request_time = now - chrono::Duration::milliseconds(latency as i64);

            let mut api_call = LLMApiCall::success(
                session_id,
                self.agent_id,
                self.user_id,
                self.tenant_id,
                user_msg_id,
                assistant_msg_id,
                &metadata.model,
                metadata.prompt_tokens as i32,
                metadata.completion_tokens as i32,
                request_time,
                now,
            );

            // 设置 response_id
            // Set response_id
            api_call = api_call.with_api_response_id(&metadata.id);

            let _ = self
                .api_call_store
                .save_api_call(&api_call)
                .await
                .map_err(|e| LLMError::Other(e.to_string()));
            info!(
                "✅ [Persistence Plugin] API call record saved: model={}, tokens={}/{}, latency={}ms",
                metadata.model, metadata.prompt_tokens, metadata.completion_tokens, latency
            );
            // ✅ [Persistence Plugin] API call record saved: model={}, tokens={}/{}, latency={}ms
        }

        // 清理状态
        // Clear state
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.response_id.write().await = None;

        Ok(Some(response.to_string()))
    }

    /// 在发生错误时调用 - 记录 API 错误
    /// Called when an error occurs - records API error
    async fn on_error(&self, error: &LLMError) -> LLMResult<Option<String>> {
        info!("✅ [Persistence Plugin] Recording API error...");
        // ✅ [Persistence Plugin] Recording API error...

        // 获取存储的模型名称，或使用默认值
        // Get stored model name, or use default
        let model = self.current_model.read().await;
        let model_name = model.as_ref().map(|s| s.as_str()).unwrap_or("unknown");

        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let session_id = *self.session_id.read().await;
            let now = chrono::Utc::now();

            let api_call = LLMApiCall::failed(
                session_id,
                self.agent_id,
                self.user_id,
                self.tenant_id,
                user_msg_id,
                model_name,
                error.to_string(),
                None,
                now,
            );

            let _ = self
                .api_call_store
                .save_api_call(&api_call)
                .await
                .map_err(|e| LLMError::Other(e.to_string()));
            info!("✅ [Persistence Plugin] API error record saved");
            // ✅ [Persistence Plugin] API error record saved
        }

        // 清理状态
        // Clear state
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.current_model.write().await = None;

        Ok(None)
    }
}
