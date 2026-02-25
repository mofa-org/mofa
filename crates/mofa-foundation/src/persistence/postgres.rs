//! PostgreSQL 存储后端
//! PostgreSQL Storage Backend
//!
//! 提供基于 PostgreSQL 的持久化实现
//! Provides persistence implementation based on PostgreSQL

use super::entities::*;
use super::traits::*;
use async_trait::async_trait;
use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::error;
use uuid::Uuid;

/// PostgreSQL 存储
/// PostgreSQL Storage
///
/// 基于 PostgreSQL 的持久化存储实现
/// Persistence storage implementation based on PostgreSQL
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::persistence::PostgresStore;
///
/// let store = PostgresStore::connect("postgres://user:pass@localhost/db").await?;
///
/// // 保存消息
/// // Save message
/// store.save_message(&message).await?;
/// ```
pub struct PostgresStore {
    /// 连接池
    /// Connection pool
    pool: PgPool,
    /// 连接状态
    /// Connection status
    connected: AtomicBool,
}

impl PostgresStore {
    /// 连接到 PostgreSQL 数据库
    /// Connect to PostgreSQL database
    pub async fn connect(database_url: &str) -> PersistenceResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| PersistenceError::Connection(e.to_string()))?;

        Ok(Self {
            pool,
            connected: AtomicBool::new(true),
        })
    }

    /// 使用自定义连接池选项连接
    /// Connect with custom connection pool options
    pub async fn connect_with_options(
        database_url: &str,
        max_connections: u32,
    ) -> PersistenceResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .map_err(|e| PersistenceError::Connection(e.to_string()))?;

        Ok(Self {
            pool,
            connected: AtomicBool::new(true),
        })
    }

    /// 从现有连接池创建
    /// Create from an existing connection pool
    pub fn from_pool(pool: PgPool) -> Self {
        Self {
            pool,
            connected: AtomicBool::new(true),
        }
    }

    /// 创建共享实例
    /// Create a shared instance
    pub async fn shared(database_url: &str) -> PersistenceResult<Arc<Self>> {
        Ok(Arc::new(Self::connect(database_url).await?))
    }

    /// 从环境变量 DATABASE_URL 创建共享实例
    /// Create a shared instance from DATABASE_URL environment variable
    ///
    /// 环境变量：
    /// Environment variables:
    /// - DATABASE_URL: PostgreSQL 连接字符串（必需）
    /// - DATABASE_URL: PostgreSQL connection string (required)
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_foundation::persistence::PostgresStore;
    ///
    /// let store = PostgresStore::from_env().await?;
    /// ```
    pub async fn from_env() -> PersistenceResult<Arc<Self>> {
        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            PersistenceError::Other("DATABASE_URL environment variable not set".to_string())
        })?;
        Self::shared(&database_url).await
    }

    /// 从环境变量创建，支持自定义连接池大小
    /// Create from environment variables with custom pool size
    ///
    /// 环境变量：
    /// Environment variables:
    /// - DATABASE_URL: PostgreSQL 连接字符串（必需）
    /// - DATABASE_URL: PostgreSQL connection string (required)
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_foundation::persistence::PostgresStore;
    ///
    /// let store = PostgresStore::from_env_with_options(20).await?;
    /// ```
    pub async fn from_env_with_options(max_connections: u32) -> PersistenceResult<Arc<Self>> {
        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            PersistenceError::Other("DATABASE_URL environment variable not set".to_string())
        })?;
        Ok(Arc::new(
            Self::connect_with_options(&database_url, max_connections).await?,
        ))
    }

    /// 获取连接池引用
    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// 从行解析消息
    /// Parse message from a row
    fn parse_message_row(row: &PgRow) -> PersistenceResult<LLMMessage> {
        let content_json: serde_json::Value = row
            .try_get("content")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let content: MessageContent = serde_json::from_value(content_json)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let role_str: String = row
            .try_get("role")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let role = role_str
            .parse()
            .map_err(|e: String| PersistenceError::Serialization(e))?;

        Ok(LLMMessage {
            id: row
                .try_get("id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            chat_session_id: row
                .try_get("chat_session_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            agent_id: row
                .try_get("agent_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            user_id: row
                .try_get("user_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            tenant_id: row.try_get("tenant_id").unwrap_or_else(|_| Uuid::nil()),
            parent_message_id: row.try_get("parent_message_id").ok(),
            role,
            content,
            create_time: row
                .try_get("create_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            update_time: row
                .try_get("update_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
        })
    }

    /// 从行解析 API 调用
    /// Parse API call from a row
    fn parse_api_call_row(row: &PgRow) -> PersistenceResult<LLMApiCall> {
        let status_str: String = row
            .try_get("status")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let status = match status_str.as_str() {
            "success" => ApiCallStatus::Success,
            "failed" => ApiCallStatus::Failed,
            "timeout" => ApiCallStatus::Timeout,
            "rate_limited" => ApiCallStatus::RateLimited,
            "cancelled" => ApiCallStatus::Cancelled,
            _ => ApiCallStatus::Failed,
        };

        let prompt_tokens_details: Option<TokenDetails> = row
            .try_get::<Option<serde_json::Value>, _>("prompt_tokens_details")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok());

        let completion_tokens_details: Option<TokenDetails> = row
            .try_get::<Option<serde_json::Value>, _>("completion_tokens_details")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok());

        let price_details: Option<PriceDetails> = row
            .try_get::<Option<serde_json::Value>, _>("price_details")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok());

        Ok(LLMApiCall {
            id: row
                .try_get("id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            chat_session_id: row
                .try_get("chat_session_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            agent_id: row
                .try_get("agent_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            user_id: row
                .try_get("user_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            tenant_id: row
                .try_get("tenant_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            request_message_id: row
                .try_get("request_message_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            response_message_id: row
                .try_get("response_message_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            model_name: row
                .try_get("model_name")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            status,
            error_message: row.try_get("error_message").ok(),
            error_code: row.try_get("error_code").ok(),
            prompt_tokens: row.try_get("prompt_tokens").unwrap_or(0),
            completion_tokens: row.try_get("completion_tokens").unwrap_or(0),
            total_tokens: row.try_get("total_tokens").unwrap_or(0),
            prompt_tokens_details,
            completion_tokens_details,
            total_price: row.try_get("total_price").ok(),
            price_details,
            latency_ms: row.try_get("latency_ms").ok(),
            time_to_first_token_ms: row.try_get("time_to_first_token_ms").ok(),
            tokens_per_second: row.try_get("tokens_per_second").ok(),
            api_response_id: row.try_get("api_response_id").ok(),
            create_time: row
                .try_get("create_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            update_time: row
                .try_get("update_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
        })
    }

    /// 从行解析会话
    /// Parse session from a row
    fn parse_session_row(row: &PgRow) -> PersistenceResult<ChatSession> {
        let metadata: HashMap<String, serde_json::Value> = row
            .try_get::<Option<serde_json::Value>, _>("metadata")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        Ok(ChatSession {
            id: row
                .try_get("id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            user_id: row
                .try_get("user_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            agent_id: row
                .try_get("agent_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            tenant_id: row.try_get("tenant_id").unwrap_or_else(|_| Uuid::nil()),
            title: row.try_get("title").ok(),
            metadata,
            create_time: row
                .try_get("create_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            update_time: row
                .try_get("update_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
        })
    }

    /// 从行解析 provider
    /// Parse provider from a row
    fn parse_provider_row(
        row: &PgRow,
    ) -> PersistenceResult<crate::persistence::entities::Provider> {
        Ok(crate::persistence::entities::Provider {
            id: row
                .try_get("id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            tenant_id: row
                .try_get("tenant_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            provider_name: row
                .try_get("provider_name")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            provider_type: row
                .try_get("provider_type")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            api_base: row
                .try_get("api_base")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            api_key: row
                .try_get("api_key")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            enabled: row
                .try_get("enabled")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            create_time: row
                .try_get("create_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            update_time: row
                .try_get("update_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
        })
    }

    /// 从行解析 agent
    /// Parse agent from a row
    fn parse_agent_row(row: &PgRow) -> PersistenceResult<crate::persistence::entities::Agent> {
        Ok(crate::persistence::entities::Agent {
            id: row
                .try_get("id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            tenant_id: row
                .try_get("tenant_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            agent_code: row
                .try_get("agent_code")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            agent_name: row
                .try_get("agent_name")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            agent_order: row
                .try_get("agent_order")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            agent_status: row
                .try_get("agent_status")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            context_limit: row.try_get("context_limit").ok(),
            custom_params: row
                .try_get::<Option<serde_json::Value>, _>("custom_params")
                .ok()
                .flatten(),
            max_completion_tokens: row.try_get("max_completion_tokens").ok(),
            model_name: row
                .try_get("model_name")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            provider_id: row
                .try_get("provider_id")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            response_format: row.try_get("response_format").ok(),
            system_prompt: row
                .try_get("system_prompt")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            temperature: row.try_get("temperature").ok(),
            stream: row.try_get("stream").ok(),
            thinking: row
                .try_get::<Option<serde_json::Value>, _>("thinking")
                .ok()
                .flatten(),
            create_time: row
                .try_get("create_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            update_time: row
                .try_get("update_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
        })
    }
}

#[async_trait]
impl MessageStore for PostgresStore {
    async fn save_message(&self, message: &LLMMessage) -> PersistenceResult<()> {
        let content_json = serde_json::to_value(&message.content)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO entity_llm_message
            (id, chat_session_id, agent_id, user_id, tenant_id, parent_message_id, role, content, create_time, update_time)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE SET
                content = EXCLUDED.content,
                update_time = EXCLUDED.update_time
            "#,
        )
        .bind(message.id)
        .bind(message.chat_session_id)
        .bind(message.agent_id)
        .bind(message.user_id)
        .bind(message.tenant_id)
        .bind(message.parent_message_id)
        .bind(message.role.to_string())
        .bind(content_json)
        .bind(message.create_time)
        .bind(message.update_time)
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_message(&self, id: Uuid) -> PersistenceResult<Option<LLMMessage>> {
        let row = sqlx::query("SELECT * FROM entity_llm_message WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_message_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_session_messages(&self, session_id: Uuid) -> PersistenceResult<Vec<LLMMessage>> {
        let rows = sqlx::query(
            "SELECT * FROM entity_llm_message WHERE chat_session_id = $1 ORDER BY create_time ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_message_row).collect()
    }

    async fn get_session_messages_paginated(
        &self,
        session_id: Uuid,
        offset: i64,
        limit: i64,
    ) -> PersistenceResult<Vec<LLMMessage>> {
        let rows = sqlx::query(
            "SELECT * FROM entity_llm_message WHERE chat_session_id = $1 ORDER BY create_time ASC LIMIT $2 OFFSET $3",
        )
        .bind(session_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_message_row).collect()
    }

    async fn delete_message(&self, id: Uuid) -> PersistenceResult<bool> {
        let result = sqlx::query("DELETE FROM entity_llm_message WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        let result = sqlx::query("DELETE FROM entity_llm_message WHERE chat_session_id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    async fn count_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM entity_llm_message WHERE chat_session_id = $1",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        let count: i64 = row
            .try_get("count")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        Ok(count)
    }
}

#[async_trait]
impl ApiCallStore for PostgresStore {
    async fn save_api_call(&self, call: &LLMApiCall) -> PersistenceResult<()> {
        let prompt_tokens_details = call
            .prompt_tokens_details
            .as_ref()
            .and_then(|d| serde_json::to_value(d).ok());
        let completion_tokens_details = call
            .completion_tokens_details
            .as_ref()
            .and_then(|d| serde_json::to_value(d).ok());
        let price_details = call
            .price_details
            .as_ref()
            .and_then(|d| serde_json::to_value(d).ok());

        sqlx::query(
            r#"
            INSERT INTO entity_llm_api_call
            (id, chat_session_id, agent_id, user_id, tenant_id, request_message_id, response_message_id,
             model_name, status, error_message, error_code, prompt_tokens, completion_tokens, total_tokens,
             prompt_tokens_details, completion_tokens_details, total_price, price_details,
             latency_ms, time_to_first_token_ms, tokens_per_second, api_response_id, create_time, update_time)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                error_message = EXCLUDED.error_message,
                error_code = EXCLUDED.error_code,
                response_message_id = EXCLUDED.response_message_id,
                prompt_tokens = EXCLUDED.prompt_tokens,
                completion_tokens = EXCLUDED.completion_tokens,
                total_tokens = EXCLUDED.total_tokens,
                prompt_tokens_details = EXCLUDED.prompt_tokens_details,
                completion_tokens_details = EXCLUDED.completion_tokens_details,
                total_price = EXCLUDED.total_price,
                price_details = EXCLUDED.price_details,
                latency_ms = EXCLUDED.latency_ms,
                time_to_first_token_ms = EXCLUDED.time_to_first_token_ms,
                tokens_per_second = EXCLUDED.tokens_per_second,
                api_response_id = EXCLUDED.api_response_id,
                update_time = EXCLUDED.update_time
            "#,
        )
        .bind(call.id)
        .bind(call.chat_session_id)
        .bind(call.agent_id)
        .bind(call.user_id)
        .bind(call.tenant_id)
        .bind(call.request_message_id)
        .bind(call.response_message_id)
        .bind(&call.model_name)
        .bind(call.status.to_string())
        .bind(&call.error_message)
        .bind(&call.error_code)
        .bind(call.prompt_tokens)
        .bind(call.completion_tokens)
        .bind(call.total_tokens)
        .bind(prompt_tokens_details)
        .bind(completion_tokens_details)
        .bind(call.total_price)
        .bind(price_details)
        .bind(call.latency_ms)
        .bind(call.time_to_first_token_ms)
        .bind(call.tokens_per_second)
        .bind(&call.api_response_id)
        .bind(call.create_time)
        .bind(call.update_time)
        .execute(&self.pool)
        .await
        .map_err(|e|{
            error!("save_api_call failed: {}", e.to_string());
            PersistenceError::Query(e.to_string())})?;

        Ok(())
    }

    async fn get_api_call(&self, id: Uuid) -> PersistenceResult<Option<LLMApiCall>> {
        let row = sqlx::query("SELECT * FROM entity_llm_api_call WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_api_call_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn query_api_calls(&self, filter: &QueryFilter) -> PersistenceResult<Vec<LLMApiCall>> {
        let mut sql = String::from("SELECT * FROM entity_llm_api_call WHERE 1=1");
        let mut params: Vec<Box<dyn std::any::Any + Send + Sync>> = Vec::new();
        let mut param_count = 0;

        if filter.user_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND user_id = ${}", param_count));
        }
        if filter.session_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND chat_session_id = ${}", param_count));
        }
        if filter.agent_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND agent_id = ${}", param_count));
        }
        if filter.start_time.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND create_time >= ${}", param_count));
        }
        if filter.end_time.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND create_time <= ${}", param_count));
        }
        if filter.status.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND status = ${}", param_count));
        }
        if filter.model_name.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND model_name = ${}", param_count));
        }

        sql.push_str(" ORDER BY create_time DESC");

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        param_count += 1;
        sql.push_str(&format!(" LIMIT ${}", param_count));
        param_count += 1;
        sql.push_str(&format!(" OFFSET ${}", param_count));

        let mut query = sqlx::query(&sql);

        if let Some(user_id) = filter.user_id {
            query = query.bind(user_id);
        }
        if let Some(session_id) = filter.session_id {
            query = query.bind(session_id);
        }
        if let Some(agent_id) = filter.agent_id {
            query = query.bind(agent_id);
        }
        if let Some(start_time) = filter.start_time {
            query = query.bind(start_time);
        }
        if let Some(end_time) = filter.end_time {
            query = query.bind(end_time);
        }
        if let Some(status) = filter.status {
            query = query.bind(status.to_string());
        }
        if let Some(ref model_name) = filter.model_name {
            query = query.bind(model_name);
        }
        query = query.bind(limit);
        query = query.bind(offset);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_api_call_row).collect()
    }

    async fn get_statistics(&self, filter: &QueryFilter) -> PersistenceResult<UsageStatistics> {
        let mut sql = String::from(
            r#"
            SELECT
                COUNT(*) as total_calls,
                COUNT(CASE WHEN status = 'success' THEN 1 END) as success_count,
                COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed_count,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COALESCE(SUM(prompt_tokens), 0) as total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) as total_completion_tokens,
                SUM(total_price) as total_cost,
                AVG(latency_ms) as avg_latency_ms,
                AVG(tokens_per_second) as avg_tokens_per_second
            FROM entity_llm_api_call WHERE 1=1
            "#,
        );

        let mut param_count = 0;

        if filter.user_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND user_id = ${}", param_count));
        }
        if filter.session_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND chat_session_id = ${}", param_count));
        }
        if filter.agent_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND agent_id = ${}", param_count));
        }
        if filter.start_time.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND create_time >= ${}", param_count));
        }
        if filter.end_time.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND create_time <= ${}", param_count));
        }

        let mut query = sqlx::query(&sql);

        if let Some(user_id) = filter.user_id {
            query = query.bind(user_id);
        }
        if let Some(session_id) = filter.session_id {
            query = query.bind(session_id);
        }
        if let Some(agent_id) = filter.agent_id {
            query = query.bind(agent_id);
        }
        if let Some(start_time) = filter.start_time {
            query = query.bind(start_time);
        }
        if let Some(end_time) = filter.end_time {
            query = query.bind(end_time);
        }

        let row = query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(UsageStatistics {
            total_calls: row.try_get("total_calls").unwrap_or(0),
            success_count: row.try_get("success_count").unwrap_or(0),
            failed_count: row.try_get("failed_count").unwrap_or(0),
            total_tokens: row.try_get("total_tokens").unwrap_or(0),
            total_prompt_tokens: row.try_get("total_prompt_tokens").unwrap_or(0),
            total_completion_tokens: row.try_get("total_completion_tokens").unwrap_or(0),
            total_cost: row.try_get("total_cost").ok(),
            avg_latency_ms: row.try_get("avg_latency_ms").ok(),
            avg_tokens_per_second: row.try_get("avg_tokens_per_second").ok(),
        })
    }

    async fn delete_api_call(&self, id: Uuid) -> PersistenceResult<bool> {
        let result = sqlx::query("DELETE FROM entity_llm_api_call WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn cleanup_old_records(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> PersistenceResult<i64> {
        let result = sqlx::query("DELETE FROM entity_llm_api_call WHERE create_time < $1")
            .bind(before)
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }
}

#[async_trait]
impl SessionStore for PostgresStore {
    async fn create_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        let metadata = serde_json::to_value(&session.metadata)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO entity_chat_session (id, user_id, agent_id, tenant_id, title, metadata, create_time, update_time)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                title = EXCLUDED.title,
                metadata = EXCLUDED.metadata,
                update_time = EXCLUDED.update_time
            "#,
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(session.agent_id)
        .bind(session.tenant_id)
        .bind(&session.title)
        .bind(metadata)
        .bind(session.create_time)
        .bind(session.update_time)
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_session(&self, id: Uuid) -> PersistenceResult<Option<ChatSession>> {
        let row = sqlx::query("SELECT * FROM entity_chat_session WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_session_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_user_sessions(&self, user_id: Uuid) -> PersistenceResult<Vec<ChatSession>> {
        let rows = sqlx::query(
            "SELECT * FROM entity_chat_session WHERE user_id = $1 ORDER BY update_time DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_session_row).collect()
    }

    async fn update_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        let metadata = serde_json::to_value(&session.metadata)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let result = sqlx::query(
            r#"
            UPDATE entity_chat_session
            SET title = $2, metadata = $3, update_time = $4
            WHERE id = $1
            "#,
        )
        .bind(session.id)
        .bind(&session.title)
        .bind(metadata)
        .bind(session.update_time)
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(PersistenceError::NotFound(format!(
                "Session {} not found",
                session.id
            )));
        }

        Ok(())
    }

    async fn delete_session(&self, id: Uuid) -> PersistenceResult<bool> {
        let result = sqlx::query("DELETE FROM entity_chat_session WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl crate::persistence::traits::ProviderStore for PostgresStore {
    async fn get_provider(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::Provider>> {
        let row = sqlx::query("SELECT * FROM entity_provider WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_provider_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_provider_by_name(
        &self,
        tenant_id: Uuid,
        name: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Provider>> {
        let row = sqlx::query(
            "SELECT * FROM entity_provider WHERE tenant_id = $1 AND provider_name = $2",
        )
        .bind(tenant_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_provider_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_providers(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Provider>> {
        let rows = sqlx::query("SELECT * FROM entity_provider WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_provider_row).collect()
    }

    async fn get_enabled_providers(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Provider>> {
        let rows =
            sqlx::query("SELECT * FROM entity_provider WHERE tenant_id = $1 AND enabled = true")
                .bind(tenant_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_provider_row).collect()
    }
}

#[async_trait]
impl crate::persistence::traits::AgentStore for PostgresStore {
    async fn get_agent(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        let row = sqlx::query("SELECT * FROM entity_agent WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_agent_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_agent_by_code(
        &self,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        let row = sqlx::query("SELECT * FROM entity_agent WHERE agent_code = $1")
            .bind(code)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_agent_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_agent_by_code_and_tenant(
        &self,
        tenant_id: Uuid,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::Agent>> {
        let row =
            sqlx::query("SELECT * FROM entity_agent WHERE tenant_id = $1 AND agent_code = $2")
                .bind(tenant_id)
                .bind(code)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_agent_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_agents(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Agent>> {
        let rows = sqlx::query("SELECT * FROM entity_agent WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_agent_row).collect()
    }

    async fn get_active_agents(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<crate::persistence::entities::Agent>> {
        let rows =
            sqlx::query("SELECT * FROM entity_agent WHERE tenant_id = $1 AND agent_status = true")
                .bind(tenant_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_agent_row).collect()
    }

    async fn get_agent_with_provider(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        let row = sqlx::query(
            r#"
            SELECT
                a.*,
                p.id as provider_id,
                p.tenant_id as provider_tenant_id,
                p.provider_name,
                p.provider_type,
                p.api_base,
                p.api_key,
                p.enabled as provider_enabled,
                p.create_time as provider_create_time,
                p.update_time as provider_update_time
            FROM entity_agent a
            INNER JOIN entity_provider p ON a.provider_id = p.id
            WHERE a.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let provider = crate::persistence::entities::Provider {
                    id: row
                        .try_get("provider_id")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    tenant_id: row
                        .try_get("provider_tenant_id")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    provider_name: row
                        .try_get("provider_name")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    provider_type: row
                        .try_get("provider_type")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    api_base: row
                        .try_get("api_base")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    api_key: row
                        .try_get("api_key")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    enabled: row
                        .try_get("provider_enabled")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    create_time: row
                        .try_get("provider_create_time")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    update_time: row
                        .try_get("provider_update_time")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                };
                let agent = Self::parse_agent_row(&row)?;
                Ok(Some(crate::persistence::entities::AgentConfig {
                    provider,
                    agent,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_agent_by_code_with_provider(
        &self,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        let row = sqlx::query(
            r#"
            SELECT
                a.*,
                p.id as provider_id,
                p.tenant_id as provider_tenant_id,
                p.provider_name,
                p.provider_type,
                p.api_base,
                p.api_key,
                p.enabled as provider_enabled,
                p.create_time as provider_create_time,
                p.update_time as provider_update_time
            FROM entity_agent a
            INNER JOIN entity_provider p ON a.provider_id = p.id
            WHERE a.agent_code = $1
            "#,
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let provider = crate::persistence::entities::Provider {
                    id: row
                        .try_get("provider_id")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    tenant_id: row
                        .try_get("provider_tenant_id")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    provider_name: row
                        .try_get("provider_name")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    provider_type: row
                        .try_get("provider_type")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    api_base: row
                        .try_get("api_base")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    api_key: row
                        .try_get("api_key")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    enabled: row
                        .try_get("provider_enabled")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    create_time: row
                        .try_get("provider_create_time")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    update_time: row
                        .try_get("provider_update_time")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                };
                let agent = Self::parse_agent_row(&row)?;
                Ok(Some(crate::persistence::entities::AgentConfig {
                    provider,
                    agent,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_agent_by_code_and_tenant_with_provider(
        &self,
        tenant_id: Uuid,
        code: &str,
    ) -> PersistenceResult<Option<crate::persistence::entities::AgentConfig>> {
        let row = sqlx::query(
            r#"
            SELECT
                a.*,
                p.id as provider_id,
                p.tenant_id as provider_tenant_id,
                p.provider_name,
                p.provider_type,
                p.api_base,
                p.api_key,
                p.enabled as provider_enabled,
                p.create_time as provider_create_time,
                p.update_time as provider_update_time
            FROM entity_agent a
            INNER JOIN entity_provider p ON a.provider_id = p.id
            WHERE a.tenant_id = $1 AND a.agent_code = $2
            "#,
        )
        .bind(tenant_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let provider = crate::persistence::entities::Provider {
                    id: row
                        .try_get("provider_id")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    tenant_id: row
                        .try_get("provider_tenant_id")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    provider_name: row
                        .try_get("provider_name")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    provider_type: row
                        .try_get("provider_type")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    api_base: row
                        .try_get("api_base")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    api_key: row
                        .try_get("api_key")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    enabled: row
                        .try_get("provider_enabled")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    create_time: row
                        .try_get("provider_create_time")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                    update_time: row
                        .try_get("provider_update_time")
                        .map_err(|e| PersistenceError::Query(e.to_string()))?,
                };
                let agent = Self::parse_agent_row(&row)?;
                Ok(Some(crate::persistence::entities::AgentConfig {
                    provider,
                    agent,
                }))
            }
            None => Ok(None),
        }
    }
}

impl PersistenceStore for PostgresStore {
    fn backend_name(&self) -> &str {
        "postgres"
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn close(&self) -> PersistenceResult<()> {
        self.pool.close().await;
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}
