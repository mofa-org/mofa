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
    /// 是否始终加载（always skills）
    #[serde(default)]
    pub always: bool,
    /// 依赖要求
    #[serde(default)]
    pub requires: Option<SkillRequirements>,
    /// 安装指令
    #[serde(default)]
    pub install: Option<String>,
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

/// Skill 依赖要求
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillRequirements {
    /// 需要的 CLI 工具
    #[serde(default)]
    pub cli_tools: Vec<String>,
    /// 需要的环境变量
    #[serde(default)]
    pub env_vars: Vec<String>,
}

/// 依赖项类型
#[derive(Debug, Clone, PartialEq)]
pub enum Requirement {
    /// CLI 工具
    CliTool(String),
    /// 环境变量
    EnvVar(String),
}

/// 依赖检查结果
#[derive(Debug, Clone, Default)]
pub struct RequirementCheck {
    /// 是否满足所有要求
    pub satisfied: bool,
    /// 缺失的依赖
    pub missing: Vec<Requirement>,
}
