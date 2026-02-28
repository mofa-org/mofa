//! 内存 Prompt 存储实现
//! In-memory Prompt storage implementation
//!
//! 提供基于内存的 Prompt 模板存储，适用于开发和测试
//! Provides memory-based storage for Prompt templates, suitable for development and testing

use super::store::{PromptCompositionEntity, PromptEntity, PromptFilter, PromptStore};
use super::template::{PromptError, PromptResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

/// 内存 Prompt 存储
/// In-memory Prompt store
///
/// 线程安全的内存存储实现，适用于：
/// Thread-safe memory storage implementation, suitable for:
/// - 开发和测试环境
/// - Development and testing environments
/// - 不需要持久化的场景
/// - Scenarios where persistence is not required
/// - 与预置模板库配合使用
/// - Working with preset template libraries
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::prompt::{InMemoryPromptStore, PromptEntity, PromptTemplate};
///
/// let store = InMemoryPromptStore::new();
///
/// // 保存模板
/// // Save template
/// let template = PromptTemplate::new("greeting")
///     .with_content("Hello, {name}!");
/// let entity = PromptEntity::from_template(&template);
/// store.save_template(&entity).await?;
///
/// // 查询模板
/// // Query template
/// let found = store.get_template("greeting").await?;
/// ```
pub struct InMemoryPromptStore {
    /// 模板存储 (UUID -> Entity)
    /// Template storage (UUID -> Entity)
    templates: RwLock<HashMap<Uuid, PromptEntity>>,
    /// 模板 ID 索引 (template_id -> UUID)
    /// Template ID index (template_id -> UUID)
    template_index: RwLock<HashMap<String, Uuid>>,
    /// 组合存储
    /// Composition storage
    compositions: RwLock<HashMap<String, PromptCompositionEntity>>,
}

impl Default for InMemoryPromptStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryPromptStore {
    /// 创建新的内存存储
    /// Create new memory storage
    pub fn new() -> Self {
        Self {
            templates: RwLock::new(HashMap::new()),
            template_index: RwLock::new(HashMap::new()),
            compositions: RwLock::new(HashMap::new()),
        }
    }

    /// 创建共享实例
    /// Create a shared instance
    pub fn shared() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::new())
    }

    /// 获取模板数量
    /// Get template count
    pub fn template_count(&self) -> usize {
        self.templates.read()
            .map(|t| t.len())
            .unwrap_or(0)
    }

    /// 清空所有数据
    /// Clear all data
    pub fn clear(&self) {
        let _ = self.templates.write().map(|mut t| t.clear());
        let _ = self.template_index.write().map(|mut i| i.clear());
        let _ = self.compositions.write().map(|mut c| c.clear());
    }
}

#[async_trait]
impl PromptStore for InMemoryPromptStore {
    async fn save_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        let mut templates = self.templates.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on templates: {}", e)))?;
        let mut index = self.template_index.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on index: {}", e)))?;

        // 如果已存在相同 template_id，删除旧的
        // If template_id already exists, remove the old one
        if let Some(&old_id) = index.get(&entity.template_id) {
            templates.remove(&old_id);
        }

        templates.insert(entity.id, entity.clone());
        index.insert(entity.template_id.clone(), entity.id);

