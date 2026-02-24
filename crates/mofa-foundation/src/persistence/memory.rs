//! 内存存储后端
//! Memory storage backend
//!
//! 提供基于内存的存储实现，适用于测试和开发环境
//! Provides an in-memory storage implementation, suitable for testing and development environments

use super::entities::*;
use super::traits::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

/// 内存存储
/// In-memory storage
///
/// 线程安全的内存存储实现，所有数据存储在内存中。
/// Thread-safe in-memory storage implementation, all data is stored in memory.
/// 适用于：
/// Suitable for:
/// - 单元测试
/// - Unit testing
/// - 开发环境
/// - Development environment
/// - 短期会话（无需持久化）
/// - Short-term sessions (no persistence required)
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::persistence::InMemoryStore;
///
/// let store = InMemoryStore::new();
///
/// // 保存消息
/// // Save message
/// store.save_message(&message).await?;
///
/// // 查询消息
/// // Query messages
/// let messages = store.get_session_messages(session_id).await?;
/// ```
pub struct InMemoryStore {
    /// 消息存储
    /// Message storage
    messages: Arc<RwLock<HashMap<Uuid, LLMMessage>>>,
    /// 会话消息索引 (session_id -> message_ids)
    /// Session message index (session_id -> message_ids)
    session_messages: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    /// API 调用记录
    /// API call records
    api_calls: Arc<RwLock<HashMap<Uuid, LLMApiCall>>>,
    /// 会话存储
    /// Session storage
    sessions: Arc<RwLock<HashMap<Uuid, ChatSession>>>,
    /// 用户会话索引 (user_id -> session_ids)
    /// User session index (user_id -> session_ids)
    user_sessions: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    /// 连接状态
    /// Connection status
    connected: AtomicBool,
}

impl InMemoryStore {
    /// 创建新的内存存储
    /// Create new in-memory store
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(HashMap::new())),
            session_messages: Arc::new(RwLock::new(HashMap::new())),
            api_calls: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            user_sessions: Arc::new(RwLock::new(HashMap::new())),
            connected: AtomicBool::new(true),
        }
    }

    /// 创建共享的内存存储
    /// Create shared in-memory store
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// 清空所有数据
    /// Clear all data
    pub async fn clear(&self) {
        self.messages.write().await.clear();
        self.session_messages.write().await.clear();
        self.api_calls.write().await.clear();
        self.sessions.write().await.clear();
        self.user_sessions.write().await.clear();
    }

    /// 获取消息总数
    /// Get total message count
    pub async fn message_count(&self) -> usize {
        self.messages.read().await.len()
    }

    /// 获取 API 调用总数
    /// Get total API call count
    pub async fn api_call_count(&self) -> usize {
        self.api_calls.read().await.len()
    }

    /// 获取会话总数
    /// Get total session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageStore for InMemoryStore {
    async fn save_message(&self, message: &LLMMessage) -> PersistenceResult<()> {
        let mut messages = self.messages.write().await;
        let mut session_messages = self.session_messages.write().await;

        messages.insert(message.id, message.clone());

        session_messages
            .entry(message.chat_session_id)
            .or_insert_with(Vec::new)
            .push(message.id);

        Ok(())
    }

    async fn get_message(&self, id: Uuid) -> PersistenceResult<Option<LLMMessage>> {
        let messages = self.messages.read().await;
        Ok(messages.get(&id).cloned())
    }

    async fn get_session_messages(&self, session_id: Uuid) -> PersistenceResult<Vec<LLMMessage>> {
        let messages = self.messages.read().await;
        let session_messages = self.session_messages.read().await;

        let msg_ids = session_messages.get(&session_id);

        let mut result = Vec::new();
        if let Some(ids) = msg_ids {
            for id in ids {
                if let Some(msg) = messages.get(id) {
                    result.push(msg.clone());
                }
            }
        }

        // 按创建时间排序
        // Sort by creation time
        result.sort_by(|a, b| a.create_time.cmp(&b.create_time));

        Ok(result)
    }

    async fn get_session_messages_paginated(
        &self,
        session_id: Uuid,
        offset: i64,
        limit: i64,
    ) -> PersistenceResult<Vec<LLMMessage>> {
        let all_messages = self.get_session_messages(session_id).await?;

        let start = offset as usize;
        let end = (offset + limit) as usize;

        Ok(all_messages
            .into_iter()
            .skip(start)
            .take(end - start)
            .collect())
    }

    async fn delete_message(&self, id: Uuid) -> PersistenceResult<bool> {
        let mut messages = self.messages.write().await;

        if let Some(msg) = messages.remove(&id) {
            let mut session_messages = self.session_messages.write().await;
            if let Some(ids) = session_messages.get_mut(&msg.chat_session_id) {
                ids.retain(|&x| x != id);
            }
            return Ok(true);
        }

        Ok(false)
    }

    async fn delete_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        let mut messages = self.messages.write().await;
        let mut session_messages = self.session_messages.write().await;

        let count = if let Some(ids) = session_messages.remove(&session_id) {
            let len = ids.len();
            for id in ids {
                messages.remove(&id);
            }
            len as i64
        } else {
            0
        };

        Ok(count)
    }

    async fn count_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        let session_messages = self.session_messages.read().await;
        Ok(session_messages
            .get(&session_id)
            .map(|ids| ids.len() as i64)
            .unwrap_or(0))
    }
}

