//! 持久化核心 traits
//!
//! 定义存储后端必须实现的接口

use super::entities::*;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

/// 持久化错误
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    /// 连接错误
    #[error("Connection error: {0}")]
    Connection(String),
    /// 查询错误
    #[error("Query error: {0}")]
    Query(String),
    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// 记录未找到
    #[error("Record not found: {0}")]
    NotFound(String),
    /// 约束冲突
    #[error("Constraint violation: {0}")]
    Constraint(String),
    /// 其他错误
    #[error("Persistence error: {0}")]
    Other(String),
}

/// 持久化结果类型
pub type PersistenceResult<T> = Result<T, PersistenceError>;

/// 消息存储 trait
///
/// 提供 LLM 消息的 CRUD 操作
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// 保存消息
    async fn save_message(&self, message: &LLMMessage) -> PersistenceResult<()>;

    /// 批量保存消息
    async fn save_messages(&self, messages: &[LLMMessage]) -> PersistenceResult<()> {
        for msg in messages {
            self.save_message(msg).await?;
        }
        Ok(())
    }

    /// 获取消息
    async fn get_message(&self, id: Uuid) -> PersistenceResult<Option<LLMMessage>>;

    /// 获取会话消息列表
    async fn get_session_messages(&self, session_id: Uuid) -> PersistenceResult<Vec<LLMMessage>>;

    /// 获取会话消息列表 (分页)
    async fn get_session_messages_paginated(
        &self,
        session_id: Uuid,
        offset: i64,
        limit: i64,
    ) -> PersistenceResult<Vec<LLMMessage>>;

    /// 删除消息
    async fn delete_message(&self, id: Uuid) -> PersistenceResult<bool>;

    /// 删除会话所有消息
    async fn delete_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64>;

    /// 统计会话消息数
    async fn count_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64>;
}

/// API 调用记录存储 trait
///
/// 提供 LLM API 调用记录的存储和查询
#[async_trait]
pub trait ApiCallStore: Send + Sync {
    /// 保存 API 调用记录
    async fn save_api_call(&self, call: &LLMApiCall) -> PersistenceResult<()>;

    /// 批量保存 API 调用记录
    async fn save_api_calls(&self, calls: &[LLMApiCall]) -> PersistenceResult<()> {
        for call in calls {
            self.save_api_call(call).await?;
        }
        Ok(())
    }

    /// 获取 API 调用记录
    async fn get_api_call(&self, id: Uuid) -> PersistenceResult<Option<LLMApiCall>>;

    /// 查询 API 调用记录
    async fn query_api_calls(&self, filter: &QueryFilter) -> PersistenceResult<Vec<LLMApiCall>>;

    /// 统计 API 调用
    async fn get_statistics(&self, filter: &QueryFilter) -> PersistenceResult<UsageStatistics>;

    /// 删除 API 调用记录
    async fn delete_api_call(&self, id: Uuid) -> PersistenceResult<bool>;

    /// 清理旧记录
    async fn cleanup_old_records(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> PersistenceResult<i64>;
}

/// 会话存储 trait
///
/// 提供聊天会话的管理
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 创建会话
    async fn create_session(&self, session: &ChatSession) -> PersistenceResult<()>;

    /// 获取会话
    async fn get_session(&self, id: Uuid) -> PersistenceResult<Option<ChatSession>>;

    /// 获取用户会话列表
    async fn get_user_sessions(&self, user_id: Uuid) -> PersistenceResult<Vec<ChatSession>>;

    /// 更新会话
    async fn update_session(&self, session: &ChatSession) -> PersistenceResult<()>;

    /// 删除会话
    async fn delete_session(&self, id: Uuid) -> PersistenceResult<bool>;
}

/// Provider 存储 trait
///
/// 提供 LLM Provider 的数据库操作
#[async_trait]
pub trait ProviderStore: Send + Sync {
    /// 根据 ID 获取 provider
    async fn get_provider(&self, id: Uuid) -> PersistenceResult<Option<super::entities::Provider>>;

    /// 根据名称和租户 ID 获取 provider
    async fn get_provider_by_name(
        &self,
        tenant_id: Uuid,
        name: &str,
    ) -> PersistenceResult<Option<super::entities::Provider>>;

