//! Skill 元数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Skill YAML Frontmatter（第1层：启动时加载）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Skill 名称（唯一标识符）
    pub name: String,
    /// Skill 描述（用于 LLM 判断何时使用）
    pub description: String,
    /// Skill 分类
    #[serde(default)]
    pub category: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 版本
    #[serde(default)]
    pub version: Option<String>,
    /// 作者
    #[serde(default)]
    pub author: Option<String>,
}

/// Skill 版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersion {
    /// 内容哈希（SHA256）
    pub content_hash: String,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// Skill 状态
#[derive(Debug, Clone, PartialEq)]
pub enum SkillState {
    Active,
    Updating,
    RolledBack,
    Disabled,
}

/// 代码文件定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFile {
    /// 文件路径（相对于 skill 目录）
    pub path: PathBuf,
    /// 语言类型
    pub language: String,
    /// 执行命令模板
    pub command: Option<String>,
}
