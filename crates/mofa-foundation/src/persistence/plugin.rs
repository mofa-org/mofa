//! 持久化插件
//!
//! 提供与 LLMAgent 集成的持久化功能

use super::entities::*;
use super::traits::*;
use crate::llm::{LLMError, LLMResult};
use crate::llm::types::LLMResponseMetadata;
use mofa_kernel::plugin::{AgentPlugin, PluginContext, PluginMetadata, PluginResult, PluginState, PluginType};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// 持久化回调 trait
///
/// 可以由 LLMAgent 的事件处理器调用
#[async_trait::async_trait]
pub trait PersistenceCallback: Send + Sync {
    /// 记录用户消息
    async fn on_user_message(&self, content: &str) -> LLMResult<Uuid>;

    /// 记录助手消息
    async fn on_assistant_message(&self, content: &str) -> LLMResult<Uuid>;

    /// 记录 API 调用
    async fn on_api_call(
        &self,
        model: &str,
        prompt_tokens: i32,
        completion_tokens: i32,
        request_message_id: Uuid,
        response_message_id: Uuid,
        latency_ms: i32,
        response_id: Option<&str>,
    ) -> LLMResult<Uuid>;

    /// 记录 API 调用错误
    async fn on_api_error(
        &self,
        model: &str,
        request_message_id: Uuid,
        error_message: &str,
    ) -> LLMResult<Uuid>;

    /// 设置会话 ID（用于持久化处理器同步）
    async fn set_session_id(&self, session_id: Uuid);
}

/// LLMAgent 事件处理器的默认持久化实现
///
/// 自动将 Agent 事件转换为持久化操作
#[derive(Clone)]
pub struct AgentPersistenceHandler {
    persistence: Arc<dyn PersistenceCallback>,
    current_user_msg_id: Arc<RwLock<Option<Uuid>>>,
    request_start_time: Arc<RwLock<Option<std::time::Instant>>>,
    response_id: Arc<RwLock<Option<String>>>,
    current_model: Arc<RwLock<Option<String>>>,
}