    /// 列出租户的所有 providers
    async fn list_providers(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<super::entities::Provider>>;

    /// 获取租户所有启用的 providers
    async fn get_enabled_providers(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<super::entities::Provider>>;
}

/// Agent 存储 trait
///
/// 提供 LLM Agent 配置的数据库操作
#[async_trait]
pub trait AgentStore: Send + Sync {
    /// 根据 ID 获取 agent
    async fn get_agent(&self, id: Uuid) -> PersistenceResult<Option<super::entities::Agent>>;

    /// 根据 code 获取 agent（全局查找）
    async fn get_agent_by_code(
        &self,
        code: &str,
    ) -> PersistenceResult<Option<super::entities::Agent>>;

    /// 根据 code 和租户 ID 获取 agent
    async fn get_agent_by_code_and_tenant(
        &self,
        tenant_id: Uuid,
        code: &str,
    ) -> PersistenceResult<Option<super::entities::Agent>>;

    /// 列出租户的所有 agents
    async fn list_agents(&self, tenant_id: Uuid) -> PersistenceResult<Vec<super::entities::Agent>>;

    /// 获取租户所有启用的 agents
    async fn get_active_agents(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<super::entities::Agent>>;

    /// 根据 ID 获取 agent 及其 provider 配置
    async fn get_agent_with_provider(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<super::entities::AgentConfig>>;

    /// 根据 code 获取 agent 及其 provider 配置（全局查找）
    async fn get_agent_by_code_with_provider(
        &self,
        code: &str,
    ) -> PersistenceResult<Option<super::entities::AgentConfig>>;

    /// 根据 code 和租户 ID 获取 agent 及其 provider 配置
    async fn get_agent_by_code_and_tenant_with_provider(
        &self,
        tenant_id: Uuid,
        code: &str,
    ) -> PersistenceResult<Option<super::entities::AgentConfig>>;
}

/// 完整的持久化存储 trait
///
/// 组合所有存储能力
pub trait PersistenceStore:
    MessageStore + ApiCallStore + SessionStore + ProviderStore + AgentStore
{
    /// 获取存储后端名称
    fn backend_name(&self) -> &str;

    /// 检查连接状态
    fn is_connected(&self) -> bool;

    /// 关闭连接
    fn close(&self) -> impl std::future::Future<Output = PersistenceResult<()>> + Send;
}

/// 存储工厂 trait
///
/// 用于创建存储实例
#[async_trait]
pub trait StoreFactory: Send + Sync {
    /// 存储类型
    type Store: PersistenceStore;

    /// 创建存储实例
    async fn create(&self, config: &str) -> PersistenceResult<Self::Store>;
}

/// 事务支持 trait (可选)
#[async_trait]
pub trait Transactional: Send + Sync {
    /// 事务类型
    type Transaction<'a>: Send + Sync
    where
        Self: 'a;

    /// 开始事务
    async fn begin_transaction(&self) -> PersistenceResult<Self::Transaction<'_>>;

    /// 提交事务
    async fn commit_transaction(&self, tx: Self::Transaction<'_>) -> PersistenceResult<()>;

    /// 回滚事务
    async fn rollback_transaction(&self, tx: Self::Transaction<'_>) -> PersistenceResult<()>;
}

/// 存储引用包装
///
/// 便于在多个组件间共享存储
pub type SharedStore<S> = Arc<S>;

/// 动态分发的存储类型
pub type DynMessageStore = Arc<dyn MessageStore>;
pub type DynApiCallStore = Arc<dyn ApiCallStore>;
pub type DynSessionStore = Arc<dyn SessionStore>;

/// 组合存储包装器
pub struct CompositeStore<M, A, S> {
    pub message_store: M,
    pub api_call_store: A,
    pub session_store: S,
}

impl<M, A, S> CompositeStore<M, A, S>
where
    M: MessageStore,
    A: ApiCallStore,
    S: SessionStore,
{
    pub fn new(message_store: M, api_call_store: A, session_store: S) -> Self {
        Self {
            message_store,
            api_call_store,
            session_store,
        }
    }
}

/// 存储事件
#[derive(Debug, Clone)]
pub enum StoreEvent {
    /// 消息已保存
    MessageSaved { message_id: Uuid, session_id: Uuid },
    /// API 调用已记录
    ApiCallRecorded { call_id: Uuid, session_id: Uuid },
    /// 会话已创建
    SessionCreated { session_id: Uuid },
    /// 会话已删除
    SessionDeleted { session_id: Uuid },
}

/// 存储事件监听器
#[async_trait]
pub trait StoreEventListener: Send + Sync {
    /// 处理事件
    async fn on_event(&self, event: StoreEvent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistence_error_display() {
        let err = PersistenceError::NotFound("user".to_string());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_query_filter_default() {
        let filter = QueryFilter::default();
        assert!(filter.user_id.is_none());
        assert!(filter.limit.is_none());
    }
}
