//! SKILL.md 文件解析器
//! SKILL.md file parser

use crate::skill::metadata::SkillMetadata;
use mofa_kernel::plugin::{PluginError, PluginResult};
use regex::Regex;
use std::fs;
use std::path::Path;

/// SKILL.md 解析器
/// SKILL.md parser
pub struct SkillParser;

impl SkillParser {
    /// 解析 YAML frontmatter
    /// Parse YAML frontmatter
    pub fn parse_frontmatter(content: &str) -> PluginResult<(SkillMetadata, String)> {
        // Use [\s\S]*? instead of .*? to match newlines in YAML content
        let frontmatter_regex = Regex::new(r"^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$").unwrap();

        if let Some(caps) = frontmatter_regex.captures(content) {
            let yaml = &caps[1];
            let markdown = &caps[2];

            let metadata: SkillMetadata = serde_yaml::from_str(yaml).map_err(|e| {
                mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                    "Failed to parse YAML frontmatter: {}",
                    e
                ))
            })?;

            Ok((metadata, markdown.to_string()))
        } else {
            Err(PluginError::ExecutionFailed(
                "SKILL.md must start with YAML frontmatter".to_string(),
            ))
        }
    }

    /// 从 SKILL.md 文件解析元数据
    /// Parse metadata from SKILL.md file
    pub fn parse_from_file(
        skill_md_path: impl AsRef<Path>,
    ) -> PluginResult<(SkillMetadata, String)> {
        let content = fs::read_to_string(skill_md_path.as_ref())
            .map_err(|e| PluginError::Other(e.to_string()))?;
        Self::parse_frontmatter(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test_skill
description: A test skill
category: test
tags: [test, example]
version: "1.0.0"
---

# Test Skill

This is a test skill."#;

        let (metadata, markdown) = SkillParser::parse_frontmatter(content).unwrap();

        assert_eq!(metadata.name, "test_skill");
        assert_eq!(metadata.description, "A test skill");
        assert_eq!(metadata.category, Some("test".to_string()));
        assert_eq!(metadata.tags, vec!["test", "example"]);
        assert_eq!(metadata.version, Some("1.0.0".to_string()));
        assert!(markdown.contains("# Test Skill"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# No Frontmatter\nJust content";
        let result = SkillParser::parse_frontmatter(content);
        assert!(result.is_err());
    }
}
