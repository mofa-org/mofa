//! 持久化插件
//!
//! 提供与 LLMAgent 集成的持久化功能

use super::entities::*;
use super::traits::*;
use crate::llm::{LLMError, LLMResult};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
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
    current_user_msg_id: Arc<tokio::sync::RwLock<Option<Uuid>>>,
    request_start_time: Arc<tokio::sync::RwLock<Option<std::time::Instant>>>,
}

impl AgentPersistenceHandler {
    /// 创建 Agent 持久化事件处理器
    pub fn new(persistence: Arc<dyn PersistenceCallback>) -> Self {
        Self {
            persistence,
            current_user_msg_id: Arc::new(tokio::sync::RwLock::new(None)),
            request_start_time: Arc::new(tokio::sync::RwLock::new(None)),
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

        // 记录 API 调用
        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let _ = self.persistence.on_api_call(
                "gpt-3.5-turbo",  // TODO: 从 LLM 配置或响应中获取真实模型名称
                100,              // TODO: 从实际请求中获取真实 prompt_tokens
                response.len() as i32 / 4,  // 简单估算 completion_tokens (每4字符一个token)
                user_msg_id,
                assistant_msg_id,
                latency,
            ).await?;
            info!("✅ [内置持久化] API 调用记录已保存: 延迟 = {}ms", latency);
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;

        Ok(Some(response.to_string()))
    }

    /// 在发生错误时调用 - 记录 API 错误
    async fn on_error(&self, error: &crate::llm::LLMError) -> crate::llm::LLMResult<Option<String>> {
        info!("✅ [内置持久化] 记录 API 错误...");

        if let Some(user_msg_id) = *self.current_user_msg_id.read().await {
            let _ = self.persistence.on_api_error(
                "gpt-3.5-turbo",  // TODO: 从 LLM 配置或响应中获取真实模型名称
                user_msg_id,
                &error.to_string(),
            ).await?;
            info!("✅ [内置持久化] API 错误记录已保存");
        }

        // 清理状态
        *self.current_user_msg_id.write().await = None;
        *self.request_start_time.write().await = None;

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
    /// Agent ID
    agent_id: Uuid,
    /// 当前会话 ID
    session_id: Arc<RwLock<Uuid>>,
    /// 请求计数器
    request_count: AtomicU64,
}

impl<S> PersistenceHandler<S>
where
    S: MessageStore + ApiCallStore + Send + Sync + 'static,
{
    /// 创建持久化处理器
    pub fn new(store: Arc<S>, user_id: Uuid, agent_id: Uuid) -> Self {
        Self {
            store,
            user_id,
            agent_id,
            session_id: Arc::new(RwLock::new(Uuid::now_v7())),
            request_count: AtomicU64::new(0),
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
        Self::new(store, Uuid::now_v7(), Uuid::now_v7())
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

        let agent_id = std::env::var("AGENT_ID")
            .ok()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(Uuid::now_v7);

        Self::new(store, user_id, agent_id)
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
        let session_id = Uuid::now_v7();
        *self.session_id.write().await = session_id;
        session_id
    }

    /// 获取请求计数
    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::SeqCst)
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

    /// 保存消息
    async fn save_message_internal(&self, role: MessageRole, content: &str) -> LLMResult<Uuid> {
        let session_id = *self.session_id.read().await;
        let message = LLMMessage::new(
            session_id,
            self.agent_id,
            self.user_id,
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
        self.request_count.fetch_add(1, Ordering::SeqCst);
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
    ) -> LLMResult<Uuid> {
        let session_id = *self.session_id.read().await;
        let now = chrono::Utc::now();
        let request_time = now - chrono::Duration::milliseconds(latency_ms as i64);

        let api_call = LLMApiCall::success(
            session_id,
            self.agent_id,
            self.user_id,
            request_message_id,
            response_message_id,
            model,
            prompt_tokens,
            completion_tokens,
            request_time,
            now,
        );
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
    session_id: Uuid,
}

impl<S> PersistenceContext<S>
where
    S: MessageStore + ApiCallStore + SessionStore + Send + Sync + 'static,
{
    /// 创建新的持久化上下文
    pub async fn new(store: Arc<S>, user_id: Uuid, agent_id: Uuid) -> LLMResult<Self> {
        let session = ChatSession::new(user_id, agent_id);
        store
            .create_session(&session)
            .await
            .map_err(|e| LLMError::Other(e.to_string()))?;

        Ok(Self {
            store,
            user_id,
            agent_id,
            session_id: session.id,
        })
    }

    /// 从现有会话创建上下文
    pub fn from_session(store: Arc<S>, user_id: Uuid, agent_id: Uuid, session_id: Uuid) -> Self {
        Self {
            store,
            user_id,
            agent_id,
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
        PersistenceHandler::new(self.store.clone(), self.user_id, self.agent_id)
    }
}

/// 创建持久化处理器的便捷函数
pub fn create_persistence_handler<S>(
    store: Arc<S>,
    user_id: Uuid,
    agent_id: Uuid,
) -> PersistenceHandler<S>
where
    S: MessageStore + ApiCallStore + Send + Sync + 'static,
{
    PersistenceHandler::new(store, user_id, agent_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::InMemoryStore;

    #[tokio::test]
    async fn test_persistence_handler() {
        let store = InMemoryStore::shared();
        let user_id = Uuid::now_v7();
        let agent_id = Uuid::now_v7();

        let handler = PersistenceHandler::new(store.clone(), user_id, agent_id);

        // 记录消息
        let msg_id = handler.on_user_message("Hello").await.unwrap();
        assert!(!msg_id.is_nil());

        let reply_id = handler.on_assistant_message("Hi there!").await.unwrap();
        assert!(!reply_id.is_nil());

        // 记录 API 调用
        let call_id = handler
            .on_api_call("gpt-4", 100, 50, msg_id, reply_id, 1000)
            .await
            .unwrap();
        assert!(!call_id.is_nil());

        assert_eq!(handler.request_count(), 1);
    }

    #[tokio::test]
    async fn test_persistence_context() {
        let store = Arc::new(InMemoryStore::new());
        let user_id = Uuid::now_v7();
        let agent_id = Uuid::now_v7();

        let ctx = PersistenceContext::new(store.clone(), user_id, agent_id)
            .await
            .unwrap();

        // 保存消息
        ctx.save_user_message("Hello").await.unwrap();
        ctx.save_assistant_message("Hi there!").await.unwrap();

        // 获取历史
        let history = ctx.get_history().await.unwrap();
        assert_eq!(history.len(), 2);
    }
}
