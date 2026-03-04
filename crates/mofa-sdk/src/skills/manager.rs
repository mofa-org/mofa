//! Skills Manager - SDK 层统一 API
//! Skills Manager - SDK Layer Unified API

use super::{DisclosureController, RequirementCheck, SkillMetadata};
use std::path::{Path, PathBuf};
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

/// Skills Manager - SDK 层统一 API
/// Skills Manager - SDK Layer Unified API
///
/// 提供 Skills 的管理和查询接口，支持渐进式披露、多目录搜索和依赖检查。
/// Provides management and query interfaces for Skills, supporting progressive disclosure, multi-directory search, and dependency checks.
#[derive(Debug, Clone)]
pub struct SkillsManager {
    controller: DisclosureController,
}

impl SkillsManager {
    /// 创建新的 Skills Manager（单目录）
    /// Create a new Skills Manager (single directory)
    ///
    /// # Arguments
    ///
    /// * `skills_dir` - Skills 目录路径
    /// * `skills_dir` - Path to the skills directory
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mofa_sdk::skills::SkillsManager;
    ///
    /// let manager = SkillsManager::new("./skills").unwrap();
    /// ```
    pub fn new(skills_dir: impl AsRef<Path>) -> GlobalResult<Self> {
        let skills_dir = skills_dir.as_ref();

        // 不要求目录必须存在（支持空目录）
        // Directory existence not required (supports empty directories)
        let mut controller = DisclosureController::new(skills_dir);
        if skills_dir.exists() {
            controller.scan_metadata().map_err(|e| GlobalError::Other(e.to_string()))?;
        }

        Ok(Self { controller })
    }

    /// 创建支持多目录搜索的 Skills Manager
    /// Create a Skills Manager supporting multi-directory search
    ///
    /// # Arguments
    ///
    /// * `search_dirs` - Skills 目录列表，按优先级排序
    /// * `search_dirs` - List of skill directories, ordered by priority
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mofa_sdk::skills::SkillsManager;
    /// use std::path::PathBuf;
    ///
    /// let workspace_skills = PathBuf::from("./workspace/skills");
    /// let builtin_skills = SkillsManager::find_builtin_skills();
    ///
    /// if let Some(builtin) = builtin_skills {
    ///     let manager = SkillsManager::with_search_dirs(vec![workspace_skills, builtin]).unwrap();
    /// }
    /// ```
    pub fn with_search_dirs(search_dirs: Vec<PathBuf>) -> GlobalResult<Self> {
        let controller = DisclosureController::with_search_dirs(search_dirs);
        let mut manager = Self { controller };
        manager.rescan()?;
        Ok(manager)
    }

    /// 查找内置 skills 目录
    /// Find the built-in skills directory
    ///
    /// 按以下顺序查找：
    /// Search in the following order:
    /// 1. CARGO_MANIFEST_DIR/skills（开发时）
    /// 1. CARGO_MANIFEST_DIR/skills (during development)
    /// 2. 可执行文件父目录/skills（已安装）
    /// 2. Executable parent directory/skills (when installed)
    /// 3. /usr/local/lib/mofa/skills（标准安装路径）
    /// 3. /usr/local/lib/mofa/skills (standard installation path)
    pub fn find_builtin_skills() -> Option<PathBuf> {
        DisclosureController::find_builtin_skills()
    }

    /// 获取系统提示（第1层：仅元数据）
    /// Get system prompt (Layer 1: Metadata only)
    ///
    /// 返回包含所有 Skills 元数据的系统提示字符串。
    /// Returns a system prompt string containing all Skills metadata.
    pub fn build_system_prompt(&self) -> String {
        self.controller.build_system_prompt()
    }

    /// 获取系统提示（异步版本）
    /// Get system prompt (Async version)
    pub async fn build_system_prompt_async(&self) -> String {
        self.build_system_prompt()
    }