impl AgentPersistenceHandler {
    /// 创建 Agent 持久化事件处理器
    pub fn new(persistence: Arc<dyn PersistenceCallback>) -> Self {
        Self {
            persistence,
            current_user_msg_id: Arc::new(RwLock::new(None)),
            request_start_time: Arc::new(RwLock::new(None)),
            response_id: Arc::new(RwLock::new(None)),
            current_model: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置会话 ID（转发到内部的 PersistenceHandler）
    pub async fn set_session_id(&self, session_id: Uuid) {
        self.persistence.set_session_id(session_id).await;
    }
}

#[async_trait::async_trait]
impl crate::llm::agent::LLMAgentEventHandler for AgentPersistenceHandler {
    fn clone_box(&self) -> Box<dyn crate::llm::agent::LLMAgentEventHandler> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// 在发送用户消息前调用 - 记录用户消息
    async fn before_chat(&self, message: &str) -> crate::llm::LLMResult<Option<String>> {
        // 记录请求开始时间
        *self.request_start_time.write().await = Some(std::time::Instant::now());

        // 保存用户消息
        let user_msg_id = self.persistence.on_user_message(message).await?;
        info!("✅ [内置持久化] 用户消息已保存: ID = {}", user_msg_id);

        // 存储当前用户消息 ID，用于后续关联 API 调用
        *self.current_user_msg_id.write().await = Some(user_msg_id);

        Ok(Some(message.to_string()))
    }

    /// 在发送用户消息前调用（带模型名称）- 记录用户消息和模型
    async fn before_chat_with_model(
        &self,
        message: &str,
        model: &str,
    ) -> crate::llm::LLMResult<Option<String>> {
        // 存储模型名称，用于后续的 after_chat 和 on_error
        *self.current_model.write().await = Some(model.to_string());

        // 调用原有的 before_chat 逻辑
        self.before_chat(message).await
    }

    /// 在收到 LLM 响应后调用 - 记录助手消息和 API 调用
    async fn after_chat(&self, response: &str) -> crate::llm::LLMResult<Option<String>> {
        // 保存助手消息
        let assistant_msg_id = self.persistence.on_assistant_message(response).await?;
        info!("✅ [内置持久化] 助手消息已保存: ID = {}", assistant_msg_id);
        // 计算请求延迟
        let latency = match *self.request_start_time.read().await {
            Some(start) => start.elapsed().as_millis() as i32,
            None => 0,
        };

        // 获取存储的模型名称，或使用默认值
        let model = self.current_model.read().await;
        let model_name = model.as_ref().map(|s| s.as_str()).unwrap_or("unknown");

        // 记录 API 调用
        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let _ = self.persistence.on_api_call(
                model_name,  // 使用存储的模型名称
                0,           // 未知（没有元数据时无法获取真实值）
                response.len() as i32 / 4,  // 简单估算 completion_tokens (每4字符一个token)
                user_msg_id,
                assistant_msg_id,
                latency,
                None, // response_id 不可用
            ).await?;
            info!("✅ [内置持久化] API 调用记录已保存: 模型={}, 延迟={}ms", model_name, latency);
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.current_model.write().await = None;

        Ok(Some(response.to_string()))
    }

    /// 在收到 LLM 响应后调用 - 记录助手消息和 API 调用（带元数据）
    async fn after_chat_with_metadata(
        &self,
        response: &str,
        metadata: &LLMResponseMetadata,
    ) -> crate::llm::LLMResult<Option<String>> {
        // 保存 response_id
        *self.response_id.write().await = Some(metadata.id.clone());

        // 保存助手消息
        let assistant_msg_id = self.persistence.on_assistant_message(response).await?;
        info!("✅ [内置持久化] 助手消息已保存: ID = {}", assistant_msg_id);

        // 计算请求延迟
        let latency = match *self.request_start_time.read().await {
            Some(start) => start.elapsed().as_millis() as i32,
            None => 0,
        };

        // 记录 API 调用
        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let _ = self.persistence.on_api_call(
                &metadata.model,
                metadata.prompt_tokens as i32,
                metadata.completion_tokens as i32,
                user_msg_id,
                assistant_msg_id,
                latency,
                Some(&metadata.id),
            ).await?;
            info!(
                "✅ [内置持久化] API 调用记录已保存: 模型={}, tokens={}/{}, 延迟={}ms",
                metadata.model, metadata.prompt_tokens, metadata.completion_tokens, latency
            );
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.response_id.write().await = None;

        Ok(Some(response.to_string()))
    }

    /// 在发生错误时调用 - 记录 API 错误
    async fn on_error(&self, error: &crate::llm::LLMError) -> crate::llm::LLMResult<Option<String>> {
        info!("✅ [内置持久化] 记录 API 错误...");

        // 获取存储的模型名称，或使用默认值
        let model = self.current_model.read().await;
        let model_name = model.as_ref().map(|s| s.as_str()).unwrap_or("unknown");

        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let _ = self.persistence.on_api_error(
                model_name,  // 使用存储的模型名称
                user_msg_id,
                &error.to_string(),
            ).await?;
            info!("✅ [内置持久化] API 错误记录已保存");
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.current_model.write().await = None;

        Ok(None)
    }
}

/// 持久化处理器
///
/// 提供便捷的持久化功能封装
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::persistence::{InMemoryStore, PersistenceHandler};
///
/// let store = InMemoryStore::shared();
/// let handler = PersistenceHandler::new(store, user_id, agent_id);
///
/// // 记录消息
/// let msg_id = handler.on_user_message("Hello").await?;
/// let reply_id = handler.on_assistant_message("Hi there!").await?;
/// ```
pub struct PersistenceHandler<S>
where
    S: MessageStore + ApiCallStore + Send + Sync + 'static,
{
    /// 存储后端
    store: Arc<S>,
    /// 用户 ID
    user_id: Uuid,
    /// 租户 ID
    tenant_id: Uuid,
    /// Agent ID
    agent_id: Uuid,
    /// 当前会话 ID（使用内部可变性）
    session_id: Arc<RwLock<Uuid>>,
}

impl<S> PersistenceHandler<S>
where
    S: MessageStore + ApiCallStore + Send + Sync + 'static,
{
    /// 创建持久化处理器
    pub fn new(store: Arc<S>, user_id: Uuid, tenant_id:Uuid, agent_id: Uuid, session_id: Uuid) -> Self {
        Self {
            store,
            user_id,
            tenant_id,
            agent_id,
            session_id: Arc::new(RwLock::new(session_id)),
        }
    }

    /// 为简单场景创建持久化处理器（自动生成 ID）
    ///
    /// 自动生成 user_id 和 agent_id，适用于单用户、单 Agent 场景。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use mofa_foundation::persistence::{InMemoryStore, PersistenceHandler};
    ///
    /// let store = InMemoryStore::shared();
    /// let handler = PersistenceHandler::auto(store);
    /// ```
    pub fn auto(store: Arc<S>) -> Self {
        Self::new(store, Uuid::now_v7(), Uuid::now_v7(),Uuid::now_v7(),Uuid::now_v7())
    }

    /// 从环境变量创建持久化处理器
    ///
    /// 自动从环境变量读取 user_id 和 agent_id，如果未设置则自动生成。
    ///
    /// 环境变量：
    /// - USER_ID: 用户 ID（可选，默认自动生成）
    /// - AGENT_ID: Agent ID（可选，默认自动生成）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use mofa_foundation::persistence::{InMemoryStore, PersistenceHandler};
    ///
    /// let store = InMemoryStore::shared();
    /// let handler = PersistenceHandler::from_env(store);
    /// ```
    pub fn from_env(store: Arc<S>) -> Self {
        let user_id = std::env::var("USER_ID")
            .ok()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(Uuid::now_v7);

        let tenant_id = std::env::var("TENANT_ID")
            .ok()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(Uuid::now_v7);

        let agent_id = std::env::var("AGENT_ID")
            .ok()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(Uuid::now_v7);

        Self::new(store, user_id, tenant_id, agent_id, Uuid::now_v7())
    }

    /// 设置会话 ID
    pub async fn set_session_id(&self, session_id: Uuid) {
        *self.session_id.write().await = session_id;
    }

    /// 获取会话 ID
    pub async fn get_session_id(&self) -> Uuid {
        *self.session_id.read().await
    }

    /// 创建新会话
    pub async fn new_session(&self) -> Uuid {
         Uuid::now_v7()
    }

    /// 获取存储后端引用
    pub fn store(&self) -> Arc<S> {
        self.store.clone()
    }

    /// 获取用户 ID
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// 获取 Agent ID
    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    pub fn tenant_id(&self) -> Uuid {self.tenant_id}

    /// 保存消息
    async fn save_message_internal(&self, role: MessageRole, content: &str) -> LLMResult<Uuid> {
        let session_id = *self.session_id.read().await;
        debug!("session_id: {}", session_id);
        let message = LLMMessage::new(
            session_id,
            self.agent_id,
            self.user_id,
            self.tenant_id,
            role,
            MessageContent::text(content),
        );
        let id = message.id;

        self.store
            .save_message(&message)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(id)
    }
}

#[async_trait::async_trait]
impl<S> PersistenceCallback for PersistenceHandler<S>
where
    S: MessageStore + ApiCallStore + Send + Sync + 'static,
{
    async fn on_user_message(&self, content: &str) -> LLMResult<Uuid> {
        self.save_message_internal(MessageRole::User, content).await
    }

    async fn on_assistant_message(&self, content: &str) -> LLMResult<Uuid> {
        self.save_message_internal(MessageRole::Assistant, content)
            .await
    }

    async fn on_api_call(
        &self,
        model: &str,
        prompt_tokens: i32,
        completion_tokens: i32,
        request_message_id: Uuid,
        response_message_id: Uuid,
        latency_ms: i32,
        response_id: Option<&str>,
    ) -> LLMResult<Uuid> {
        let session_id = *self.session_id.read().await;
        let now = chrono::Utc::now();
        let request_time = now - chrono::Duration::milliseconds(latency_ms as i64);

        let mut api_call = LLMApiCall::success(
            session_id,
            self.agent_id,
            self.user_id,
            self.tenant_id,
            request_message_id,
            response_message_id,
            model,
            prompt_tokens,
            completion_tokens,
            request_time,
            now,
        );

        // 设置 response_id（如果提供）
        if let Some(rid) = response_id {
            api_call = api_call.with_api_response_id(rid);
        }

        let id = api_call.id;

        self.store
            .save_api_call(&api_call)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(id)
    }

    async fn on_api_error(
        &self,
        model: &str,
        request_message_id: Uuid,
        error_message: &str,
    ) -> LLMResult<Uuid> {
        let session_id = *self.session_id.read().await;
        let now = chrono::Utc::now();

        let api_call = LLMApiCall::failed(
            session_id,
            self.agent_id,
            self.user_id,
            self.tenant_id,
            request_message_id,
            model,
            error_message,
            None,
            now,
        );
        let id = api_call.id;

        self.store
            .save_api_call(&api_call)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(id)
    }

    async fn set_session_id(&self, session_id: Uuid) {
        *self.session_id.write().await = session_id;
        info!("✅ PersistenceHandler session_id 已同步: {}", session_id);
    }
}

/// 持久化上下文
///
/// 提供对持久化功能的便捷访问
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
    pub async fn new(store: Arc<S>, user_id: Uuid, tenant_id: Uuid, agent_id: Uuid) -> LLMResult<Self> {
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
    pub fn from_session(store: Arc<S>, user_id: Uuid, agent_id: Uuid, tenant_id:Uuid, session_id: Uuid) -> Self {
        Self {
            store,
            user_id,
            agent_id,
            tenant_id,
            session_id,
        }
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// 保存用户消息
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
    pub async fn get_history(&self) -> LLMResult<Vec<LLMMessage>> {
        self.store
            .get_session_messages(self.session_id)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))
    }

    /// 获取使用统计
    pub async fn get_usage_stats(&self) -> LLMResult<UsageStatistics> {
        let filter = QueryFilter::new().session(self.session_id);
        self.store
            .get_statistics(&filter)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))
    }

    /// 创建新会话
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
    pub fn store(&self) -> Arc<S> {
        self.store.clone()
    }

    /// 创建持久化处理器
    pub fn create_handler(&self) -> PersistenceHandler<S>
    where
        S: Clone,
    {
        PersistenceHandler::new(self.store.clone(), self.user_id, self.tenant_id, self.agent_id, self.session_id)
    }
}

