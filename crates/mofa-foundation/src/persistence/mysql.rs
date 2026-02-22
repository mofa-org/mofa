//! MySQL/MariaDB 存储后端
//! MySQL/MariaDB storage backend
//!
//! 提供基于 MySQL/MariaDB 的持久化实现
//! Provides a persistence implementation based on MySQL/MariaDB

use super::entities::*;
use super::traits::*;
use async_trait::async_trait;
use sqlx::Row;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions, MySqlRow};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

/// MySQL 存储
/// MySQL storage
pub struct MySqlStore {
    pool: MySqlPool,
    connected: AtomicBool,
}

impl MySqlStore {
    pub async fn connect(database_url: &str) -> PersistenceResult<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| PersistenceError::Connection(e.to_string()))?;

        Ok(Self {
            pool,
            connected: AtomicBool::new(true),
        })
    }

    pub async fn connect_with_options(
        database_url: &str,
        max_connections: u32,
    ) -> PersistenceResult<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .map_err(|e| PersistenceError::Connection(e.to_string()))?;

        Ok(Self {
            pool,
            connected: AtomicBool::new(true),
        })
    }

    pub fn from_pool(pool: MySqlPool) -> Self {
        Self {
            pool,
            connected: AtomicBool::new(true),
        }
    }

    pub async fn shared(database_url: &str) -> PersistenceResult<Arc<Self>> {
        Ok(Arc::new(Self::connect(database_url).await?))
    }

    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    fn parse_message_row(row: &MySqlRow) -> PersistenceResult<LLMMessage> {
        let content_str: String = row
            .try_get("content")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let content: MessageContent = serde_json::from_str(&content_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let role_str: String = row
            .try_get("role")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let role = role_str
            .parse()
            .map_err(|e: String| PersistenceError::Serialization(e))?;

        let id_str: String = row
            .try_get("id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let id =
            Uuid::parse_str(&id_str).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let session_id_str: String = row
            .try_get("chat_session_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let chat_session_id = Uuid::parse_str(&session_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let agent_id_str: String = row
            .try_get("agent_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let agent_id = Uuid::parse_str(&agent_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let user_id_str: String = row
            .try_get("user_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let user_id = Uuid::parse_str(&user_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let tenant_id: Uuid = row
            .try_get::<Option<String>, _>("tenant_id")
            .ok()
            .flatten()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or(Uuid::nil());

        let parent_message_id: Option<Uuid> = row
            .try_get::<Option<String>, _>("parent_message_id")
            .ok()
            .flatten()
            .and_then(|s| Uuid::parse_str(&s).ok());

        Ok(LLMMessage {
            id,
            chat_session_id,
            agent_id,
            user_id,
            tenant_id,
            parent_message_id,
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

    fn parse_api_call_row(row: &MySqlRow) -> PersistenceResult<LLMApiCall> {
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

        let id_str: String = row
            .try_get("id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let id =
            Uuid::parse_str(&id_str).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let session_id_str: String = row
            .try_get("chat_session_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let chat_session_id = Uuid::parse_str(&session_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let agent_id_str: String = row
            .try_get("agent_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let agent_id = Uuid::parse_str(&agent_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let user_id_str: String = row
            .try_get("user_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let user_id = Uuid::parse_str(&user_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let tenant_id: Uuid = row
            .try_get::<Option<String>, _>("tenant_id")
            .ok()
            .flatten()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or(Uuid::nil());

        let request_message_id_str: String = row
            .try_get("request_message_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let request_message_id = Uuid::parse_str(&request_message_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let response_message_id_str: String = row
            .try_get("response_message_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let response_message_id = Uuid::parse_str(&response_message_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let prompt_tokens_details: Option<TokenDetails> = row
            .try_get::<Option<String>, _>("prompt_tokens_details")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        let completion_tokens_details: Option<TokenDetails> = row
            .try_get::<Option<String>, _>("completion_tokens_details")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        let price_details: Option<PriceDetails> = row
            .try_get::<Option<String>, _>("price_details")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        Ok(LLMApiCall {
            id,
            chat_session_id,
            agent_id,
            user_id,
            tenant_id,
            request_message_id,
            response_message_id,
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

    fn parse_session_row(row: &MySqlRow) -> PersistenceResult<ChatSession> {
        let id_str: String = row
            .try_get("id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let id =
            Uuid::parse_str(&id_str).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let user_id_str: String = row
            .try_get("user_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let user_id = Uuid::parse_str(&user_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let agent_id_str: String = row
            .try_get("agent_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let agent_id = Uuid::parse_str(&agent_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let tenant_id: Uuid = row
            .try_get::<Option<String>, _>("tenant_id")
            .ok()
            .flatten()
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or(Uuid::nil());

        let metadata: HashMap<String, serde_json::Value> = row
            .try_get::<Option<String>, _>("metadata")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(ChatSession {
            id,
            user_id,
            agent_id,
            tenant_id,
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
}

#[async_trait]
impl MessageStore for MySqlStore {
    async fn save_message(&self, message: &LLMMessage) -> PersistenceResult<()> {
        let content_json = serde_json::to_string(&message.content)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO entity_llm_message
            (id, chat_session_id, agent_id, user_id, tenant_id, parent_message_id, role, content, create_time, update_time)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE content = VALUES(content), update_time = VALUES(update_time)
            "#,
        )
        .bind(message.id.to_string())
        .bind(message.chat_session_id.to_string())
        .bind(message.agent_id.to_string())
        .bind(message.user_id.to_string())
        .bind(message.tenant_id.to_string())
        .bind(message.parent_message_id.map(|u| u.to_string()))
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
        let row = sqlx::query("SELECT * FROM entity_llm_message WHERE id = ?")
            .bind(id.to_string())
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
            "SELECT * FROM entity_llm_message WHERE chat_session_id = ? ORDER BY create_time ASC",
        )
        .bind(session_id.to_string())
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
        let rows = sqlx::query("SELECT * FROM entity_llm_message WHERE chat_session_id = ? ORDER BY create_time ASC LIMIT ? OFFSET ?")
            .bind(session_id.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_message_row).collect()
    }

    async fn delete_message(&self, id: Uuid) -> PersistenceResult<bool> {
        let result = sqlx::query("DELETE FROM entity_llm_message WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        let result = sqlx::query("DELETE FROM entity_llm_message WHERE chat_session_id = ?")
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }

    async fn count_session_messages(&self, session_id: Uuid) -> PersistenceResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM entity_llm_message WHERE chat_session_id = ?",
        )
        .bind(session_id.to_string())
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
impl ApiCallStore for MySqlStore {
    async fn save_api_call(&self, call: &LLMApiCall) -> PersistenceResult<()> {
        let prompt_tokens_details = call
            .prompt_tokens_details
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());
        let completion_tokens_details = call
            .completion_tokens_details
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());
        let price_details = call
            .price_details
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());

        sqlx::query(
            r#"
            INSERT INTO entity_llm_api_call
            (id, chat_session_id, agent_id, user_id, tenant_id, request_message_id, response_message_id,
             model_name, status, error_message, error_code, prompt_tokens, completion_tokens, total_tokens,
             prompt_tokens_details, completion_tokens_details, total_price, price_details,
             latency_ms, time_to_first_token_ms, tokens_per_second, api_response_id,
             create_time, update_time)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                status = VALUES(status), error_message = VALUES(error_message),
                completion_tokens = VALUES(completion_tokens), total_tokens = VALUES(total_tokens),
                update_time = VALUES(update_time), latency_ms = VALUES(latency_ms)
            "#,
        )
        .bind(call.id.to_string())
        .bind(call.chat_session_id.to_string())
        .bind(call.agent_id.to_string())
        .bind(call.user_id.to_string())
        .bind(call.tenant_id.to_string())
        .bind(call.request_message_id.to_string())
        .bind(call.response_message_id.to_string())
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
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_api_call(&self, id: Uuid) -> PersistenceResult<Option<LLMApiCall>> {
        let row = sqlx::query("SELECT * FROM entity_llm_api_call WHERE id = ?")
            .bind(id.to_string())
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
        let mut binds: Vec<String> = Vec::new();

        if let Some(user_id) = filter.user_id {
            sql.push_str(" AND user_id = ?");
            binds.push(user_id.to_string());
        }
        if let Some(session_id) = filter.session_id {
            sql.push_str(" AND chat_session_id = ?");
            binds.push(session_id.to_string());
        }
        if let Some(agent_id) = filter.agent_id {
            sql.push_str(" AND agent_id = ?");
            binds.push(agent_id.to_string());
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            binds.push(status.to_string());
        }
        if let Some(ref model_name) = filter.model_name {
            sql.push_str(" AND model_name = ?");
            binds.push(model_name.clone());
        }

        sql.push_str(" ORDER BY create_time DESC");

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        let mut query = sqlx::query(&sql);
        for bind in &binds {
            query = query.bind(bind);
        }

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
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as success_count,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed_count,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COALESCE(SUM(prompt_tokens), 0) as total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) as total_completion_tokens,
                SUM(total_price) as total_cost,
                AVG(latency_ms) as avg_latency_ms,
                AVG(tokens_per_second) as avg_tokens_per_second
            FROM entity_llm_api_call WHERE 1=1
        "#,
        );

        let mut binds: Vec<String> = Vec::new();

        if let Some(user_id) = filter.user_id {
            sql.push_str(" AND user_id = ?");
            binds.push(user_id.to_string());
        }
        if let Some(session_id) = filter.session_id {
            sql.push_str(" AND chat_session_id = ?");
            binds.push(session_id.to_string());
        }
        if let Some(agent_id) = filter.agent_id {
            sql.push_str(" AND agent_id = ?");
            binds.push(agent_id.to_string());
        }

        let mut query = sqlx::query(&sql);
        for bind in &binds {
            query = query.bind(bind);
        }

        let row = query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(UsageStatistics {
            total_calls: row.try_get("total_calls").unwrap_or(0),
            success_count: row.try_get::<i64, _>("success_count").unwrap_or(0),
            failed_count: row.try_get::<i64, _>("failed_count").unwrap_or(0),
            total_tokens: row.try_get("total_tokens").unwrap_or(0),
            total_prompt_tokens: row.try_get("total_prompt_tokens").unwrap_or(0),
            total_completion_tokens: row.try_get("total_completion_tokens").unwrap_or(0),
            total_cost: row.try_get("total_cost").ok(),
            avg_latency_ms: row.try_get("avg_latency_ms").ok(),
            avg_tokens_per_second: row.try_get("avg_tokens_per_second").ok(),
        })
    }

    async fn delete_api_call(&self, id: Uuid) -> PersistenceResult<bool> {
        let result = sqlx::query("DELETE FROM entity_llm_api_call WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn cleanup_old_records(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> PersistenceResult<i64> {
        let result = sqlx::query("DELETE FROM entity_llm_api_call WHERE create_time < ?")
            .bind(before)
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }
}

#[async_trait]
impl SessionStore for MySqlStore {
    async fn create_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        let metadata = serde_json::to_string(&session.metadata)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        sqlx::query(r#"
            INSERT INTO entity_chat_session (id, user_id, agent_id, tenant_id, title, metadata, create_time, update_time)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#)
        .bind(session.id.to_string())
        .bind(session.user_id.to_string())
        .bind(session.agent_id.to_string())
        .bind(session.tenant_id.to_string())
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
        let row = sqlx::query("SELECT * FROM entity_chat_session WHERE id = ?")
            .bind(id.to_string())
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
            "SELECT * FROM entity_chat_session WHERE user_id = ? ORDER BY update_time DESC",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_session_row).collect()
    }

    async fn update_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        let metadata = serde_json::to_string(&session.metadata)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let result = sqlx::query(
            "UPDATE entity_chat_session SET title = ?, metadata = ?, update_time = ? WHERE id = ?",
        )
        .bind(&session.title)
        .bind(metadata)
        .bind(session.update_time)
        .bind(session.id.to_string())
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
        let result = sqlx::query("DELETE FROM entity_chat_session WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl ProviderStore for MySqlStore {
    async fn get_provider(&self, id: Uuid) -> PersistenceResult<Option<super::entities::Provider>> {
        let row = sqlx::query("SELECT * FROM entity_provider WHERE id = ?")
            .bind(id.to_string())
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
    ) -> PersistenceResult<Option<super::entities::Provider>> {
        let row =
            sqlx::query("SELECT * FROM entity_provider WHERE tenant_id = ? AND provider_name = ?")
                .bind(tenant_id.to_string())
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
    ) -> PersistenceResult<Vec<super::entities::Provider>> {
        let rows = sqlx::query(
            "SELECT * FROM entity_provider WHERE tenant_id = ? ORDER BY create_time DESC",
        )
        .bind(tenant_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_provider_row).collect()
    }

    async fn get_enabled_providers(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<super::entities::Provider>> {
        let rows = sqlx::query("SELECT * FROM entity_provider WHERE tenant_id = ? AND enabled = TRUE ORDER BY create_time DESC")
            .bind(tenant_id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_provider_row).collect()
    }
}

#[async_trait]
impl AgentStore for MySqlStore {
    async fn get_agent(&self, id: Uuid) -> PersistenceResult<Option<super::entities::Agent>> {
        let row = sqlx::query("SELECT * FROM entity_agent WHERE id = ?")
            .bind(id.to_string())
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
    ) -> PersistenceResult<Option<super::entities::Agent>> {
        let row = sqlx::query("SELECT * FROM entity_agent WHERE agent_code = ?")
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
    ) -> PersistenceResult<Option<super::entities::Agent>> {
        let row = sqlx::query("SELECT * FROM entity_agent WHERE tenant_id = ? AND agent_code = ?")
            .bind(tenant_id.to_string())
            .bind(code)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_agent_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_agents(&self, tenant_id: Uuid) -> PersistenceResult<Vec<super::entities::Agent>> {
        let rows =
            sqlx::query("SELECT * FROM entity_agent WHERE tenant_id = ? ORDER BY agent_order")
                .bind(tenant_id.to_string())
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_agent_row).collect()
    }

    async fn get_active_agents(
        &self,
        tenant_id: Uuid,
    ) -> PersistenceResult<Vec<super::entities::Agent>> {
        let rows = sqlx::query("SELECT * FROM entity_agent WHERE tenant_id = ? AND agent_status = TRUE ORDER BY agent_order")
            .bind(tenant_id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        rows.iter().map(Self::parse_agent_row).collect()
    }

    async fn get_agent_with_provider(
        &self,
        id: Uuid,
    ) -> PersistenceResult<Option<super::entities::AgentConfig>> {
        let row = sqlx::query(
            r#"
            SELECT
                a.*,
                p.id as provider_id, p.tenant_id as provider_tenant_id, p.provider_name, p.provider_type,
                p.api_base, p.api_key, p.enabled as provider_enabled,
                p.create_time as provider_create_time, p.update_time as provider_update_time
            FROM entity_agent a
            INNER JOIN entity_provider p ON a.provider_id = p.id
            WHERE a.id = ?
            "#
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let agent = Self::parse_agent_row_from_join(&row)?;
                let provider = Self::parse_provider_row_from_join(&row)?;
                Ok(Some(super::entities::AgentConfig { provider, agent }))
            }
            None => Ok(None),
        }
    }

    async fn get_agent_by_code_with_provider(
        &self,
        code: &str,
    ) -> PersistenceResult<Option<super::entities::AgentConfig>> {
        let row = sqlx::query(
            r#"
            SELECT
                a.*,
                p.id as provider_id, p.tenant_id as provider_tenant_id, p.provider_name, p.provider_type,
                p.api_base, p.api_key, p.enabled as provider_enabled,
                p.create_time as provider_create_time, p.update_time as provider_update_time
            FROM entity_agent a
            INNER JOIN entity_provider p ON a.provider_id = p.id
            WHERE a.agent_code = ?
            "#
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let agent = Self::parse_agent_row_from_join(&row)?;
                let provider = Self::parse_provider_row_from_join(&row)?;
                Ok(Some(super::entities::AgentConfig { provider, agent }))
            }
            None => Ok(None),
        }
    }

    async fn get_agent_by_code_and_tenant_with_provider(
        &self,
        tenant_id: Uuid,
        code: &str,
    ) -> PersistenceResult<Option<super::entities::AgentConfig>> {
        let row = sqlx::query(
            r#"
            SELECT
                a.*,
                p.id as provider_id, p.tenant_id as provider_tenant_id, p.provider_name, p.provider_type,
                p.api_base, p.api_key, p.enabled as provider_enabled,
                p.create_time as provider_create_time, p.update_time as provider_update_time
            FROM entity_agent a
            INNER JOIN entity_provider p ON a.provider_id = p.id
            WHERE a.tenant_id = ? AND a.agent_code = ?
            "#
        )
        .bind(tenant_id.to_string())
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let agent = Self::parse_agent_row_from_join(&row)?;
                let provider = Self::parse_provider_row_from_join(&row)?;
                Ok(Some(super::entities::AgentConfig { provider, agent }))
            }
            None => Ok(None),
        }
    }
}

impl MySqlStore {
    fn parse_provider_row(row: &MySqlRow) -> PersistenceResult<super::entities::Provider> {
        let id_str: String = row
            .try_get("id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let id =
            Uuid::parse_str(&id_str).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let tenant_id_str: String = row
            .try_get("tenant_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let tenant_id = Uuid::parse_str(&tenant_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        Ok(super::entities::Provider {
            id,
            tenant_id,
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

    fn parse_agent_row(row: &MySqlRow) -> PersistenceResult<super::entities::Agent> {
        let id_str: String = row
            .try_get("id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let id =
            Uuid::parse_str(&id_str).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let tenant_id_str: String = row
            .try_get("tenant_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let tenant_id = Uuid::parse_str(&tenant_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let provider_id_str: String = row
            .try_get("provider_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let provider_id = Uuid::parse_str(&provider_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let custom_params: Option<serde_json::Value> = row
            .try_get::<Option<String>, _>("custom_params")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        let thinking: Option<serde_json::Value> = row
            .try_get::<Option<String>, _>("thinking")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        Ok(super::entities::Agent {
            id,
            tenant_id,
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
            custom_params,
            max_completion_tokens: row.try_get("max_completion_tokens").ok(),
            model_name: row
                .try_get("model_name")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            provider_id,
            response_format: row.try_get("response_format").ok(),
            system_prompt: row
                .try_get("system_prompt")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            temperature: row.try_get("temperature").ok(),
            stream: row.try_get("stream").ok(),
            thinking,
            create_time: row
                .try_get("create_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            update_time: row
                .try_get("update_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
        })
    }

    fn parse_provider_row_from_join(
        row: &MySqlRow,
    ) -> PersistenceResult<super::entities::Provider> {
        let id_str: String = row
            .try_get("provider_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let id =
            Uuid::parse_str(&id_str).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let tenant_id_str: String = row
            .try_get("provider_tenant_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let tenant_id = Uuid::parse_str(&tenant_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        Ok(super::entities::Provider {
            id,
            tenant_id,
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
        })
    }

    fn parse_agent_row_from_join(row: &MySqlRow) -> PersistenceResult<super::entities::Agent> {
        let id_str: String = row
            .try_get("id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let id =
            Uuid::parse_str(&id_str).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let tenant_id_str: String = row
            .try_get("tenant_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let tenant_id = Uuid::parse_str(&tenant_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        // Use provider_id from the prefixed column
        let provider_id_str: String = row
            .try_get("provider_id")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let provider_id = Uuid::parse_str(&provider_id_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let custom_params: Option<serde_json::Value> = row
            .try_get::<Option<String>, _>("custom_params")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        let thinking: Option<serde_json::Value> = row
            .try_get::<Option<String>, _>("thinking")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        Ok(super::entities::Agent {
            id,
            tenant_id,
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
            custom_params,
            max_completion_tokens: row.try_get("max_completion_tokens").ok(),
            model_name: row
                .try_get("model_name")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            provider_id,
            response_format: row.try_get("response_format").ok(),
            system_prompt: row
                .try_get("system_prompt")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            temperature: row.try_get("temperature").ok(),
            stream: row.try_get("stream").ok(),
            thinking,
            create_time: row
                .try_get("create_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
            update_time: row
                .try_get("update_time")
                .map_err(|e| PersistenceError::Query(e.to_string()))?,
        })
    }
}

impl PersistenceStore for MySqlStore {
    fn backend_name(&self) -> &str {
        "mysql"
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