    /// 加载 Skill 的 SKILL.md 内容（第2层）
    /// Load SKILL.md content for a Skill (Layer 2)
    ///
    /// # Arguments
    ///
    /// * `name` - Skill 名称
    /// * `name` - Skill name
    ///
    /// # Returns
    ///
    /// 返回 SKILL.md 的 Markdown 内容（去除 frontmatter）
    /// Returns Markdown content of SKILL.md (frontmatter removed)
    pub fn load_skill(&self, name: &str) -> Option<String> {
        let skill_path = self.controller.get_skill_path(name)?;
        let skill_md = skill_path.join("SKILL.md");

        let content = std::fs::read_to_string(&skill_md).ok()?;

        // 去除 frontmatter，返回纯 Markdown
        // Remove frontmatter and return pure Markdown
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            Some(parts[2].trim().to_string())
        } else {
            Some(content)
        }
    }

    /// 加载 Skill 的 SKILL.md 内容（异步版本）
    /// Load SKILL.md content for a Skill (Async version)
    pub async fn load_skill_async(&self, name: &str) -> Option<String> {
        let skill_path = self.controller.get_skill_path(name)?;
        let skill_md = skill_path.join("SKILL.md");

        let content = tokio::fs::read_to_string(&skill_md).await.ok()?;

        // 去除 frontmatter，返回纯 Markdown
        // Remove frontmatter and return pure Markdown
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            Some(parts[2].trim().to_string())
        } else {
            Some(content)
        }
    }

    /// 加载多个 Skills 的内容用于上下文
    /// Load content of multiple Skills for context
    ///
    /// # Arguments
    ///
    /// * `skill_names` - Skill 名称列表
    /// * `skill_names` - List of skill names
    ///
    /// # Returns
    ///
    /// 返回所有 Skills 的 Markdown 内容，用 --- 分隔
    /// Returns Markdown content for all skills, separated by ---
    pub async fn load_skills_for_context(&self, skill_names: &[String]) -> String {
        let mut parts = Vec::new();

        for name in skill_names {
            if let Some(content) = self.load_skill_async(name).await
                && !content.is_empty()
            {
                parts.push(format!("### Skill: {}\n\n{}", name, content));
            }
        }

        parts.join("\n\n---\n\n")
    }

    /// 获取标记为 always 的技能名称列表
    /// Get list of skill names marked as "always"
    pub fn get_always_skills(&self) -> Vec<String> {
        self.controller.get_always_skills()
    }

    /// 获取标记为 always 的技能名称列表（异步版本）
    /// Get list of skill names marked as "always" (Async version)
    pub async fn get_always_skills_async(&self) -> Vec<String> {
        self.get_always_skills()
    }

    /// 检查技能依赖是否满足
    /// Check if skill requirements are satisfied
    pub fn check_requirements(&self, name: &str) -> RequirementCheck {
        self.controller.check_requirements(name)
    }

    /// 检查技能依赖是否满足（异步版本）
    /// Check if skill requirements are satisfied (Async version)
    pub async fn check_requirements_async(&self, name: &str) -> RequirementCheck {
        self.check_requirements(name)
    }

    /// 获取技能的安装指令
    /// Get installation instructions for a skill
    pub fn get_install_instructions(&self, name: &str) -> Option<String> {
        self.controller.get_install_instructions(name)
    }

    /// 获取缺失依赖的描述字符串
    /// Get description string for missing requirements
    pub fn get_missing_requirements_description(&self, name: &str) -> String {
        self.controller.get_missing_requirements_description(name)
    }

    /// 构建技能摘要 XML 格式
    /// Build skills summary in XML format
    ///
    /// 返回所有技能的名称、描述、位置和可用性信息
    /// Returns name, description, location, and availability for all skills
    pub async fn build_skills_summary(&self) -> String {
        let all_metadata = self.get_all_metadata();

        if all_metadata.is_empty() {
            return String::new();
        }

        let mut lines = vec!["<skills>".to_string()];

        for metadata in all_metadata {
            let name = escape_xml(&metadata.name);
            let desc = escape_xml(&metadata.description);
            let path = self
                .controller
                .get_skill_path(&metadata.name)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let check = self.check_requirements_async(&metadata.name).await;
            let available = check.satisfied;

            lines.push(format!("  <skill available=\"{}\">", available));
            lines.push(format!("    <name>{}</name>", name));
            lines.push(format!("    <description>{}</description>", desc));
            lines.push(format!("    <location>{}</location>", escape_xml(&path)));

            // Show missing requirements for unavailable skills
            if !available {
                let missing = check
                    .missing
                    .iter()
                    .map(|r| match r {
                        super::Requirement::CliTool(t) => format!("CLI: {}", t),
                        super::Requirement::EnvVar(v) => format!("ENV: {}", v),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                if !missing.is_empty() {
                    lines.push(format!("    <requires>{}</requires>", escape_xml(&missing)));
                }
            }

            lines.push("  </skill>".to_string());
        }

        lines.push("</skills>".to_string());

        lines.join("\n")
    }

    /// 获取技能描述
    /// Get skill description
    pub async fn get_skill_description(&self, name: &str) -> String {
        self.get_all_metadata()
            .iter()
            .find(|m| m.name == name)
            .map(|m| {
                if m.description.is_empty() {
                    name.to_string()
                } else {
                    m.description.clone()
                }
            })
            .unwrap_or_else(|| name.to_string())
    }

    /// 获取所有 Skills 的元数据
    /// Get metadata for all Skills
    pub fn get_all_metadata(&self) -> Vec<SkillMetadata> {
        self.controller.get_all_metadata()
    }

    /// 搜索相关 Skills
    /// Search for relevant Skills
    ///
    /// # Arguments
    ///
    /// * `query` - 搜索关键词
    /// * `query` - Search keyword
    ///
    /// # Returns
    ///
    /// 返回匹配的 Skill 名称列表
    /// Returns list of matching Skill names
    pub fn search(&self, query: &str) -> Vec<String> {
        self.controller.search(query)
    }

    /// 检查 Skill 是否存在
    /// Check if a Skill exists
    pub fn has_skill(&self, name: &str) -> bool {
        self.controller.has_skill(name)
    }

    /// 重新扫描 Skills 目录
    /// Rescan Skills directory
    pub fn rescan(&mut self) -> GlobalResult<usize> {
        self.controller.scan_metadata().map_err(|e| GlobalError::Other(e.to_string()))
    }

    /// 重新扫描 Skills 目录（异步版本）
    /// Rescan Skills directory (Async version)
    pub async fn rescan_async(&mut self) -> GlobalResult<usize> {
        self.rescan()
    }

    /// 列出所有可用的技能信息
    /// List all available skill information
    ///
    /// # Arguments
    ///
    /// * `filter_unavailable` - 是否过滤掉不满足依赖的技能
    /// * `filter_unavailable` - Whether to filter out skills with unmet dependencies
    pub async fn list_skills(&self, filter_unavailable: bool) -> Vec<SkillInfo> {
        let mut skills = Vec::new();
        let all_metadata = self.get_all_metadata();

        for metadata in all_metadata {
            let name = metadata.name.clone();
            let path = self
                .controller
                .get_skill_path(&name)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let source = "skills".to_string();

            if filter_unavailable {
                let check = self.check_requirements_async(&name).await;
                if !check.satisfied {
                    continue;
                }
            }

            skills.push(SkillInfo { name, path, source });
        }

        skills
    }
}

/// 技能信息
/// Skill information
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub path: String,
    pub source: String,
}