/// 创建持久化处理器的便捷函数
pub fn create_persistence_handler<S>(
    store: Arc<S>,
    user_id: Uuid,
    tenant_id: Uuid,
    agent_id: Uuid,
    session_id: Uuid,
) -> PersistenceHandler<S>
where
    S: MessageStore + ApiCallStore + Send + Sync + 'static,
{
    PersistenceHandler::new(store, user_id, tenant_id, agent_id,session_id)
}

// ============================================================================
// PersistencePlugin - 实现 AgentPlugin trait
// ============================================================================

/// 持久化插件
///
/// 实现 AgentPlugin trait，提供完整的持久化能力：
/// - 从数据库加载会话历史
/// - 自动记录用户消息、助手消息、API 调用
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::persistence::{PersistencePlugin, PostgresStore};
/// use mofa_sdk::llm::LLMAgentBuilder;
/// use std::sync::Arc;
/// use uuid::Uuid;
///
/// # async fn example() -> anyhow::Result<()> {
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
    ///
    /// # 参数
    /// - `plugin_id`: 插件唯一标识
    /// - `message_store`: 消息存储后端
    /// - `api_call_store`: API 调用存储后端
    /// - `user_id`: 用户 ID
    /// - `tenant_id`: 租户 ID
    /// - `agent_id`: Agent ID
    /// - `session_id`: 会话 ID
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
    ///
    /// # 参数
    /// - `plugin_id`: 插件唯一标识
    /// - `store`: 持久化存储后端（需要同时实现 MessageStore、ApiCallStore、SessionStore）
    /// - `user_id`: 用户 ID
    /// - `tenant_id`: 租户 ID
    /// - `agent_id`: Agent ID
    /// - `session_id`: 会话 ID
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
        Self::new(
            plugin_id,
            store_arc.clone(),
            store_arc,
            user_id,
            tenant_id,
            agent_id,
            session_id,
        )
    }

    /// 更新会话 ID
    pub async fn with_session_id(&self, session_id: Uuid) {
        *self.session_id.write().await = session_id;
    }

    /// 获取当前会话 ID
    pub async fn session_id(&self) -> Uuid {
        *self.session_id.read().await
    }

    /// 获取历史消息（用于 build_async）
    pub async fn load_history(&self) -> PersistenceResult<Vec<LLMMessage>> {
        self.message_store.get_session_messages(*self.session_id.read().await).await
    }

    /// 获取消息存储引用
    pub fn message_store(&self) -> Arc<dyn MessageStore + Send + Sync> {
        self.message_store.clone()
    }

    /// 获取 API 调用存储引用
    pub fn api_call_store(&self) -> Arc<dyn ApiCallStore + Send + Sync> {
        self.api_call_store.clone()
    }

    /// 获取用户 ID
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// 获取租户 ID
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    /// 获取 Agent ID
    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    /// 保存消息（内部方法）
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
    pub async fn save_user_message(&self, content: &str) -> LLMResult<Uuid> {
        self.save_message_internal(MessageRole::User, content).await
    }

    /// 保存助手消息
    pub async fn save_assistant_message(&self, content: &str) -> LLMResult<Uuid> {
        self.save_message_internal(MessageRole::Assistant, content).await
    }
}

