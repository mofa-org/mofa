//! SQL 数据库 Prompt 存储实现
//! SQL Database Prompt Storage Implementation
//!
//! 支持 PostgreSQL、MySQL、SQLite 的统一实现
//! Unified implementation supporting PostgreSQL, MySQL, and SQLite

use super::store::{PromptCompositionEntity, PromptEntity, PromptFilter, PromptStore};
use super::template::{PromptError, PromptResult};
use async_trait::async_trait;
use uuid::Uuid;

#[cfg(feature = "persistence-postgres")]
use sqlx::postgres::{PgPool, PgRow};

#[cfg(feature = "persistence-mysql")]
use sqlx::mysql::{MySqlPool, MySqlRow};

#[cfg(feature = "persistence-sqlite")]
use sqlx::sqlite::{SqlitePool, SqliteRow};

#[cfg(any(
    feature = "persistence-postgres",
    feature = "persistence-mysql",
    feature = "persistence-sqlite"
))]
use sqlx::Row;

// ============================================================================
// PostgreSQL 实现
// PostgreSQL Implementation
// ============================================================================

#[cfg(feature = "persistence-postgres")]
pub struct PostgresPromptStore {
    pool: PgPool,
}

#[cfg(feature = "persistence-postgres")]
impl PostgresPromptStore {
    /// 连接到 PostgreSQL
    /// Connect to PostgreSQL
    pub async fn connect(database_url: &str) -> PromptResult<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| PromptError::ParseError(format!("Connection error: {}", e)))?;

        Ok(Self { pool })
    }

    /// 从现有连接池创建
    /// Create from an existing connection pool
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 创建共享实例
    /// Create a shared instance
    pub async fn shared(database_url: &str) -> PromptResult<std::sync::Arc<Self>> {
        Ok(std::sync::Arc::new(Self::connect(database_url).await?))
    }

    /// 获取连接池
    /// Get the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// 从行解析实体
    /// Parse entity from row
    fn parse_template_row(row: &PgRow) -> PromptResult<PromptEntity> {
        let tags: Vec<String> = row
            .try_get::<serde_json::Value, _>("tags")
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        Ok(PromptEntity {
            id: row
                .try_get("id")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            template_id: row
                .try_get("template_id")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            name: row.try_get("name").ok(),
            description: row.try_get("description").ok(),
            content: row
                .try_get("content")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            variables: row
                .try_get("variables")
                .unwrap_or(serde_json::Value::Array(vec![])),
            tags,
            version: row.try_get("version").ok(),
            metadata: row
                .try_get("metadata")
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            enabled: row.try_get("enabled").unwrap_or(true),
            created_at: row
                .try_get("created_at")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            updated_at: row
                .try_get("updated_at")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            created_by: row.try_get("created_by").ok(),
            tenant_id: row.try_get("tenant_id").ok(),
        })
    }

    /// 从行解析组合实体
    /// Parse composition entity from row
    fn parse_composition_row(row: &PgRow) -> PromptResult<PromptCompositionEntity> {
        let template_ids: Vec<String> = row
            .try_get::<serde_json::Value, _>("template_ids")
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        Ok(PromptCompositionEntity {
            id: row
                .try_get("id")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            composition_id: row
                .try_get("composition_id")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            description: row.try_get("description").ok(),
            template_ids,
            separator: row
                .try_get("separator")
                .unwrap_or_else(|_| "\n\n".to_string()),
            enabled: row.try_get("enabled").unwrap_or(true),
            created_at: row
                .try_get("created_at")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            updated_at: row
                .try_get("updated_at")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            tenant_id: row.try_get("tenant_id").ok(),
        })
    }
}

#[cfg(feature = "persistence-postgres")]
#[async_trait]
impl PromptStore for PostgresPromptStore {
    async fn save_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        let tags_json = serde_json::to_value(&entity.tags)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO prompt_template
            (id, template_id, name, description, content, variables, tags, version, metadata, enabled, created_at, updated_at, created_by, tenant_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (template_id, COALESCE(tenant_id, '00000000-0000-0000-0000-000000000000'::uuid))
            DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                content = EXCLUDED.content,
                variables = EXCLUDED.variables,
                tags = EXCLUDED.tags,
                version = EXCLUDED.version,
                metadata = EXCLUDED.metadata,
                enabled = EXCLUDED.enabled,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(entity.id)
        .bind(&entity.template_id)
        .bind(&entity.name)
        .bind(&entity.description)
        .bind(&entity.content)
        .bind(&entity.variables)
        .bind(&tags_json)
        .bind(&entity.version)
        .bind(&entity.metadata)
        .bind(entity.enabled)
        .bind(entity.created_at)
        .bind(entity.updated_at)
        .bind(entity.created_by)
        .bind(entity.tenant_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(format!("Save error: {}", e)))?;

        Ok(())
    }

