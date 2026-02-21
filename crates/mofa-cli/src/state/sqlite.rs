//! SQLite-based agent state store implementation

use super::store::{AgentRecord, AgentStatus, AgentStateStore};
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;

/// SQLite-based persistent agent state store
pub struct SqliteAgentStateStore {
    pool: SqlitePool,
}

impl SqliteAgentStateStore {
    /// Create a new SQLite agent state store
    pub async fn new<P: AsRef<Path>>(db_path: P) -> anyhow::Result<Self> {
        let db_url = format!("sqlite://{}", db_path.as_ref().display());

        // Create connection options
        let connect_options = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true);

        // Create pool
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await?;

        // Initialize database schema
        Self::init_schema(&pool).await?;

        Ok(Self { pool })
    }

    /// Initialize the database schema
    async fn init_schema(pool: &SqlitePool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at INTEGER,
                config_path TEXT,
                provider TEXT,
                model TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Convert database row to AgentRecord
    fn row_to_record(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<AgentRecord> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "running" => AgentStatus::Running,
            "stopped" => AgentStatus::Stopped,
            "paused" => AgentStatus::Paused,
            s if s.starts_with("error:") => {
                AgentStatus::Error(s.strip_prefix("error:").unwrap_or("").to_string())
            }
            _ => AgentStatus::Stopped,
        };

        Ok(AgentRecord {
            id: row.get("id"),
            name: row.get("name"),
            status,
            started_at: row.get("started_at"),
            config_path: row.get("config_path"),
            provider: row.get("provider"),
            model: row.get("model"),
        })
    }

    /// Convert status to database string
    fn status_to_string(status: &AgentStatus) -> String {
        match status {
            AgentStatus::Running => "running".to_string(),
            AgentStatus::Stopped => "stopped".to_string(),
            AgentStatus::Paused => "paused".to_string(),
            AgentStatus::Error(e) => format!("error:{}", e),
        }
    }
}

#[async_trait]
impl AgentStateStore for SqliteAgentStateStore {
    async fn list(&self) -> anyhow::Result<Vec<AgentRecord>> {
        let rows = sqlx::query("SELECT * FROM agents ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let records = rows
            .iter()
            .filter_map(|row| Self::row_to_record(row).ok())
            .collect();

        Ok(records)
    }

    async fn get(&self, agent_id: &str) -> anyhow::Result<Option<AgentRecord>> {
        let row = sqlx::query("SELECT * FROM agents WHERE id = ?")
            .bind(agent_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(Self::row_to_record(&row)?)),
            None => Ok(None),
        }
    }

    async fn create(&self, record: AgentRecord) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let status_str = Self::status_to_string(&record.status);

        sqlx::query(
            r#"
            INSERT INTO agents (id, name, status, started_at, config_path, provider, model, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&record.id)
        .bind(&record.name)
        .bind(status_str)
        .bind(record.started_at.map(|t| t as i64))
        .bind(&record.config_path)
        .bind(&record.provider)
        .bind(&record.model)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update(&self, record: AgentRecord) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let status_str = Self::status_to_string(&record.status);

        let result = sqlx::query(
            r#"
            UPDATE agents 
            SET name = ?, status = ?, started_at = ?, config_path = ?, provider = ?, model = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&record.name)
        .bind(status_str)
        .bind(record.started_at.map(|t| t as i64))
        .bind(&record.config_path)
        .bind(&record.provider)
        .bind(&record.model)
        .bind(now)
        .bind(&record.id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Agent '{}' not found", record.id));
        }

        Ok(())
    }

    async fn delete(&self, agent_id: &str) -> anyhow::Result<()> {
        let result = sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }

        Ok(())
    }
}