impl Clone for PersistencePlugin {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            state: self.state.clone(),
            message_store: self.message_store.clone(),
            api_call_store: self.api_call_store.clone(),
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
impl AgentPlugin for PersistencePlugin
{
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
        self.state = PluginState::Unloaded2;
        Ok(())
    }

    async fn unload(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded2;
        Ok(())
    }

    async fn execute(&mut self, _input: String) -> PluginResult<String> {
        Ok("persistence plugin".to_string())
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        stats.insert("plugin_type".to_string(), serde_json::Value::String("persistence".to_string()));
        stats.insert("user_id".to_string(), serde_json::Value::String(self.user_id.to_string()));
        stats.insert("tenant_id".to_string(), serde_json::Value::String(self.tenant_id.to_string()));
        stats.insert("agent_id".to_string(), serde_json::Value::String(self.agent_id.to_string()));
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

// 实现 LLMAgentEventHandler trait（复用现有 AgentPersistenceHandler 逻辑）
#[async_trait::async_trait]
impl crate::llm::agent::LLMAgentEventHandler for PersistencePlugin
{
    fn clone_box(&self) -> Box<dyn crate::llm::agent::LLMAgentEventHandler> {
        // 由于 PersistencePlugin 需要 Arc<S>，我们创建一个新的克隆实例
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// 在发送用户消息前调用 - 记录用户消息
    async fn before_chat(&self, message: &str) -> LLMResult<Option<String>> {
        // 记录请求开始时间
        *self.request_start_time.write().await = Some(std::time::Instant::now());

        // 保存用户消息
        let user_msg_id = self.save_user_message(message).await?;
        info!("✅ [持久化插件] 用户消息已保存: ID = {}", user_msg_id);

        // 存储当前用户消息 ID，用于后续关联 API 调用
        *self.current_user_msg_id.write().await = Some(user_msg_id);

        Ok(Some(message.to_string()))
    }

    /// 在发送用户消息前调用（带模型名称）- 记录用户消息和模型
    async fn before_chat_with_model(
        &self,
        message: &str,
        model: &str,
    ) -> LLMResult<Option<String>> {
        // 存储模型名称，用于后续的 after_chat 和 on_error
        *self.current_model.write().await = Some(model.to_string());

        // 调用原有的 before_chat 逻辑
        self.before_chat(message).await
    }

    /// 在收到 LLM 响应后调用 - 记录助手消息和 API 调用
    async fn after_chat(&self, response: &str) -> LLMResult<Option<String>> {
        // 保存助手消息
        let assistant_msg_id = self.save_assistant_message(response).await?;
        info!("✅ [持久化插件] 助手消息已保存: ID = {}", assistant_msg_id);

        // 计算请求延迟
        let latency = match *self.request_start_time.read().await {
            Some(start) => start.elapsed().as_millis() as i32,
            None => 0,
        };

        // 获取存储的模型名称，或使用默认值
        let model = self.current_model.read().await;
        let model_name = model.as_ref().map(|s| s.as_str()).unwrap_or("unknown");

        // 记录 API 调用
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
                0,  // 未知（没有元数据时无法获取真实值）
                response.len() as i32 / 4,  // 简单估算 completion_tokens (每4字符一个token)
                request_time,
                now,
            );

            let _ = self.api_call_store
                .save_api_call(&api_call)
                .await
                .map_err(|e| LLMError::Other(e.to_string()));
            info!("✅ [持久化插件] API 调用记录已保存: 模型={}, 延迟={}ms", model_name, latency);
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.current_model.write().await = None;

        Ok(Some(response.to_string()))
    }

    /// 在收到 LLM 响应后调用 - 记录助手消息和 API 调用（带元数据）
    async fn after_chat_with_metadata(
        &self,
        response: &str,
        metadata: &LLMResponseMetadata,
    ) -> LLMResult<Option<String>> {
        // 保存 response_id
        *self.response_id.write().await = Some(metadata.id.clone());

        // 保存助手消息
        let assistant_msg_id = self.save_assistant_message(response).await?;
        info!("✅ [持久化插件] 助手消息已保存: ID = {}", assistant_msg_id);

        // 计算请求延迟
        let latency = match *self.request_start_time.read().await {
            Some(start) => start.elapsed().as_millis() as i32,
            None => 0,
        };

        // 记录 API 调用
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
            api_call = api_call.with_api_response_id(&metadata.id);

            let _ = self.api_call_store
                .save_api_call(&api_call)
                .await
                .map_err(|e| LLMError::Other(e.to_string()));
            info!(
                "✅ [持久化插件] API 调用记录已保存: 模型={}, tokens={}/{}, 延迟={}ms",
                metadata.model, metadata.prompt_tokens, metadata.completion_tokens, latency
            );
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.response_id.write().await = None;

        Ok(Some(response.to_string()))
    }

    /// 在发生错误时调用 - 记录 API 错误
    async fn on_error(&self, error: &LLMError) -> LLMResult<Option<String>> {
        info!("✅ [持久化插件] 记录 API 错误...");

        // 获取存储的模型名称，或使用默认值
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

            let _ = self.api_call_store
                .save_api_call(&api_call)
                .await
                .map_err(|e| LLMError::Other(e.to_string()));
            info!("✅ [持久化插件] API 错误记录已保存");
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;
        *self.current_model.write().await = None;

        Ok(None)
    }
}
