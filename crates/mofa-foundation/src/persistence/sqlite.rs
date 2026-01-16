//! SQLite 存储后端
//!
//! 提供基于 SQLite 的持久化实现，适用于轻量级部署和本地存储

use super::entities::*;
use super::traits::*;
use async_trait::async_trait;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

/// SQLite 存储
pub struct SqliteStore {
    pool: SqlitePool,
    connected: AtomicBool,
}

impl SqliteStore {
    pub async fn connect(database_url: &str) -> PersistenceResult<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| PersistenceError::Connection(e.to_string()))?;

        let store = Self {
            pool,
            connected: AtomicBool::new(true),
        };

        store.run_migrations().await?;
        Ok(store)
    }

    pub async fn in_memory() -> PersistenceResult<Self> {
        Self::connect("sqlite::memory:").await
    }

    pub async fn connect_with_options(
        database_url: &str,
        max_connections: u32,
    ) -> PersistenceResult<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .map_err(|e| PersistenceError::Connection(e.to_string()))?;

        let store = Self {
            pool,
            connected: AtomicBool::new(true),
        };

        store.run_migrations().await?;
        Ok(store)
    }

    pub fn from_pool(pool: SqlitePool) -> Self {
        Self {
            pool,
            connected: AtomicBool::new(true),
        }
    }

    pub async fn shared(database_url: &str) -> PersistenceResult<Arc<Self>> {
        Ok(Arc::new(Self::connect(database_url).await?))
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn run_migrations(&self) -> PersistenceResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS entity_chat_session (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                title TEXT,
                metadata TEXT,
                create_time TEXT NOT NULL,
                update_time TEXT NOT NULL
            )
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS entity_llm_message (
                id TEXT PRIMARY KEY,
                chat_session_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                tenant_id TEXT,
                parent_message_id TEXT,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                create_time TEXT NOT NULL,
                update_time TEXT NOT NULL
            )
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS entity_llm_api_call (
                id TEXT PRIMARY KEY,
                chat_session_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                request_message_id TEXT NOT NULL,
                response_message_id TEXT NOT NULL,
                model_name TEXT NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                error_code TEXT,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                prompt_tokens_details TEXT,
                completion_tokens_details TEXT,
                total_price REAL,
                price_details TEXT,
                latency_ms INTEGER,
                time_to_first_token_ms INTEGER,
                tokens_per_second REAL,
                api_response_id TEXT,
                request_time TEXT NOT NULL,
                response_time TEXT NOT NULL,
                create_time TEXT NOT NULL
            )
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_message_session ON entity_llm_message(chat_session_id)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::Query(e.to_string()))?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_api_call_session ON entity_llm_api_call(chat_session_id)")
            .execute(&self.pool).await.map_err(|e| PersistenceError::Query(e.to_string()))?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_session_user ON entity_chat_session(user_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(())
    }

    fn parse_message_row(row: &SqliteRow) -> PersistenceResult<LLMMessage> {
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

        let create_time_str: String = row
            .try_get("create_time")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let create_time = chrono::DateTime::parse_from_rfc3339(&create_time_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&chrono::Utc);

        let update_time_str: String = row
            .try_get("update_time")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let update_time = chrono::DateTime::parse_from_rfc3339(&update_time_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&chrono::Utc);

        Ok(LLMMessage {
            id,
            chat_session_id,
            agent_id,
            user_id,
            tenant_id,
            parent_message_id,
            role,
            content,
            create_time,
            update_time,
        })
    }

    fn parse_api_call_row(row: &SqliteRow) -> PersistenceResult<LLMApiCall> {
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

        let request_time_str: String = row
            .try_get("request_time")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let request_time = chrono::DateTime::parse_from_rfc3339(&request_time_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&chrono::Utc);

        let response_time_str: String = row
            .try_get("response_time")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let response_time = chrono::DateTime::parse_from_rfc3339(&response_time_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&chrono::Utc);

        let create_time_str: String = row
            .try_get("create_time")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let create_time = chrono::DateTime::parse_from_rfc3339(&create_time_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&chrono::Utc);

        Ok(LLMApiCall {
            id,
            chat_session_id,
            agent_id,
            user_id,
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
            request_time,
            response_time,
            create_time,
        })
    }

    fn parse_session_row(row: &SqliteRow) -> PersistenceResult<ChatSession> {
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

        let metadata: HashMap<String, serde_json::Value> = row
            .try_get::<Option<String>, _>("metadata")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let create_time_str: String = row
            .try_get("create_time")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let create_time = chrono::DateTime::parse_from_rfc3339(&create_time_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&chrono::Utc);

        let update_time_str: String = row
            .try_get("update_time")
            .map_err(|e| PersistenceError::Query(e.to_string()))?;
        let update_time = chrono::DateTime::parse_from_rfc3339(&update_time_str)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&chrono::Utc);

        Ok(ChatSession {
            id,
            user_id,
            agent_id,
            title: row.try_get("title").ok(),
            metadata,
            create_time,
            update_time,
        })
    }
}