/// Escape XML 特殊字符
/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path, name: &str, description: &str) -> std::io::Result<()> {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir)?;

        let content = format!(
            r#"---
name: {}
description: {}
category: test
tags: [test]
version: "1.0.0"
---

# {} Skill

This is a test skill."#,
            name, description, name
        );

        fs::write(skill_dir.join("SKILL.md"), content)?;
        Ok(())
    }

    #[test]
    fn test_new_manager() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "skill1", "First skill").unwrap();
        create_test_skill(skills_dir, "skill2", "Second skill").unwrap();

        let manager = SkillsManager::new(skills_dir).unwrap();
        assert_eq!(manager.get_all_metadata().len(), 2);
    }

    #[test]
    fn test_new_manager_nonexistent_dir() {
        // Non-existent dirs are now allowed (empty skills list)
        let result = SkillsManager::new("/nonexistent/skills");
        assert!(result.is_ok());
        assert!(result.unwrap().get_all_metadata().is_empty());
    }

    #[test]
    fn test_build_system_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "skill1", "First skill").unwrap();

        let manager = SkillsManager::new(skills_dir).unwrap();
        let prompt = manager.build_system_prompt();
        assert!(prompt.contains("skill1"));
        assert!(prompt.contains("First skill"));
    }

    #[test]
    fn test_load_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "skill1", "First skill").unwrap();

        let manager = SkillsManager::new(skills_dir).unwrap();
        let content = manager.load_skill("skill1").unwrap();
        assert!(content.contains("# skill1 Skill"));
        assert!(content.contains("This is a test skill"));
        // 不应该包含 frontmatter
        // Should not contain frontmatter
        assert!(!content.contains("name:"));
    }

    #[test]
    fn test_search() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "pdf_processing", "Process PDF files").unwrap();
        create_test_skill(skills_dir, "web_scraping", "Scrape web pages").unwrap();

        let manager = SkillsManager::new(skills_dir).unwrap();

        let results = manager.search("pdf");
        assert_eq!(results, vec!["pdf_processing".to_string()]);

        let results = manager.search("web");
        assert_eq!(results, vec!["web_scraping".to_string()]);
    }

    #[test]
    fn test_has_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "skill1", "First skill").unwrap();

        let manager = SkillsManager::new(skills_dir).unwrap();
        assert!(manager.has_skill("skill1"));
        assert!(!manager.has_skill("skill2"));
    }
}