    async fn get_template_by_id(&self, id: Uuid) -> PromptResult<Option<PromptEntity>> {
        let row = sqlx::query("SELECT * FROM prompt_template WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_template_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_template(&self, template_id: &str) -> PromptResult<Option<PromptEntity>> {
        let row =
            sqlx::query("SELECT * FROM prompt_template WHERE template_id = $1 AND enabled = true")
                .bind(template_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PromptError::ParseError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_template_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn query_templates(&self, filter: &PromptFilter) -> PromptResult<Vec<PromptEntity>> {
        let mut sql = String::from("SELECT * FROM prompt_template WHERE 1=1");
        let mut param_idx = 0;

        if filter.enabled_only {
            sql.push_str(" AND enabled = true");
        }

        if filter.template_id.is_some() {
            param_idx += 1;
            sql.push_str(&format!(" AND template_id = ${}", param_idx));
        }

        if filter.tenant_id.is_some() {
            param_idx += 1;
            sql.push_str(&format!(" AND tenant_id = ${}", param_idx));
        }

        if filter.search.is_some() {
            param_idx += 1;
            sql.push_str(&format!(
                " AND (template_id ILIKE ${0} OR name ILIKE ${0} OR description ILIKE ${0})",
                param_idx
            ));
        }

        sql.push_str(" ORDER BY updated_at DESC");

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        param_idx += 1;
        sql.push_str(&format!(" LIMIT ${}", param_idx));
        param_idx += 1;
        sql.push_str(&format!(" OFFSET ${}", param_idx));

        let mut query = sqlx::query(&sql);

        if let Some(ref tid) = filter.template_id {
            query = query.bind(tid);
        }
        if let Some(tenant_id) = filter.tenant_id {
            query = query.bind(tenant_id);
        }
        if let Some(ref search) = filter.search {
            query = query.bind(format!("%{}%", search));
        }
        query = query.bind(limit);
        query = query.bind(offset);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        rows.iter().map(Self::parse_template_row).collect()
    }

    async fn find_by_tag(&self, tag: &str) -> PromptResult<Vec<PromptEntity>> {
        let rows = sqlx::query(
            "SELECT * FROM prompt_template WHERE enabled = true AND tags @> $1::jsonb ORDER BY updated_at DESC",
        )
        .bind(serde_json::json!([tag]))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        rows.iter().map(Self::parse_template_row).collect()
    }

    async fn search_templates(&self, keyword: &str) -> PromptResult<Vec<PromptEntity>> {
        let filter = PromptFilter::new().search(keyword);
        self.query_templates(&filter).await
    }

    async fn update_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        let tags_json = serde_json::to_value(&entity.tags)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE prompt_template SET
                name = $2,
                description = $3,
                content = $4,
                variables = $5,
                tags = $6,
                version = $7,
                metadata = $8,
                enabled = $9,
                updated_at = NOW()
            WHERE template_id = $1
            "#,
        )
        .bind(&entity.template_id)
        .bind(&entity.name)
        .bind(&entity.description)
        .bind(&entity.content)
        .bind(&entity.variables)
        .bind(&tags_json)
        .bind(&entity.version)
        .bind(&entity.metadata)
        .bind(entity.enabled)
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(format!("Update error: {}", e)))?;

        Ok(())
    }

