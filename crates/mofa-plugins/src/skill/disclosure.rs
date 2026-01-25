//! 渐进式披露控制

use crate::skill::{metadata::SkillMetadata, parser::SkillParser};
use std::collections::HashMap;
use std::path::PathBuf;

/// 渐进式披露控制器
#[derive(Debug, Clone)]
pub struct DisclosureController {
    skills_dir: PathBuf,
    /// 缓存的元数据（第1层）
    metadata_cache: HashMap<String, SkillMetadata>,
}

impl DisclosureController {
    /// 创建新的披露控制器
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            skills_dir: skills_dir.into(),
            metadata_cache: HashMap::new(),
        }
    }

    /// 扫描并加载所有 Skills 的元数据（第1层）
    pub fn scan_metadata(&mut self) -> anyhow::Result<usize> {
        let mut count = 0;

        for entry in walkdir::WalkDir::new(&self.skills_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                let skill_md = entry.path().join("SKILL.md");
                if skill_md.exists()
                    && let Ok((metadata, _)) = SkillParser::parse_from_file(&skill_md) {
                        self.metadata_cache.insert(metadata.name.clone(), metadata);
                        count += 1;
                    }
            }
        }

        tracing::info!("Scanned {} skills from {}", count, self.skills_dir.display());
        Ok(count)
    }

    /// 第1层：获取所有 Skills 的元数据（用于系统提示）
    pub fn get_all_metadata(&self) -> Vec<SkillMetadata> {
        self.metadata_cache.values().cloned().collect()
    }

    /// 构建系统提示（仅包含元数据）
    pub fn build_system_prompt(&self) -> String {
        let metadata: Vec<String> = self.metadata_cache.values()
            .map(|m| format!("- {}: {}", m.name, m.description))
            .collect();

        format!(
            "You have access to the following skills:\n{}\n\nWhen a task requires a specific skill, \
             load the full SKILL.md file to get detailed instructions.",
            metadata.join("\n")
        )
    }

    /// 获取 Skill 目录路径
    pub fn get_skill_path(&self, name: &str) -> Option<PathBuf> {
        self.metadata_cache.get(name).map(|_| self.skills_dir.join(name))
    }

    /// 检查 Skill 是否存在
    pub fn has_skill(&self, name: &str) -> bool {
        self.metadata_cache.contains_key(name)
    }

    /// 按关键词搜索相关 Skills
    pub fn search(&self, query: &str) -> Vec<String> {
        let query_lower = query.to_lowercase();

        self.metadata_cache.values()
            .filter(|m| {
                m.name.to_lowercase().contains(&query_lower)
                    || m.description.to_lowercase().contains(&query_lower)
                    || m.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .map(|m| m.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
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
    fn test_scan_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "skill1", "First skill").unwrap();
        create_test_skill(skills_dir, "skill2", "Second skill").unwrap();

        let mut controller = DisclosureController::new(skills_dir);
        let count = controller.scan_metadata().unwrap();

        assert_eq!(count, 2);
        assert!(controller.has_skill("skill1"));
        assert!(controller.has_skill("skill2"));
        assert!(!controller.has_skill("skill3"));
    }

    #[test]
    fn test_build_system_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "skill1", "First skill").unwrap();
        create_test_skill(skills_dir, "skill2", "Second skill").unwrap();

        let mut controller = DisclosureController::new(skills_dir);
        controller.scan_metadata().unwrap();

        let prompt = controller.build_system_prompt();
        assert!(prompt.contains("skill1"));
        assert!(prompt.contains("First skill"));
        assert!(prompt.contains("skill2"));
        assert!(prompt.contains("Second skill"));
    }

    #[test]
    fn test_search() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_test_skill(skills_dir, "pdf_processing", "Process PDF files").unwrap();
        create_test_skill(skills_dir, "web_scraping", "Scrape web pages").unwrap();

        let mut controller = DisclosureController::new(skills_dir);
        controller.scan_metadata().unwrap();

        let results = controller.search("pdf");
        assert_eq!(results, vec!["pdf_processing".to_string()]);

        let results = controller.search("web");
        assert_eq!(results, vec!["web_scraping".to_string()]);
    }
}
