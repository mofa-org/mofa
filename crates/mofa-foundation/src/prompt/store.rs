//! Prompt 持久化存储
//! Prompt Persistence Storage
//!
//! 提供 Prompt 模板的数据库存储支持
//! Provides database storage support for Prompt templates

use super::template::{
    PromptComposition, PromptError, PromptResult, PromptTemplate, PromptVariable,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Prompt 模板数据库实体
/// Prompt template database entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEntity {
    /// 唯一 ID
    /// Unique ID
    pub id: Uuid,
    /// 模板标识符（用于查找）
    /// Template identifier (used for lookup)
    pub template_id: String,
    /// 模板名称
    /// Template name
    pub name: Option<String>,
    /// 模板描述
    /// Template description
    pub description: Option<String>,
    /// 模板内容
    /// Template content
    pub content: String,
    /// 变量定义（JSON）
    /// Variable definitions (JSON)
    pub variables: serde_json::Value,
    /// 标签列表
    /// Tag list
    pub tags: Vec<String>,
    /// 版本号
    /// Version number
    pub version: Option<String>,
    /// 元数据（JSON）
    /// Metadata (JSON)
    pub metadata: serde_json::Value,
    /// 是否启用
    /// Whether enabled
    pub enabled: bool,
    /// 创建时间
    /// Creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    /// Update time
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 创建者 ID
    /// Creator ID
    pub created_by: Option<Uuid>,
    /// 租户 ID（用于多租户隔离）
    /// Tenant ID (for multi-tenant isolation)
    pub tenant_id: Option<Uuid>,
}

impl PromptEntity {
    /// 从 PromptTemplate 创建实体
    /// Create entity from PromptTemplate
    pub fn from_template(template: &PromptTemplate) -> Self {
        let now = chrono::Utc::now();
        let variables = serde_json::to_value(&template.variables).unwrap_or_default();
        let metadata = serde_json::to_value(&template.metadata).unwrap_or_default();

        Self {
            id: Uuid::now_v7(),
            template_id: template.id.clone(),
            name: template.name.clone(),
            description: template.description.clone(),
            content: template.content.clone(),
            variables,
            tags: template.tags.clone(),
            version: template.version.clone(),
            metadata,
            enabled: true,
            created_at: now,
            updated_at: now,
            created_by: None,
            tenant_id: None,
        }
    }

    /// 转换为 PromptTemplate
    /// Convert to PromptTemplate
    pub fn to_template(&self) -> PromptResult<PromptTemplate> {
        let variables: Vec<PromptVariable> = serde_json::from_value(self.variables.clone())
            .map_err(|e| PromptError::ParseError(e.to_string()))?;
        let metadata: HashMap<String, String> =
            serde_json::from_value(self.metadata.clone()).unwrap_or_default();

        Ok(PromptTemplate {
            id: self.template_id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            content: self.content.clone(),
            variables,
            tags: self.tags.clone(),
            version: self.version.clone(),
            metadata,
        })
    }

    /// 设置创建者
    /// Set creator
    pub fn with_creator(mut self, creator_id: Uuid) -> Self {
        self.created_by = Some(creator_id);
        self
    }

    /// 设置租户
    /// Set tenant
    pub fn with_tenant(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }
}

/// Prompt 组合数据库实体
/// Prompt composition database entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCompositionEntity {
    /// 唯一 ID
    /// Unique ID
    pub id: Uuid,
    /// 组合标识符
    /// Composition identifier
    pub composition_id: String,
    /// 描述
    /// Description
    pub description: Option<String>,
    /// 模板 ID 列表
    /// Template ID list
    pub template_ids: Vec<String>,
    /// 分隔符
    /// Separator
    pub separator: String,
    /// 是否启用
    /// Whether enabled
    pub enabled: bool,
    /// 创建时间
    /// Creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    /// Update time
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 租户 ID
    /// Tenant ID
    pub tenant_id: Option<Uuid>,
}

impl PromptCompositionEntity {
    /// 从 PromptComposition 创建实体
    /// Create entity from PromptComposition
    pub fn from_composition(composition: &PromptComposition) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::now_v7(),
            composition_id: composition.id.clone(),
            description: composition.description.clone(),
            template_ids: composition.template_ids.clone(),
            separator: composition.separator.clone(),
            enabled: true,
            created_at: now,
            updated_at: now,
            tenant_id: None,
        }
    }

    /// 转换为 PromptComposition
    /// Convert to PromptComposition
    pub fn to_composition(&self) -> PromptComposition {
        PromptComposition {
            id: self.composition_id.clone(),
            description: self.description.clone(),
            template_ids: self.template_ids.clone(),
            separator: self.separator.clone(),
        }
    }
}

/// Prompt 查询过滤器
/// Prompt query filter
#[derive(Debug, Clone, Default)]
pub struct PromptFilter {
    /// 按模板 ID 查找
    /// Find by template ID
    pub template_id: Option<String>,
    /// 按标签查找
    /// Find by tags
    pub tags: Option<Vec<String>>,
    /// 搜索关键词（名称、描述）
    /// Search keywords (name, description)
    pub search: Option<String>,
    /// 只返回启用的
    /// Only return enabled
    pub enabled_only: bool,
    /// 租户 ID
    /// Tenant ID
    pub tenant_id: Option<Uuid>,
    /// 分页偏移
    /// Pagination offset
    pub offset: Option<i64>,
    /// 分页限制
    /// Pagination limit
    pub limit: Option<i64>,
}