#[async_trait]
impl ApiCallStore for InMemoryStore {
    async fn save_api_call(&self, call: &LLMApiCall) -> PersistenceResult<()> {
        let mut api_calls = self.api_calls.write().await;
        api_calls.insert(call.id, call.clone());
        Ok(())
    }

    async fn get_api_call(&self, id: Uuid) -> PersistenceResult<Option<LLMApiCall>> {
        let api_calls = self.api_calls.read().await;
        Ok(api_calls.get(&id).cloned())
    }

    async fn query_api_calls(&self, filter: &QueryFilter) -> PersistenceResult<Vec<LLMApiCall>> {
        let api_calls = self.api_calls.read().await;

        let mut result: Vec<LLMApiCall> = api_calls
            .values()
            .filter(|call| {
                // 用户过滤
                // User filtering
                if let Some(user_id) = filter.user_id
                    && call.user_id != user_id
                {
                    return false;
                }

                // 会话过滤
                // Session filtering
                if let Some(session_id) = filter.session_id
                    && call.chat_session_id != session_id
                {
                    return false;
                }

                // Agent 过滤
                // Agent filtering
                if let Some(agent_id) = filter.agent_id
                    && call.agent_id != agent_id
                {
                    return false;
                }

                // 时间范围过滤
                // Time range filtering
                if let Some(start) = filter.start_time
                    && call.create_time < start
                {
                    return false;
                }
                if let Some(end) = filter.end_time
                    && call.create_time > end
                {
                    return false;
                }

                // 状态过滤
                // Status filtering
                if let Some(status) = filter.status
                    && call.status != status
                {
                    return false;
                }

                // 模型过滤
                // Model filtering
                if let Some(ref model) = filter.model_name
                    && &call.model_name != model
                {
                    return false;
                }

                true
            })
            .cloned()
            .collect();

        // 按创建时间降序排序
        // Sort by creation time descending
        result.sort_by(|a, b| b.create_time.cmp(&a.create_time));

        // 分页
        // Pagination
        let offset = filter.offset.unwrap_or(0) as usize;
        let limit = filter.limit.unwrap_or(100) as usize;

        Ok(result.into_iter().skip(offset).take(limit).collect())
    }

    async fn get_statistics(&self, filter: &QueryFilter) -> PersistenceResult<UsageStatistics> {
        let calls = self.query_api_calls(filter).await?;

        let total_calls = calls.len() as i64;
        let success_count = calls
            .iter()
            .filter(|c| c.status == ApiCallStatus::Success)
            .count() as i64;
        let failed_count = total_calls - success_count;

        let total_prompt_tokens: i64 = calls.iter().map(|c| c.prompt_tokens as i64).sum();
        let total_completion_tokens: i64 = calls.iter().map(|c| c.completion_tokens as i64).sum();
        let total_tokens = total_prompt_tokens + total_completion_tokens;

        let total_cost: Option<f64> = {
            let costs: Vec<f64> = calls.iter().filter_map(|c| c.total_price).collect();
            if costs.is_empty() {
                None
            } else {
                Some(costs.iter().sum())
            }
        };

        let avg_latency_ms: Option<f64> = {
            let latencies: Vec<i32> = calls.iter().filter_map(|c| c.latency_ms).collect();
            if latencies.is_empty() {
                None
            } else {
                Some(latencies.iter().sum::<i32>() as f64 / latencies.len() as f64)
            }
        };

        let avg_tokens_per_second: Option<f64> = {
            let tps: Vec<f64> = calls.iter().filter_map(|c| c.tokens_per_second).collect();
            if tps.is_empty() {
                None
            } else {
                Some(tps.iter().sum::<f64>() / tps.len() as f64)
            }
        };

        Ok(UsageStatistics {
            total_calls,
            success_count,
            failed_count,
            total_tokens,
            total_prompt_tokens,
            total_completion_tokens,
            total_cost,
            avg_latency_ms,
            avg_tokens_per_second,
        })
    }