        Ok(())
    }

    async fn get_template_by_id(&self, id: Uuid) -> PromptResult<Option<PromptEntity>> {
        let templates = self.templates.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on templates: {}", e)))?;
        Ok(templates.get(&id).cloned())
    }

    async fn get_template(&self, template_id: &str) -> PromptResult<Option<PromptEntity>> {
        let index = self.template_index.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on index: {}", e)))?;
        let templates = self.templates.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on templates: {}", e)))?;

        if let Some(&uuid) = index.get(template_id) {
            Ok(templates.get(&uuid).cloned())
        } else {
            Ok(None)
        }
    }

    async fn query_templates(&self, filter: &PromptFilter) -> PromptResult<Vec<PromptEntity>> {
        let templates = self.templates.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on templates: {}", e)))?;
        let mut results: Vec<PromptEntity> = templates
            .values()
            .filter(|e| {
                // 按启用状态过滤
                // Filter by enabled status
                if filter.enabled_only && !e.enabled {
                    return false;
                }

                // 按模板 ID 过滤
                // Filter by template ID
                if let Some(ref tid) = filter.template_id
                    && &e.template_id != tid
                {
                    return false;
                }

                // 按租户过滤
                // Filter by tenant
                if let Some(tenant_id) = filter.tenant_id
                    && e.tenant_id != Some(tenant_id)
                {
                    return false;
                }

                // 按标签过滤
                // Filter by tags
                if let Some(ref tags) = filter.tags
                    && !tags.iter().any(|t| e.tags.contains(t))
                {
                    return false;
                }

                // 按关键词搜索
                // Search by keywords
                if let Some(ref keyword) = filter.search {
                    let kw = keyword.to_lowercase();
                    let match_id = e.template_id.to_lowercase().contains(&kw);
                    let match_name = e
                        .name
                        .as_ref()
                        .is_some_and(|n: &String| n.to_lowercase().contains(&kw));
                    let match_desc = e
                        .description
                        .as_ref()
                        .is_some_and(|d: &String| d.to_lowercase().contains(&kw));

                    if !match_id && !match_name && !match_desc {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // 按更新时间排序
        // Sort by update time
        results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        // 分页
        // Pagination
        let offset = filter.offset.unwrap_or(0) as usize;
        let limit = filter.limit.unwrap_or(100) as usize;

        Ok(results.into_iter().skip(offset).take(limit).collect())
    }

    async fn find_by_tag(&self, tag: &str) -> PromptResult<Vec<PromptEntity>> {
        let filter = PromptFilter::new().with_tag(tag);
        self.query_templates(&filter).await
    }

    async fn search_templates(&self, keyword: &str) -> PromptResult<Vec<PromptEntity>> {
        let filter = PromptFilter::new().search(keyword);
        self.query_templates(&filter).await
    }

    async fn update_template(&self, entity: &PromptEntity) -> PromptResult<()> {
        let mut templates = self.templates.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on templates: {}", e)))?;
        let index = self.template_index.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on index: {}", e)))?;

        // 确保模板存在
        // Ensure template exists
        if let Some(&uuid) = index.get(&entity.template_id) {
            let mut updated = entity.clone();
            updated.id = uuid;
            updated.updated_at = chrono::Utc::now();
            templates.insert(uuid, updated);
        }

        Ok(())
    }

    async fn delete_template_by_id(&self, id: Uuid) -> PromptResult<bool> {
        let mut templates = self.templates.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on templates: {}", e)))?;
        let mut index = self.template_index.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on index: {}", e)))?;

        if let Some(entity) = templates.remove(&id) {
            index.remove(&entity.template_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn delete_template(&self, template_id: &str) -> PromptResult<bool> {
        let uuid = {
            let index = self.template_index.read()
                .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on index: {}", e)))?;
            index.get(template_id).copied()
        };
        if let Some(uuid) = uuid {
            self.delete_template_by_id(uuid).await
        } else {
            Ok(false)
        }
    }

    async fn set_template_enabled(&self, template_id: &str, enabled: bool) -> PromptResult<()> {
        let index = self.template_index.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on index: {}", e)))?;
        let mut templates = self.templates.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on templates: {}", e)))?;

        if let Some(&uuid) = index.get(template_id)
            && let Some(entity) = templates.get_mut(&uuid)
        {
            entity.enabled = enabled;
            entity.updated_at = chrono::Utc::now();
        }

        Ok(())
    }

    async fn exists(&self, template_id: &str) -> PromptResult<bool> {
        let index = self.template_index.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on index: {}", e)))?;
        Ok(index.contains_key(template_id))
    }

    async fn count(&self, filter: &PromptFilter) -> PromptResult<i64> {
        let results = self.query_templates(filter).await?;
        Ok(results.len() as i64)
    }

    async fn get_all_tags(&self) -> PromptResult<Vec<String>> {
        let templates = self.templates.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on templates: {}", e)))?;
        let mut tags: std::collections::HashSet<String> = std::collections::HashSet::new();

        for entity in templates.values() {
            for tag in &entity.tags {
                tags.insert(tag.clone());
            }
        }

        let mut result: Vec<String> = tags.into_iter().collect();
        result.sort();
        Ok(result)
    }

    async fn save_composition(&self, entity: &PromptCompositionEntity) -> PromptResult<()> {
        let mut compositions = self.compositions.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on compositions: {}", e)))?;
        compositions.insert(entity.composition_id.clone(), entity.clone());
        Ok(())
    }

    async fn get_composition(
        &self,
        composition_id: &str,
    ) -> PromptResult<Option<PromptCompositionEntity>> {
        let compositions = self.compositions.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on compositions: {}", e)))?;
        Ok(compositions.get(composition_id).cloned())
    }

    async fn query_compositions(&self) -> PromptResult<Vec<PromptCompositionEntity>> {
        let compositions = self.compositions.read()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire read lock on compositions: {}", e)))?;
        Ok(compositions.values().cloned().collect())
    }

    async fn delete_composition(&self, composition_id: &str) -> PromptResult<bool> {
        let mut compositions = self.compositions.write()
            .map_err(|e| PromptError::LockPoisoned(format!("Failed to acquire write lock on compositions: {}", e)))?;
        Ok(compositions.remove(composition_id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::store::PromptStore;
    use crate::prompt::template::PromptTemplate;

    #[tokio::test]
    async fn test_memory_store_basic() {
        let store = InMemoryPromptStore::new();

        let template = PromptTemplate::new("test")
            .with_name("Test Template")
            .with_content("Hello, {name}!")
            .with_tag("greeting");

        let entity = PromptEntity::from_template(&template);
        store.save_template(&entity).await.unwrap();

        assert!(store.exists("test").await.unwrap());
        assert_eq!(store.template_count(), 1);

        let found = store.get_template("test").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().template_id, "test");
    }

    #[tokio::test]
    async fn test_memory_store_query() {
        let store = InMemoryPromptStore::new();

        // 保存多个模板
        // Save multiple templates
        for i in 0..5 {
            let template = PromptTemplate::new(format!("template-{}", i))
                .with_name(format!("Template {}", i))
                .with_tag(if i % 2 == 0 { "even" } else { "odd" });

            store
                .save_template(&PromptEntity::from_template(&template))
                .await
                .unwrap();
        }

        // 按标签查询
        // Query by tag
        let even = store.find_by_tag("even").await.unwrap();
        assert_eq!(even.len(), 3);

        let odd = store.find_by_tag("odd").await.unwrap();
        assert_eq!(odd.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_store_search() {
        let store = InMemoryPromptStore::new();

        store
            .save_template(&PromptEntity::from_template(
                &PromptTemplate::new("code-review")
                    .with_name("Code Review")
                    .with_description("Review code for issues"),
            ))
            .await
            .unwrap();

        store
            .save_template(&PromptEntity::from_template(
                &PromptTemplate::new("code-explain")
                    .with_name("Code Explanation")
                    .with_description("Explain code in detail"),
            ))
            .await
            .unwrap();

        store
            .save_template(&PromptEntity::from_template(
                &PromptTemplate::new("chat").with_name("Chat Assistant"),
            ))
            .await
            .unwrap();

        // 搜索 "code"
        // Search "code"
        let results = store.search_templates("code").await.unwrap();
        assert_eq!(results.len(), 2);

        // 搜索 "review"
        // Search "review"
        let results = store.search_templates("review").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_store_delete() {
        let store = InMemoryPromptStore::new();

        let entity = PromptEntity::from_template(&PromptTemplate::new("test").with_content("test"));

        store.save_template(&entity).await.unwrap();
        assert!(store.exists("test").await.unwrap());

        store.delete_template("test").await.unwrap();
        assert!(!store.exists("test").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_store_enable_disable() {
        let store = InMemoryPromptStore::new();

        let entity = PromptEntity::from_template(&PromptTemplate::new("test").with_content("test"));

        store.save_template(&entity).await.unwrap();

        // 禁用
        // Disable
        store.set_template_enabled("test", false).await.unwrap();

        // 启用模式查询应该找不到
        // Enabled mode query should not find it
        let filter = PromptFilter::new();
        let results = store.query_templates(&filter).await.unwrap();
        assert_eq!(results.len(), 0);

        // 包含禁用的查询应该能找到
        // Query including disabled should find it
        let filter = PromptFilter::new().include_disabled();
        let results = store.query_templates(&filter).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_store_tags() {
        let store = InMemoryPromptStore::new();

        store
            .save_template(&PromptEntity::from_template(
                &PromptTemplate::new("t1").with_tag("a").with_tag("b"),
            ))
            .await
            .unwrap();

        store
            .save_template(&PromptEntity::from_template(
                &PromptTemplate::new("t2").with_tag("b").with_tag("c"),
            ))
            .await
            .unwrap();

        let tags = store.get_all_tags().await.unwrap();
        assert_eq!(tags, vec!["a", "b", "c"]);
    }
}