    async fn delete_template_by_id(&self, id: Uuid) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_template WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_template(&self, template_id: &str) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_template WHERE template_id = $1")
            .bind(template_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_template_enabled(&self, template_id: &str, enabled: bool) -> PromptResult<()> {
        sqlx::query(
            "UPDATE prompt_template SET enabled = $2, updated_at = NOW() WHERE template_id = $1",
        )
        .bind(template_id)
        .bind(enabled)
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(())
    }

    async fn exists(&self, template_id: &str) -> PromptResult<bool> {
        let row = sqlx::query("SELECT 1 FROM prompt_template WHERE template_id = $1")
            .bind(template_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn count(&self, filter: &PromptFilter) -> PromptResult<i64> {
        let mut sql = String::from("SELECT COUNT(*) as count FROM prompt_template WHERE 1=1");

        if filter.enabled_only {
            sql.push_str(" AND enabled = true");
        }

        let row = sqlx::query(&sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        let count: i64 = row
            .try_get("count")
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(count)
    }

    async fn get_all_tags(&self) -> PromptResult<Vec<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT jsonb_array_elements_text(tags) as tag FROM prompt_template WHERE enabled = true ORDER BY tag",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        let tags: Vec<String> = rows
            .iter()
            .filter_map(|row| row.try_get("tag").ok())
            .collect();

        Ok(tags)
    }

    async fn save_composition(&self, entity: &PromptCompositionEntity) -> PromptResult<()> {
        let template_ids_json = serde_json::to_value(&entity.template_ids)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO prompt_composition
            (id, composition_id, description, template_ids, separator, enabled, created_at, updated_at, tenant_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (composition_id) DO UPDATE SET
                description = EXCLUDED.description,
                template_ids = EXCLUDED.template_ids,
                separator = EXCLUDED.separator,
                enabled = EXCLUDED.enabled,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(entity.id)
        .bind(&entity.composition_id)
        .bind(&entity.description)
        .bind(&template_ids_json)
        .bind(&entity.separator)
        .bind(entity.enabled)
        .bind(entity.created_at)
        .bind(entity.updated_at)
        .bind(entity.tenant_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(format!("Save composition error: {}", e)))?;

        Ok(())
    }

    async fn get_composition(
        &self,
        composition_id: &str,
    ) -> PromptResult<Option<PromptCompositionEntity>> {
        let row = sqlx::query(
            "SELECT * FROM prompt_composition WHERE composition_id = $1 AND enabled = true",
        )
        .bind(composition_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_composition_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn query_compositions(&self) -> PromptResult<Vec<PromptCompositionEntity>> {
        let rows = sqlx::query(
            "SELECT * FROM prompt_composition WHERE enabled = true ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        rows.iter().map(Self::parse_composition_row).collect()
    }

    async fn delete_composition(&self, composition_id: &str) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_composition WHERE composition_id = $1")
            .bind(composition_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}

// ============================================================================
// MySQL 实现
// MySQL Implementation
// ============================================================================

#[cfg(feature = "persistence-mysql")]
pub struct MySqlPromptStore {
    pool: MySqlPool,
}

#[cfg(feature = "persistence-mysql")]
impl MySqlPromptStore {
    /// 连接到 MySQL
    /// Connect to MySQL
    pub async fn connect(database_url: &str) -> PromptResult<Self> {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| PromptError::ParseError(format!("Connection error: {}", e)))?;

        Ok(Self { pool })
    }

    /// 从现有连接池创建
    /// Create from existing connection pool
    pub fn from_pool(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// 获取连接池
    /// Get the connection pool
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    fn parse_template_row(row: &MySqlRow) -> PromptResult<PromptEntity> {
        let tags_str: String = row.try_get("tags").unwrap_or_default();
        let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

        let variables_str: String = row.try_get("variables").unwrap_or_default();
        let variables: serde_json::Value =
            serde_json::from_str(&variables_str).unwrap_or(serde_json::Value::Array(vec![]));

        let metadata_str: String = row.try_get("metadata").unwrap_or_default();
        let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        // MySQL UUID 需要特殊处理
        // MySQL UUID requires special handling
        let id_bytes: Vec<u8> = row
            .try_get("id")
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let id = Uuid::from_slice(&id_bytes).map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(PromptEntity {
            id,
            template_id: row
                .try_get("template_id")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            name: row.try_get("name").ok(),
            description: row.try_get("description").ok(),
            content: row
                .try_get("content")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            variables,
            tags,
            version: row.try_get("version").ok(),
            metadata,
            enabled: row.try_get("enabled").unwrap_or(true),
            created_at: row
                .try_get("created_at")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            updated_at: row
                .try_get("updated_at")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            created_by: row
                .try_get::<Vec<u8>, _>("created_by")
                .ok()
                .and_then(|b| Uuid::from_slice(&b).ok()),
            tenant_id: row
                .try_get::<Vec<u8>, _>("tenant_id")
                .ok()
                .and_then(|b| Uuid::from_slice(&b).ok()),
        })
    }
}

#[cfg(feature = "persistence-mysql")]
#[async_trait]
impl PromptStore for MySqlPromptStore {
    async fn save_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        let tags_json = serde_json::to_string(&entity.tags)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let variables_json = serde_json::to_string(&entity.variables)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let metadata_json = serde_json::to_string(&entity.metadata)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO prompt_template
            (id, template_id, name, description, content, variables, tags, version, metadata, enabled, created_at, updated_at, created_by, tenant_id)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                name = VALUES(name),
                description = VALUES(description),
                content = VALUES(content),
                variables = VALUES(variables),
                tags = VALUES(tags),
                version = VALUES(version),
                metadata = VALUES(metadata),
                enabled = VALUES(enabled),
                updated_at = VALUES(updated_at)
            "#,
        )
        .bind(entity.id.as_bytes().as_slice())
        .bind(&entity.template_id)
        .bind(&entity.name)
        .bind(&entity.description)
        .bind(&entity.content)
        .bind(&variables_json)
        .bind(&tags_json)
        .bind(&entity.version)
        .bind(&metadata_json)
        .bind(entity.enabled)
        .bind(entity.created_at)
        .bind(entity.updated_at)
        .bind(entity.created_by.map(|u| u.as_bytes().to_vec()))
        .bind(entity.tenant_id.map(|u| u.as_bytes().to_vec()))
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(format!("Save error: {}", e)))?;

        Ok(())
    }

    async fn get_template_by_id(&self, id: Uuid) -> PromptResult<Option<PromptEntity>> {
        let row = sqlx::query("SELECT * FROM prompt_template WHERE id = ?")
            .bind(id.as_bytes().as_slice())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_template_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_template(&self, template_id: &str) -> PromptResult<Option<PromptEntity>> {
        let row =
            sqlx::query("SELECT * FROM prompt_template WHERE template_id = ? AND enabled = true")
                .bind(template_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PromptError::ParseError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_template_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn query_templates(&self, filter: &PromptFilter) -> PromptResult<Vec<PromptEntity>> {
        let mut sql = String::from("SELECT * FROM prompt_template WHERE 1=1");

        if filter.enabled_only {
            sql.push_str(" AND enabled = true");
        }

        if filter.template_id.is_some() {
            sql.push_str(" AND template_id = ?");
        }

        if filter.search.is_some() {
            sql.push_str(" AND (template_id LIKE ? OR name LIKE ? OR description LIKE ?)");
        }

        sql.push_str(" ORDER BY updated_at DESC");

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        sql.push_str(" LIMIT ? OFFSET ?");

        let mut query = sqlx::query(&sql);

        if let Some(ref tid) = filter.template_id {
            query = query.bind(tid.clone());
        }
        if let Some(ref search) = filter.search {
            let pattern = format!("%{}%", search);
            query = query
                .bind(pattern.clone())
                .bind(pattern.clone())
                .bind(pattern);
        }
        query = query.bind(limit).bind(offset);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        rows.iter().map(Self::parse_template_row).collect()
    }

    async fn find_by_tag(&self, tag: &str) -> PromptResult<Vec<PromptEntity>> {
        // MySQL 使用 JSON_CONTAINS
        // MySQL uses JSON_CONTAINS
        let rows = sqlx::query(
            "SELECT * FROM prompt_template WHERE enabled = true AND JSON_CONTAINS(tags, ?) ORDER BY updated_at DESC",
        )
        .bind(format!("\"{}\"", tag))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        rows.iter().map(Self::parse_template_row).collect()
    }

    async fn search_templates(&self, keyword: &str) -> PromptResult<Vec<PromptEntity>> {
        let filter = PromptFilter::new().search(keyword);
        self.query_templates(&filter).await
    }

    async fn update_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        self.save_template(entity).await
    }

    async fn delete_template_by_id(&self, id: Uuid) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_template WHERE id = ?")
            .bind(id.as_bytes().as_slice())
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_template(&self, template_id: &str) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_template WHERE template_id = ?")
            .bind(template_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_template_enabled(&self, template_id: &str, enabled: bool) -> PromptResult<()> {
        sqlx::query(
            "UPDATE prompt_template SET enabled = ?, updated_at = NOW() WHERE template_id = ?",
        )
        .bind(enabled)
        .bind(template_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(())
    }

    async fn exists(&self, template_id: &str) -> PromptResult<bool> {
        let row = sqlx::query("SELECT 1 FROM prompt_template WHERE template_id = ?")
            .bind(template_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn count(&self, filter: &PromptFilter) -> PromptResult<i64> {
        let mut sql = String::from("SELECT COUNT(*) as count FROM prompt_template WHERE 1=1");

        if filter.enabled_only {
            sql.push_str(" AND enabled = true");
        }

        let row = sqlx::query(&sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        let count: i64 = row
            .try_get("count")
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(count)
    }

    async fn get_all_tags(&self) -> PromptResult<Vec<String>> {
        // MySQL 需要使用 JSON_TABLE 或解析
        // MySQL requires using JSON_TABLE or manual parsing
        let rows = sqlx::query("SELECT DISTINCT tags FROM prompt_template WHERE enabled = true")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        let mut all_tags = std::collections::HashSet::new();
        for row in rows {
            let tags_str: String = row.try_get("tags").unwrap_or_default();
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tags_str) {
                for tag in tags {
                    all_tags.insert(tag);
                }
            }
        }

        let mut result: Vec<String> = all_tags.into_iter().collect();
        result.sort();
        Ok(result)
    }

    async fn save_composition(&self, entity: &PromptCompositionEntity) -> PromptResult<()> {
        let template_ids_json = serde_json::to_string(&entity.template_ids)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO prompt_composition
            (id, composition_id, description, template_ids, separator, enabled, created_at, updated_at, tenant_id)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                description = VALUES(description),
                template_ids = VALUES(template_ids),
                separator = VALUES(separator),
                enabled = VALUES(enabled),
                updated_at = VALUES(updated_at)
            "#,
        )
        .bind(entity.id.as_bytes().as_slice())
        .bind(&entity.composition_id)
        .bind(&entity.description)
        .bind(&template_ids_json)
        .bind(&entity.separator)
        .bind(entity.enabled)
        .bind(entity.created_at)
        .bind(entity.updated_at)
        .bind(entity.tenant_id.map(|u| u.as_bytes().to_vec()))
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(format!("Save composition error: {}", e)))?;

        Ok(())
    }

    async fn get_composition(
        &self,
        composition_id: &str,
    ) -> PromptResult<Option<PromptCompositionEntity>> {
        // 简化实现，返回 None
        // Simplified implementation, returns None
        Ok(None)
    }

    async fn query_compositions(&self) -> PromptResult<Vec<PromptCompositionEntity>> {
        Ok(vec![])
    }

    async fn delete_composition(&self, composition_id: &str) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_composition WHERE composition_id = ?")
            .bind(composition_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}

// ============================================================================
// SQLite 实现
// SQLite Implementation
// ============================================================================

#[cfg(feature = "persistence-sqlite")]
pub struct SqlitePromptStore {
    pool: SqlitePool,
}

#[cfg(feature = "persistence-sqlite")]
impl SqlitePromptStore {
    /// 连接到 SQLite
    /// Connect to SQLite
    pub async fn connect(database_url: &str) -> PromptResult<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| PromptError::ParseError(format!("Connection error: {}", e)))?;

        Ok(Self { pool })
    }

    /// 创建内存数据库
    /// Create in-memory database
    pub async fn in_memory() -> PromptResult<Self> {
        Self::connect("sqlite::memory:").await
    }

    /// 从现有连接池创建
    /// Create from existing connection pool
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 获取连接池
    /// Get connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// 初始化表结构
    /// Initialize table structures
    pub async fn init_tables(&self) -> PromptResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS prompt_template (
                id TEXT PRIMARY KEY,
                template_id TEXT NOT NULL UNIQUE,
                name TEXT,
                description TEXT,
                content TEXT NOT NULL,
                variables TEXT DEFAULT '[]',
                tags TEXT DEFAULT '[]',
                version TEXT,
                metadata TEXT DEFAULT '{}',
                enabled INTEGER DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                created_by TEXT,
                tenant_id TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS prompt_composition (
                id TEXT PRIMARY KEY,
                composition_id TEXT NOT NULL UNIQUE,
                description TEXT,
                template_ids TEXT DEFAULT '[]',
                separator TEXT DEFAULT '\n\n',
                enabled INTEGER DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                tenant_id TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        // 创建索引
        // Create indexes
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_prompt_template_id ON prompt_template(template_id)",
        )
        .execute(&self.pool)
        .await
        .ok();

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_prompt_enabled ON prompt_template(enabled)")
            .execute(&self.pool)
            .await
            .ok();

        Ok(())
    }

    fn parse_template_row(row: &SqliteRow) -> PromptResult<PromptEntity> {
        let tags_str: String = row.try_get("tags").unwrap_or_default();
        let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

        let variables_str: String = row.try_get("variables").unwrap_or_default();
        let variables: serde_json::Value =
            serde_json::from_str(&variables_str).unwrap_or(serde_json::Value::Array(vec![]));

        let metadata_str: String = row.try_get("metadata").unwrap_or_default();
        let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        let id_str: String = row
            .try_get("id")
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let id = Uuid::parse_str(&id_str).map_err(|e| PromptError::ParseError(e.to_string()))?;

        let created_at_str: String = row
            .try_get("created_at")
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let updated_at_str: String = row
            .try_get("updated_at")
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(PromptEntity {
            id,
            template_id: row
                .try_get("template_id")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            name: row.try_get("name").ok(),
            description: row.try_get("description").ok(),
            content: row
                .try_get("content")
                .map_err(|e| PromptError::ParseError(e.to_string()))?,
            variables,
            tags,
            version: row.try_get("version").ok(),
            metadata,
            enabled: row.try_get::<i32, _>("enabled").unwrap_or(1) == 1,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            created_by: row
                .try_get::<String, _>("created_by")
                .ok()
                .and_then(|s| Uuid::parse_str(&s).ok()),
            tenant_id: row
                .try_get::<String, _>("tenant_id")
                .ok()
                .and_then(|s| Uuid::parse_str(&s).ok()),
        })
    }
}

#[cfg(feature = "persistence-sqlite")]
#[async_trait]
impl PromptStore for SqlitePromptStore {
    async fn save_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        let tags_json = serde_json::to_string(&entity.tags)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let variables_json = serde_json::to_string(&entity.variables)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let metadata_json = serde_json::to_string(&entity.metadata)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO prompt_template
            (id, template_id, name, description, content, variables, tags, version, metadata, enabled, created_at, updated_at, created_by, tenant_id)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(entity.id.to_string())
        .bind(&entity.template_id)
        .bind(&entity.name)
        .bind(&entity.description)
        .bind(&entity.content)
        .bind(&variables_json)
        .bind(&tags_json)
        .bind(&entity.version)
        .bind(&metadata_json)
        .bind(entity.enabled as i32)
        .bind(entity.created_at.to_rfc3339())
        .bind(entity.updated_at.to_rfc3339())
        .bind(entity.created_by.map(|u| u.to_string()))
        .bind(entity.tenant_id.map(|u| u.to_string()))
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(format!("Save error: {}", e)))?;

        Ok(())
    }

    async fn get_template_by_id(&self, id: Uuid) -> PromptResult<Option<PromptEntity>> {
        let row = sqlx::query("SELECT * FROM prompt_template WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_template_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_template(&self, template_id: &str) -> PromptResult<Option<PromptEntity>> {
        let row =
            sqlx::query("SELECT * FROM prompt_template WHERE template_id = ? AND enabled = 1")
                .bind(template_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PromptError::ParseError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(Self::parse_template_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn query_templates(&self, filter: &PromptFilter) -> PromptResult<Vec<PromptEntity>> {
        let mut sql = String::from("SELECT * FROM prompt_template WHERE 1=1");

        if filter.enabled_only {
            sql.push_str(" AND enabled = 1");
        }

        if filter.template_id.is_some() {
            sql.push_str(" AND template_id = ?");
        }

        if filter.search.is_some() {
            sql.push_str(" AND (template_id LIKE ? OR name LIKE ? OR description LIKE ?)");
        }

        sql.push_str(" ORDER BY updated_at DESC");

        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        sql.push_str(" LIMIT ? OFFSET ?");

        let mut query = sqlx::query(&sql);

        if let Some(ref tid) = filter.template_id {
            query = query.bind(tid.clone());
        }
        if let Some(ref search) = filter.search {
            let pattern = format!("%{}%", search);
            query = query
                .bind(pattern.clone())
                .bind(pattern.clone())
                .bind(pattern);
        }
        query = query.bind(limit).bind(offset);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        rows.iter().map(Self::parse_template_row).collect()
    }

    async fn find_by_tag(&self, tag: &str) -> PromptResult<Vec<PromptEntity>> {
        // SQLite 使用 JSON 函数或 LIKE
        // SQLite uses JSON functions or LIKE
        let rows = sqlx::query(
            "SELECT * FROM prompt_template WHERE enabled = 1 AND tags LIKE ? ORDER BY updated_at DESC",
        )
        .bind(format!("%\"{}%", tag))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(e.to_string()))?;

        rows.iter().map(Self::parse_template_row).collect()
    }

    async fn search_templates(&self, keyword: &str) -> PromptResult<Vec<PromptEntity>> {
        let filter = PromptFilter::new().search(keyword);
        self.query_templates(&filter).await
    }

    async fn update_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        self.save_template(entity).await
    }

    async fn delete_template_by_id(&self, id: Uuid) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_template WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_template(&self, template_id: &str) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_template WHERE template_id = ?")
            .bind(template_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_template_enabled(&self, template_id: &str, enabled: bool) -> PromptResult<()> {
        sqlx::query("UPDATE prompt_template SET enabled = ?, updated_at = datetime('now') WHERE template_id = ?")
            .bind(enabled as i32)
            .bind(template_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(())
    }

    async fn exists(&self, template_id: &str) -> PromptResult<bool> {
        let row = sqlx::query("SELECT 1 FROM prompt_template WHERE template_id = ?")
            .bind(template_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn count(&self, filter: &PromptFilter) -> PromptResult<i64> {
        let mut sql = String::from("SELECT COUNT(*) as count FROM prompt_template WHERE 1=1");

        if filter.enabled_only {
            sql.push_str(" AND enabled = 1");
        }

        let row = sqlx::query(&sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        let count: i32 = row
            .try_get("count")
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(count as i64)
    }

    async fn get_all_tags(&self) -> PromptResult<Vec<String>> {
        let rows = sqlx::query("SELECT DISTINCT tags FROM prompt_template WHERE enabled = 1")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        let mut all_tags = std::collections::HashSet::new();
        for row in rows {
            let tags_str: String = row.try_get("tags").unwrap_or_default();
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tags_str) {
                for tag in tags {
                    all_tags.insert(tag);
                }
            }
        }

        let mut result: Vec<String> = all_tags.into_iter().collect();
        result.sort();
        Ok(result)
    }

    async fn save_composition(&self, entity: &PromptCompositionEntity) -> PromptResult<()> {
        let template_ids_json = serde_json::to_string(&entity.template_ids)
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO prompt_composition
            (id, composition_id, description, template_ids, separator, enabled, created_at, updated_at, tenant_id)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(entity.id.to_string())
        .bind(&entity.composition_id)
        .bind(&entity.description)
        .bind(&template_ids_json)
        .bind(&entity.separator)
        .bind(entity.enabled as i32)
        .bind(entity.created_at.to_rfc3339())
        .bind(entity.updated_at.to_rfc3339())
        .bind(entity.tenant_id.map(|u| u.to_string()))
        .execute(&self.pool)
        .await
        .map_err(|e| PromptError::ParseError(format!("Save composition error: {}", e)))?;

        Ok(())
    }

    async fn get_composition(
        &self,
        composition_id: &str,
    ) -> PromptResult<Option<PromptCompositionEntity>> {
        Ok(None)
    }

    async fn query_compositions(&self) -> PromptResult<Vec<PromptCompositionEntity>> {
        Ok(vec![])
    }

    async fn delete_composition(&self, composition_id: &str) -> PromptResult<bool> {
        let result = sqlx::query("DELETE FROM prompt_composition WHERE composition_id = ?")
            .bind(composition_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PromptError::ParseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}