#[async_trait]
impl MessageStore for SqliteStore {
    async fn save_message(&self, message: &LLMMessage) -> PersistenceResult<()> {
        let content_json = serde_json::to_string(&message.content)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        sqlx::query(r#"
            INSERT OR REPLACE INTO entity_llm_message
            (id, chat_session_id, agent_id, user_id, tenant_id, parent_message_id, role, content, create_time, update_time)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#)
        .bind(message.id.to_string())
        .bind(message.chat_session_id.to_string())
        .bind(message.agent_id.to_string())
        .bind(message.user_id.to_string())
        .bind(message.tenant_id.to_string())
        .bind(message.parent_message_id.map(|u| u.to_string()))
        .bind(message.role.to_string())
        .bind(content_json)
        .bind(message.create_time.to_rfc3339())
        .bind(message.update_time.to_rfc3339())
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
impl ApiCallStore for SqliteStore {
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

        sqlx::query(r#"
            INSERT OR REPLACE INTO entity_llm_api_call
            (id, chat_session_id, agent_id, user_id, request_message_id, response_message_id,
             model_name, status, error_message, error_code, prompt_tokens, completion_tokens, total_tokens,
             prompt_tokens_details, completion_tokens_details, total_price, price_details,
             latency_ms, time_to_first_token_ms, tokens_per_second, api_response_id,
             request_time, response_time, create_time)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#)
        .bind(call.id.to_string())
        .bind(call.chat_session_id.to_string())
        .bind(call.agent_id.to_string())
        .bind(call.user_id.to_string())
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
        .bind(call.request_time.to_rfc3339())
        .bind(call.response_time.to_rfc3339())
        .bind(call.create_time.to_rfc3339())
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
            .bind(before.to_rfc3339())
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::Query(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn create_session(&self, session: &ChatSession) -> PersistenceResult<()> {
        let metadata = serde_json::to_string(&session.metadata)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        sqlx::query(r#"
            INSERT INTO entity_chat_session (id, user_id, agent_id, title, metadata, create_time, update_time)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        "#)
        .bind(session.id.to_string())
        .bind(session.user_id.to_string())
        .bind(session.agent_id.to_string())
        .bind(&session.title)
        .bind(metadata)
        .bind(session.create_time.to_rfc3339())
        .bind(session.update_time.to_rfc3339())
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
        .bind(session.update_time.to_rfc3339())
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

impl PersistenceStore for SqliteStore {
    fn backend_name(&self) -> &str {
        "sqlite"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_store_messages() {
        let store = SqliteStore::in_memory().await.unwrap();
        let user_id = Uuid::now_v7();
        let agent_id = Uuid::now_v7();

        let session = ChatSession::new(user_id, agent_id);
        store.create_session(&session).await.unwrap();

        let msg1 = LLMMessage::new(
            session.id,
            agent_id,
            user_id,
            MessageRole::User,
            MessageContent::text("Hello"),
        );
        store.save_message(&msg1).await.unwrap();

        let msg2 = LLMMessage::new(
            session.id,
            agent_id,
            user_id,
            MessageRole::Assistant,
            MessageContent::text("Hi!"),
        );
        store.save_message(&msg2).await.unwrap();

        let messages = store.get_session_messages(session.id).await.unwrap();
        assert_eq!(messages.len(), 2);

        let count = store.count_session_messages(session.id).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_sqlite_store_api_calls() {
        let store = SqliteStore::in_memory().await.unwrap();
        let user_id = Uuid::now_v7();
        let agent_id = Uuid::now_v7();
        let session_id = Uuid::now_v7();

        let now = chrono::Utc::now();
        let call = LLMApiCall::success(
            session_id,
            agent_id,
            user_id,
            Uuid::now_v7(),
            Uuid::now_v7(),
            "gpt-4",
            100,
            50,
            now - chrono::Duration::seconds(1),
            now,
        );

        store.save_api_call(&call).await.unwrap();

        let filter = QueryFilter::new().user(user_id);
        let calls = store.query_api_calls(&filter).await.unwrap();
        assert_eq!(calls.len(), 1);

        let stats = store.get_statistics(&filter).await.unwrap();
        assert_eq!(stats.total_calls, 1);
        assert_eq!(stats.total_tokens, 150);
    }
}
