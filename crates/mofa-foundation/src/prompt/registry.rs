//! Prompt 注册中心
//! Prompt Registry
//!
//! 提供全局和局部的 Prompt 模板管理
//! Provides global and local Prompt template management

use super::template::{PromptComposition, PromptError, PromptResult, PromptTemplate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Prompt 注册中心
/// Prompt Registry
///
/// 管理所有 Prompt 模板，支持注册、查询、删除和从文件加载
/// Manages all Prompt templates, supporting registration, query, deletion, and loading from files
#[derive(Default)]
pub struct PromptRegistry {
    /// 模板存储
    /// Template storage
    templates: HashMap<String, PromptTemplate>,
    /// 组合存储
    /// Composition storage
    compositions: HashMap<String, PromptComposition>,
    /// 分类索引 (tag -> template_ids)
    /// Category index (tag -> template_ids)
    tag_index: HashMap<String, Vec<String>>,
}

impl PromptRegistry {
    /// 创建新的注册中心
    /// Create a new registry
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册模板
    /// Register a template
    pub fn register(&mut self, template: PromptTemplate) {
        let id = template.id.clone();

        // 更新标签索引
        // Update tag index
        for tag in &template.tags {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .push(id.clone());
        }

        self.templates.insert(id, template);
    }

    /// 注册组合
    /// Register a composition
    pub fn register_composition(&mut self, composition: PromptComposition) {
        self.compositions
            .insert(composition.id.clone(), composition);
    }

    /// 获取模板
    /// Get a template
    pub fn get(&self, id: &str) -> PromptResult<&PromptTemplate> {
        self.templates
            .get(id)
            .ok_or_else(|| PromptError::TemplateNotFound(id.to_string()))
    }

    /// 获取可变模板引用
    /// Get a mutable template reference
    pub fn get_mut(&mut self, id: &str) -> PromptResult<&mut PromptTemplate> {
        self.templates
            .get_mut(id)
            .ok_or_else(|| PromptError::TemplateNotFound(id.to_string()))
    }

    /// 获取组合
    /// Get a composition
    pub fn get_composition(&self, id: &str) -> PromptResult<&PromptComposition> {
        self.compositions
            .get(id)
            .ok_or_else(|| PromptError::TemplateNotFound(format!("composition:{}", id)))
    }

    /// 检查模板是否存在
    /// Check if a template exists
    pub fn contains(&self, id: &str) -> bool {
        self.templates.contains_key(id)
    }

    /// 删除模板
    /// Remove a template
    pub fn remove(&mut self, id: &str) -> Option<PromptTemplate> {
        if let Some(template) = self.templates.remove(id) {
            // 清理标签索引
            // Clean up tag index
            for tag in &template.tags {
                if let Some(ids) = self.tag_index.get_mut(tag) {
                    ids.retain(|i| i != id);
                }
            }
            Some(template)
        } else {
            None
        }
    }

    /// 获取所有模板 ID
    /// Get all template IDs
    pub fn list_ids(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// 按标签查找模板
    /// Find templates by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<&PromptTemplate> {
        self.tag_index
            .get(tag)
            .map(|ids| ids.iter().filter_map(|id| self.templates.get(id)).collect())
            .unwrap_or_default()
    }

    /// 搜索模板（按名称或描述）
    /// Search templates (by name or description)
    pub fn search(&self, query: &str) -> Vec<&PromptTemplate> {
        let query_lower = query.to_lowercase();
        self.templates
            .values()
            .filter(|t| {
                t.id.to_lowercase().contains(&query_lower)
                    || t.name
                        .as_ref()
                        .is_some_and(|n| n.to_lowercase().contains(&query_lower))
                    || t.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// 获取所有标签
    /// List all tags
    pub fn list_tags(&self) -> Vec<&str> {
        self.tag_index.keys().map(|s| s.as_str()).collect()
    }

    /// 渲染模板
    /// Render a template
    pub fn render(&self, id: &str, vars: &[(&str, &str)]) -> PromptResult<String> {
        self.get(id)?.render(vars)
    }

    /// 渲染组合
    /// Render a composition
    pub fn render_composition(
        &self,
        composition_id: &str,
        vars: &[(&str, &str)],
    ) -> PromptResult<String> {
        let composition = self.get_composition(composition_id)?;
        let mut results = Vec::new();

        for template_id in &composition.template_ids {
            let rendered = self.render(template_id, vars)?;
            results.push(rendered);
        }

        Ok(results.join(&composition.separator))
    }

    /// 从 YAML 文件加载
    /// Load from a YAML file
    ///
    /// # YAML 格式
    /// # YAML Format
    ///
    /// ```yaml
    /// templates:
    ///   - id: greeting
    ///     name: Greeting Template
    ///     content: "Hello, {name}!"
    ///     description: A simple greeting
    ///     tags:
    ///       - basic
    ///       - greeting
    ///     variables:
    ///       - name: name
    ///         description: The person's name
    ///         required: true
    ///
    ///   - id: assistant
    ///     content: "You are a {role} assistant."
    ///     variables:
    ///       - name: role
    ///         default: helpful
    ///
    /// compositions:
    ///   - id: full-greeting
    ///     template_ids:
    ///       - greeting
    ///       - assistant
    ///     separator: "\n\n"
    /// ```
    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> PromptResult<()> {
        let content = std::fs::read_to_string(path)?;
        self.load_from_yaml(&content)
    }

    /// 从 YAML 字符串加载
    /// Load from a YAML string
    pub fn load_from_yaml(&mut self, yaml: &str) -> PromptResult<()> {
        let config: PromptYamlConfig =
            serde_yaml::from_str(yaml).map_err(|e| PromptError::YamlError(e.to_string()))?;

        // 加载模板
        // Load templates
        if let Some(templates) = config.templates {
            for template in templates {
                self.register(template);
            }
        }

        // 加载组合
        // Load compositions
        if let Some(compositions) = config.compositions {
            for composition in compositions {
                self.register_composition(composition);
            }
        }

        Ok(())
    }

    /// 导出为 YAML
    /// Export to YAML
    pub fn export_to_yaml(&self) -> PromptResult<String> {
        let config = PromptYamlConfig {
            templates: Some(self.templates.values().cloned().collect()),
            compositions: Some(self.compositions.values().cloned().collect()),
        };

        serde_yaml::to_string(&config).map_err(|e| PromptError::YamlError(e.to_string()))
    }

    /// 合并另一个注册中心
    /// Merge another registry
    pub fn merge(&mut self, other: PromptRegistry) {
        for (id, template) in other.templates {
            self.templates.insert(id, template);
        }
        for (id, composition) in other.compositions {
            self.compositions.insert(id, composition);
        }
        // 重建标签索引
        // Rebuild tag index
        self.rebuild_tag_index();
    }

    /// 重建标签索引
    /// Rebuild tag index
    fn rebuild_tag_index(&mut self) {
        self.tag_index.clear();
        for (id, template) in &self.templates {
            for tag in &template.tags {
                self.tag_index
                    .entry(tag.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
    }

    /// 模板数量
    /// Number of templates
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// 是否为空
    /// Whether it is empty
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// 清空所有模板
    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
        self.compositions.clear();
        self.tag_index.clear();
    }
}

/// YAML 配置结构
/// YAML configuration structure
#[derive(Debug, Serialize, Deserialize)]
struct PromptYamlConfig {
    #[serde(default)]
    templates: Option<Vec<PromptTemplate>>,
    #[serde(default)]
    compositions: Option<Vec<PromptComposition>>,
}

/// 线程安全的全局注册中心
/// Thread-safe global registry
#[derive(Clone, Default)]
pub struct GlobalPromptRegistry {
    inner: Arc<RwLock<PromptRegistry>>,
}

impl GlobalPromptRegistry {
    /// 创建新的全局注册中心
    /// Create a new global registry
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册模板
    /// Register a template
    pub fn register(&self, template: PromptTemplate) {
        self.inner.write()
            .expect("Failed to acquire write lock on prompt registry")
            .register(template);
    }

    /// 获取模板（克隆）
    /// Get a template (cloned)
    pub fn get(&self, id: &str) -> PromptResult<PromptTemplate> {
        self.inner.read()
            .expect("Failed to acquire read lock on prompt registry")
            .get(id).cloned()
    }

    /// 渲染模板
    /// Render a template
    pub fn render(&self, id: &str, vars: &[(&str, &str)]) -> PromptResult<String> {
        self.inner.read()
            .expect("Failed to acquire read lock on prompt registry")
            .render(id, vars)
    }

    /// 检查是否包含
    /// Check if it contains
    pub fn contains(&self, id: &str) -> bool {
        self.inner.read()
            .expect("Failed to acquire read lock on prompt registry")
            .contains(id)
    }

    /// 删除模板
    /// Remove a template
    pub fn remove(&self, id: &str) -> Option<PromptTemplate> {
        self.inner.write()
            .expect("Failed to acquire write lock on prompt registry")
            .remove(id)
    }

    /// 从文件加载
    /// Load from a file
    pub fn load_from_file(&self, path: impl AsRef<Path>) -> PromptResult<()> {
        self.inner.write()
            .expect("Failed to acquire write lock on prompt registry")
            .load_from_file(path)
    }

    /// 从 YAML 加载
    /// Load from YAML
    pub fn load_from_yaml(&self, yaml: &str) -> PromptResult<()> {
        self.inner.write()
            .expect("Failed to acquire write lock on prompt registry")
            .load_from_yaml(yaml)
    }

    /// 获取所有模板 ID
    /// Get all template IDs
    pub fn list_ids(&self) -> Vec<String> {
        self.inner
            .read()
            .unwrap()
            .list_ids()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// 按标签查找
    /// Find by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<PromptTemplate> {
        self.inner
            .read()
            .unwrap()
            .find_by_tag(tag)
            .iter()
            .map(|t| (*t).clone())
            .collect()
    }

    /// 搜索模板
    /// Search templates
    pub fn search(&self, query: &str) -> Vec<PromptTemplate> {
        self.inner
            .read()
            .unwrap()
            .search(query)
            .iter()
            .map(|t| (*t).clone())
            .collect()
    }

    /// 模板数量
    /// Number of templates
    pub fn len(&self) -> usize {
        self.inner.read()
            .expect("Failed to acquire read lock on prompt registry")
            .len()
    }

    /// 是否为空
    /// Whether it is empty
    pub fn is_empty(&self) -> bool {
        self.inner.read()
            .expect("Failed to acquire read lock on prompt registry")
            .is_empty()
    }

    /// 清空
    /// Clear
    pub fn clear(&self) {
        self.inner.write()
            .expect("Failed to acquire write lock on prompt registry")
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic() {
        let mut registry = PromptRegistry::new();

        let template = PromptTemplate::new("greeting")
            .with_content("Hello, {name}!")
            .with_tag("basic");

        registry.register(template);

        assert!(registry.contains("greeting"));
        assert_eq!(registry.len(), 1);

        let result = registry.render("greeting", &[("name", "World")]).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_registry_tags() {
        let mut registry = PromptRegistry::new();

        registry.register(
            PromptTemplate::new("t1")
                .with_content("Template 1")
                .with_tag("tag-a")
                .with_tag("tag-b"),
        );

        registry.register(
            PromptTemplate::new("t2")
                .with_content("Template 2")
                .with_tag("tag-a"),
        );

        registry.register(
            PromptTemplate::new("t3")
                .with_content("Template 3")
                .with_tag("tag-c"),
        );

        let tag_a_templates = registry.find_by_tag("tag-a");
        assert_eq!(tag_a_templates.len(), 2);

        let tag_b_templates = registry.find_by_tag("tag-b");
        assert_eq!(tag_b_templates.len(), 1);

        let tag_c_templates = registry.find_by_tag("tag-c");
        assert_eq!(tag_c_templates.len(), 1);
    }

    #[test]
    fn test_registry_search() {
        let mut registry = PromptRegistry::new();

        registry.register(
            PromptTemplate::new("code-review")
                .with_name("Code Review")
                .with_description("Review code for issues"),
        );

        registry.register(
            PromptTemplate::new("code-explain")
                .with_name("Code Explanation")
                .with_description("Explain code in detail"),
        );

        registry.register(
            PromptTemplate::new("chat")
                .with_name("Chat Assistant")
                .with_description("General chat"),
        );

        let code_templates = registry.search("code");
        assert_eq!(code_templates.len(), 2);

        let review_templates = registry.search("review");
        assert_eq!(review_templates.len(), 1);
    }

    #[test]
    fn test_registry_yaml() {
        let yaml = r#"
templates:
  - id: greeting
    name: Greeting
    content: "Hello, {name}!"
    tags:
      - basic
    variables:
      - name: name
        required: true

  - id: farewell
    content: "Goodbye, {name}!"
    variables:
      - name: name
        default: friend

compositions:
  - id: full-conversation
    template_ids:
      - greeting
      - farewell
    separator: "\n"
"#;

        let mut registry = PromptRegistry::new();
        registry.load_from_yaml(yaml).unwrap();

        assert_eq!(registry.len(), 2);
        assert!(registry.contains("greeting"));
        assert!(registry.contains("farewell"));

        // 测试渲染
        // Test rendering
        let greeting = registry.render("greeting", &[("name", "Alice")]).unwrap();
        assert_eq!(greeting, "Hello, Alice!");

        // 测试默认值
        // Test default values
        let farewell = registry.render("farewell", &[]).unwrap();
        assert_eq!(farewell, "Goodbye, friend!");

        // 测试组合
        // Test composition
        let composition = registry
            .render_composition("full-conversation", &[("name", "Bob")])
            .unwrap();
        assert_eq!(composition, "Hello, Bob!\nGoodbye, Bob!");
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = PromptRegistry::new();

        registry.register(
            PromptTemplate::new("test")
                .with_content("Test")
                .with_tag("removable"),
        );

        assert!(registry.contains("test"));
        assert_eq!(registry.find_by_tag("removable").len(), 1);

        let removed = registry.remove("test");
        assert!(removed.is_some());
        assert!(!registry.contains("test"));
        assert_eq!(registry.find_by_tag("removable").len(), 0);
    }

    #[test]
    fn test_global_registry() {
        let registry = GlobalPromptRegistry::new();

        registry.register(PromptTemplate::new("test").with_content("Hello, {name}!"));

        assert!(registry.contains("test"));

        let result = registry.render("test", &[("name", "World")]).unwrap();
        assert_eq!(result, "Hello, World!");
    }
}