impl PromptFilter {
    pub fn new() -> Self {
        Self {
            enabled_only: true,
            ..Default::default()
        }
    }

    pub fn template_id(mut self, id: impl Into<String>) -> Self {
        self.template_id = Some(id.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.get_or_insert_with(Vec::new).push(tag.into());
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn search(mut self, keyword: impl Into<String>) -> Self {
        self.search = Some(keyword.into());
        self
    }

    pub fn include_disabled(mut self) -> Self {
        self.enabled_only = false;
        self
    }

    pub fn tenant(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn paginate(mut self, offset: i64, limit: i64) -> Self {
        self.offset = Some(offset);
        self.limit = Some(limit);
        self
    }
}

/// Prompt 存储 trait
/// Prompt storage trait
///
/// 定义 Prompt 模板的 CRUD 操作
/// Defines CRUD operations for Prompt templates
#[async_trait]
pub trait PromptStore: Send + Sync {
    /// 保存模板
    /// Save template
    async fn save_template(&self, entity: &PromptEntity) -> PromptResult<()>;

    /// 批量保存模板
    /// Batch save templates
    async fn save_templates(&self, entities: &[PromptEntity]) -> PromptResult<()> {
        for entity in entities {
            self.save_template(entity).await?;
        }
        Ok(())
    }

    /// 获取模板（按 UUID）
    /// Get template (by UUID)
    async fn get_template_by_id(&self, id: Uuid) -> PromptResult<Option<PromptEntity>>;

    /// 获取模板（按模板 ID）
    /// Get template (by template ID)
    async fn get_template(&self, template_id: &str) -> PromptResult<Option<PromptEntity>>;

    /// 查询模板列表
    /// Query template list
    async fn query_templates(&self, filter: &PromptFilter) -> PromptResult<Vec<PromptEntity>>;

    /// 按标签查找模板
    /// Find templates by tag
    async fn find_by_tag(&self, tag: &str) -> PromptResult<Vec<PromptEntity>>;

    /// 搜索模板
    /// Search templates
    async fn search_templates(&self, keyword: &str) -> PromptResult<Vec<PromptEntity>>;

    /// 更新模板
    /// Update template
    async fn update_template(&self, entity: &PromptEntity) -> PromptResult<()>;

    /// 删除模板（按 UUID）
    /// Delete template (by UUID)
    async fn delete_template_by_id(&self, id: Uuid) -> PromptResult<bool>;

    /// 删除模板（按模板 ID）
    /// Delete template (by template ID)
    async fn delete_template(&self, template_id: &str) -> PromptResult<bool>;

    /// 启用/禁用模板
    /// Enable/disable template
    async fn set_template_enabled(&self, template_id: &str, enabled: bool) -> PromptResult<()>;

    /// 检查模板是否存在
    /// Check if template exists
    async fn exists(&self, template_id: &str) -> PromptResult<bool>;

    /// 统计模板数量
    /// Count template quantity
    async fn count(&self, filter: &PromptFilter) -> PromptResult<i64>;

    /// 获取所有标签
    /// Get all tags
    async fn get_all_tags(&self) -> PromptResult<Vec<String>>;

    // ========== 组合操作 ==========
    // ========== Composition Operations ==========

    /// 保存组合
    /// Save composition
    async fn save_composition(&self, entity: &PromptCompositionEntity) -> PromptResult<()>;

    /// 获取组合
    /// Get composition
    async fn get_composition(
        &self,
        composition_id: &str,
    ) -> PromptResult<Option<PromptCompositionEntity>>;

    /// 查询所有组合
    /// Query all compositions
    async fn query_compositions(&self) -> PromptResult<Vec<PromptCompositionEntity>>;

    /// 删除组合
    /// Delete composition
    async fn delete_composition(&self, composition_id: &str) -> PromptResult<bool>;
}

/// 动态分发的 PromptStore
/// Dynamically dispatched PromptStore
pub type DynPromptStore = std::sync::Arc<dyn PromptStore>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_entity_from_template() {
        let template = PromptTemplate::new("test")
            .with_name("Test Template")
            .with_content("Hello, {name}!")
            .with_tag("greeting");

        let entity = PromptEntity::from_template(&template);

        assert_eq!(entity.template_id, "test");
        assert_eq!(entity.name, Some("Test Template".to_string()));
        assert!(entity.enabled);
    }

    #[test]
    fn test_prompt_entity_to_template() {
        let template = PromptTemplate::new("test")
            .with_name("Test Template")
            .with_content("Hello, {name}!")
            .with_tag("greeting");

        let entity = PromptEntity::from_template(&template);
        let converted = entity.to_template().unwrap();

        assert_eq!(converted.id, template.id);
        assert_eq!(converted.name, template.name);
        assert_eq!(converted.content, template.content);
    }

    #[test]
    fn test_prompt_filter_builder() {
        let filter = PromptFilter::new()
            .with_tag("code")
            .search("review")
            .paginate(0, 10);

        assert_eq!(filter.tags, Some(vec!["code".to_string()]));
        assert_eq!(filter.search, Some("review".to_string()));
        assert_eq!(filter.limit, Some(10));
    }
}
