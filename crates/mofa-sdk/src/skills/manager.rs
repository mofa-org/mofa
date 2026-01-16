//! Skills Manager - SDK 层统一 API

use super::DisclosureController;
use std::path::{Path, PathBuf};

/// Skills Manager - SDK 层统一 API
///
/// 提供 Skills 的管理和查询接口，支持渐进式披露。
#[derive(Debug, Clone)]
pub struct SkillsManager {
    skills_dir: PathBuf,
    controller: DisclosureController,
}

impl SkillsManager {
    /// 创建新的 Skills Manager
    ///
    /// # Arguments
    ///
    /// * `skills_dir` - Skills 目录路径
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mofa_sdk::skills::SkillsManager;
    ///
    /// let manager = SkillsManager::new("./skills").unwrap();
    /// ```
    pub fn new(skills_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let skills_dir = skills_dir.as_ref();

        if !skills_dir.exists() {
            anyhow::bail!("Skills directory does not exist: {}", skills_dir.display());
        }

        let mut controller = DisclosureController::new(skills_dir);
        controller.scan_metadata()?;

        Ok(Self {
            skills_dir: skills_dir.to_path_buf(),
            controller,
        })
    }

    /// 获取系统提示（第1层：仅元数据）
    ///
    /// 返回包含所有 Skills 元数据的系统提示字符串。
    pub fn build_system_prompt(&self) -> String {
        self.controller.build_system_prompt()
    }

    /// 获取所有 Skills 的元数据
    pub fn get_all_metadata(&self) -> Vec<super::SkillMetadata> {
        self.controller.get_all_metadata()
    }

    /// 加载 Skill 的 SKILL.md 内容（第2层）
    ///
    /// # Arguments
    ///
    /// * `name` - Skill 名称
    ///
    /// # Returns
    ///
    /// 返回 SKILL.md 的 Markdown 内容（去除 frontmatter）
    pub fn load_skill(&self, name: &str) -> Option<String> {
        let skill_path = self.controller.get_skill_path(name)?;
        let skill_md = skill_path.join("SKILL.md");

        let content = std::fs::read_to_string(&skill_md).ok()?;

        // 去除 frontmatter，返回纯 Markdown
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            Some(parts[2].trim().to_string())
        } else {
            Some(content)
        }
    }

    /// 搜索相关 Skills
    ///
    /// # Arguments
    ///
    /// * `query` - 搜索关键词
    ///
    /// # Returns
    ///
    /// 返回匹配的 Skill 名称列表
    pub fn search(&self, query: &str) -> Vec<String> {
        self.controller.search(query)
    }

    /// 检查 Skill 是否存在
    pub fn has_skill(&self, name: &str) -> bool {
        self.controller.has_skill(name)
    }

    /// 获取 Skills 目录路径
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    /// 重新扫描 Skills 目录
    pub fn rescan(&mut self) -> anyhow::Result<usize> {
        self.controller.scan_metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path, name: &str, description: &str) -> std::io::Result<()> {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir)?;

        let content = format!(r#"---
name: {}
description: {}
category: test
tags: [test]
version: "1.0.0"
---

# {} Skill

This is a test skill."#, name, description, name);

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
        let result = SkillsManager::new("/nonexistent/skills");
        assert!(result.is_err());
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