    async fn delete_api_call(&self, id: Uuid) -> PersistenceResult<bool> {
        let mut api_calls = self.api_calls.write().await;
        Ok(api_calls.remove(&id).is_some())
    }

    async fn cleanup_old_records(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> PersistenceResult<i64> {
        let mut api_calls = self.api_calls.write().await;
        let old_len = api_calls.len();

        api_calls.retain(|_, call| call.create_time >= before);

        Ok((old_len - api_calls.len()) as i64)
    }
}

#[async_trait]
impl SessionStore for InMemoryStore {
    async fn create_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        let mut sessions = self.sessions.write().await;
        let mut user_sessions = self.user_sessions.write().await;

        sessions.insert(session.id, session.clone());

        user_sessions
            .entry(session.user_id)
            .or_insert_with(Vec::new)
            .push(session.id);

        Ok(())
    }

    async fn get_session(&self, id: Uuid) -> PersistenceResult<Option<ChatSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(&id).cloned())
    }

    async fn get_user_sessions(&self, user_id: Uuid) -> PersistenceResult<Vec<ChatSession>> {
        let sessions = self.sessions.read().await;
        let user_sessions = self.user_sessions.read().await;

        let session_ids = user_sessions.get(&user_id);

        let mut result = Vec::new();
        if let Some(ids) = session_ids {
            for id in ids {
                if let Some(session) = sessions.get(id) {
                    result.push(session.clone());
                }
            }
        }

        // 按更新时间降序排序
        // Sort by update time descending
        result.sort_by(|a, b| b.update_time.cmp(&a.update_time));

        Ok(result)
    }

    async fn update_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        let mut sessions = self.sessions.write().await;

        if let std::collections::hash_map::Entry::Occupied(mut e) = sessions.entry(session.id) {
            e.insert(session.clone());
            Ok(())
        } else {
            Err(PersistenceError::NotFound(format!(
                "Session {} not found",
                session.id
            )))
        }
    }

    async fn delete_session(&self, id: Uuid) -> PersistenceResult<bool> {
        let mut sessions = self.sessions.write().await;
        let mut user_sessions = self.user_sessions.write().await;

        if let Some(session) = sessions.remove(&id) {
            if let Some(ids) = user_sessions.get_mut(&session.user_id) {
                ids.retain(|&x| x != id);
            }
            return Ok(true);
        }

        Ok(false)
    }
}

