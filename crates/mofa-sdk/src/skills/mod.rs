//! Agent Skills 管理 API
//!
//! 提供 Skills 的统一管理接口，支持：
//! - 渐进式披露（Progressive Disclosure）
//! - 热更新支持
//! - 搜索和加载

pub mod manager;

pub use manager::SkillsManager;

// 重新导出 skill 相关类型
pub use mofa_plugins::skill::{
    DisclosureController, SkillMetadata, SkillParser,
    SkillState, SkillVersion,
};

use std::path::PathBuf;

/// Skills 管理器构建器
#[derive(Debug, Clone)]
pub struct SkillsManagerBuilder {
    skills_dir: PathBuf,
}

impl SkillsManagerBuilder {
    /// 创建新的构建器
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            skills_dir: skills_dir.into(),
        }
    }

    /// 设置 skills 目录
    pub fn with_skills_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.skills_dir = dir.into();
        self
    }

    /// 构建 SkillsManager
    pub fn build(&self) -> anyhow::Result<SkillsManager> {
        SkillsManager::new(&self.skills_dir)
    }
}

/// 便捷函数：从目录创建 SkillsManager
pub fn from_dir(skills_dir: impl AsRef<std::path::Path>) -> anyhow::Result<SkillsManager> {
    SkillsManager::new(skills_dir)
}