#[async_trait]
impl ProviderStore for InMemoryStore {
    async fn get_provider(
        &self,
        _id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::Provider>> {
        // Memory store doesn't support providers - return not found
        // 内存存储不支持提供商 - 返回未找到
        Ok(None)
    }

    async fn get_provider_by_name(
        &self,
        _tenant_id: Uuid,
        _name: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Provider>> {
        // Memory store doesn't support providers - return not found
        // 内存存储不支持提供商 - 返回未找到
        Ok(None)
    }

    async fn list_providers(
        &self,
        _tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Provider>> {
        // Memory store doesn't support providers - return empty list
        // 内存存储不支持提供商 - 返回空列表
        Ok(Vec::new())
    }

    async fn get_enabled_providers(
        &self,
        _tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Provider>> {
        // Memory store doesn't support providers - return empty list
        // 内存存储不支持提供商 - 返回空列表
        Ok(Vec::new())
    }
}

#[async_trait]
impl AgentStore for InMemoryStore {
    async fn get_agent(
        &self,
        _id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        // Memory store doesn't support agents - return not found
        // 内存存储不支持智能体 - 返回未找到
        Ok(None)
    }

    async fn get_agent_by_code(
        &self,
        _code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        // Memory store doesn't support agents - return not found
        // 内存存储不支持智能体 - 返回未找到
        Ok(None)
    }

    async fn get_agent_by_code_and_tenant(
        &self,
        _tenant_id: Uuid,
        _code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        // Memory store doesn't support agents - return not found
        // 内存存储不支持智能体 - 返回未找到
        Ok(None)
    }

    async fn list_agents(
        &self,
        _tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Agent>> {
        // Memory store doesn't support agents - return empty list
        // 内存存储不支持智能体 - 返回空列表
        Ok(Vec::new())
    }

    async fn get_active_agents(
        &self,
        _tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Agent>> {
        // Memory store doesn't support agents - return empty list
        // 内存存储不支持智能体 - 返回空列表
        Ok(Vec::new())
    }

    async fn get_agent_with_provider(
        &self,
        _id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        // Memory store doesn't support agents - return not found
        // 内存存储不支持智能体 - 返回未找到
        Ok(None)
    }

    async fn get_agent_by_code_with_provider(
        &self,
        _code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        // Memory store doesn't support agents - return not found
        // 内存存储不支持智能体 - 返回未找到
        Ok(None)
    }

    async fn get_agent_by_code_and_tenant_with_provider(
        &self,
        _tenant_id: Uuid,
        _code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        // Memory store doesn't support agents - return not found
        // 内存存储不支持智能体 - 返回未找到
        Ok(None)
    }
}

impl PersistenceStore for InMemoryStore {
    fn backend_name(&self) -> &str {
        "memory"
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn close(&self) -> PersistenceResult<()> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}

/// 带容量限制的内存存储
/// Bounded in-memory store
///
/// 当达到容量限制时，自动清理最旧的记录
/// Automatically cleans up the oldest records when capacity limits are reached
pub struct BoundedInMemoryStore {
    /// 内部存储
    /// Inner storage
    inner: InMemoryStore,
    /// 消息容量限制
    /// Message capacity limit
    max_messages: usize,
    /// API 调用容量限制
    /// API call capacity limit
    max_api_calls: usize,
}

impl BoundedInMemoryStore {
    /// 创建带容量限制的内存存储
    /// Create bounded in-memory store
    pub fn new(max_messages: usize, max_api_calls: usize) -> Self {
        Self {
            inner: InMemoryStore::new(),
            max_messages,
            max_api_calls,
        }
    }

    /// 创建共享实例
    /// Create shared instance
    pub fn shared(max_messages: usize, max_api_calls: usize) -> Arc<Self> {
        Arc::new(Self::new(max_messages, max_api_calls))
    }

    /// 清理超出容量的消息
    /// Cleanup messages exceeding capacity
    async fn cleanup_messages_if_needed(&self) {
        let mut messages = self.inner.messages.write().await;

        if messages.len() > self.max_messages {
            // 收集要删除的 ID
            // Collect IDs to remove
            let mut sorted: Vec<_> = messages
                .iter()
                .map(|(id, msg)| (*id, msg.create_time))
                .collect();
            sorted.sort_by(|a, b| a.1.cmp(&b.1));

            let to_remove: Vec<Uuid> = sorted
                .into_iter()
                .take(messages.len() - self.max_messages)
                .map(|(id, _)| id)
                .collect();

            for id in to_remove {
                messages.remove(&id);
            }
        }
    }

    /// 清理超出容量的 API 调用记录
    /// Cleanup API calls exceeding capacity
    async fn cleanup_api_calls_if_needed(&self) {
        let mut api_calls = self.inner.api_calls.write().await;

        if api_calls.len() > self.max_api_calls {
            // 收集要删除的 ID
            // Collect IDs to remove
            let mut sorted: Vec<_> = api_calls
                .iter()
                .map(|(id, call)| (*id, call.create_time))
                .collect();
            sorted.sort_by(|a, b| a.1.cmp(&b.1));

            let to_remove: Vec<Uuid> = sorted
                .into_iter()
                .take(api_calls.len() - self.max_api_calls)
                .map(|(id, _)| id)
                .collect();

            for id in to_remove {
                api_calls.remove(&id);
            }
        }
    }
}

#[async_trait]
impl MessageStore for BoundedInMemoryStore {
    async fn save_message(&self, message: &LLMMessage) -> PersistenceResult<()> {
        self.inner.save_message(message).await?;
        self.cleanup_messages_if_needed().await;
        Ok(())
    }

    async fn get_message(&self, id: Uuid) -> PersistenceResult<Option<LLMMessage>> {
        self.inner.get_message(id).await
    }

    async fn get_session_messages(&self, session_id: Uuid) -> PersistenceResult<Vec<LLMMessage>> {
        self.inner.get_session_messages(session_id).await
    }

    async fn get_session_messages_paginated(
        &self,
        session_id: Uuid,
        offset: i64,
        limit: i64,
    ) -> PersistenceResult<Vec<LLMMessage>> {
        self.inner
            .get_session_messages_paginated(session_id, offset, limit)
            .await
    }

    async fn delete_message(&self, id: Uuid) -> PersistenceResult<bool> {
        self.inner.delete_message(id).await
    }

    async fn delete_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        self.inner.delete_session_messages(session_id).await
    }

    async fn count_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        self.inner.count_session_messages(session_id).await
    }
}

#[async_trait]
impl ApiCallStore for BoundedInMemoryStore {
    async fn save_api_call(&self, call: &LLMApiCall) -> PersistenceResult<()> {
        self.inner.save_api_call(call).await?;
        self.cleanup_api_calls_if_needed().await;
        Ok(())
    }

    async fn get_api_call(&self, id: Uuid) -> PersistenceResult<Option<LLMApiCall>> {
        self.inner.get_api_call(id).await
    }

    async fn query_api_calls(&self, filter: &QueryFilter) -> PersistenceResult<Vec<LLMApiCall>> {
        self.inner.query_api_calls(filter).await
    }

    async fn get_statistics(&self, filter: &QueryFilter) -> PersistenceResult<UsageStatistics> {
        self.inner.get_statistics(filter).await
    }

    async fn delete_api_call(&self, id: Uuid) -> PersistenceResult<bool> {
        self.inner.delete_api_call(id).await
    }

    async fn cleanup_old_records(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> PersistenceResult<i64> {
        self.inner.cleanup_old_records(before).await
    }
}

#[async_trait]
impl SessionStore for BoundedInMemoryStore {
    async fn create_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        self.inner.create_session(session).await
    }

    async fn get_session(&self, id: Uuid) -> PersistenceResult<Option<ChatSession>> {
        self.inner.get_session(id).await
    }

    async fn get_user_sessions(&self, user_id: Uuid) -> PersistenceResult<Vec<ChatSession>> {
        self.inner.get_user_sessions(user_id).await
    }

    async fn update_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        self.inner.update_session(session).await
    }

    async fn delete_session(&self, id: Uuid) -> PersistenceResult<bool> {
        self.inner.delete_session(id).await
    }
}

#[async_trait]
impl ProviderStore for BoundedInMemoryStore {
    async fn get_provider(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::Provider>> {
        self.inner.get_provider(id).await
    }

    async fn get_provider_by_name(
        &self,
        tenant_id: Uuid,
        name: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Provider>> {
        self.inner.get_provider_by_name(tenant_id, name).await
    }

    async fn list_providers(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Provider>> {
        self.inner.list_providers(tenant_id).await
    }

    async fn get_enabled_providers(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Provider>> {
        self.inner.get_enabled_providers(tenant_id).await
    }
}

#[async_trait]
impl AgentStore for BoundedInMemoryStore {
    async fn get_agent(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        self.inner.get_agent(id).await
    }

    async fn get_agent_by_code(
        &self,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        self.inner.get_agent_by_code(code).await
    }

    async fn get_agent_by_code_and_tenant(
        &self,
        tenant_id: Uuid,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        self.inner
            .get_agent_by_code_and_tenant(tenant_id, code)
            .await
    }

    async fn list_agents(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Agent>> {
        self.inner.list_agents(tenant_id).await
    }

    async fn get_active_agents(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Agent>> {
        self.inner.get_active_agents(tenant_id).await
    }

    async fn get_agent_with_provider(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        self.inner.get_agent_with_provider(id).await
    }

    async fn get_agent_by_code_with_provider(
        &self,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        self.inner.get_agent_by_code_with_provider(code).await
    }

    async fn get_agent_by_code_and_tenant_with_provider(
        &self,
        tenant_id: Uuid,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        self.inner
            .get_agent_by_code_and_tenant_with_provider(tenant_id, code)
            .await
    }
}

impl PersistenceStore for BoundedInMemoryStore {
    fn backend_name(&self) -> &str {
        "bounded-memory"
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    async fn close(&self) -> PersistenceResult<()> {
        self.inner.close().await
    }
}
