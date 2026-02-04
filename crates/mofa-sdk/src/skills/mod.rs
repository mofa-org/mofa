//! Agent Skills 管理 API
//!
//! 提供 Skills 的统一管理接口，支持：
//! - 渐进式披露（Progressive Disclosure）
//! - 热更新支持
//! - 搜索和加载

pub mod manager;

pub use manager::{SkillsManager, SkillInfo};

// 重新导出 skill 相关类型
pub use mofa_plugins::skill::{
    DisclosureController, Requirement, RequirementCheck, SkillMetadata, SkillParser,
    SkillRequirements, SkillState, SkillVersion,
};

use std::path::PathBuf;

/// Skills 管理器构建器
#[derive(Debug, Clone)]
pub struct SkillsManagerBuilder {
    search_dirs: Vec<PathBuf>,
}

impl SkillsManagerBuilder {
    /// 创建新的构建器
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            search_dirs: vec![skills_dir.into()],
        }
    }

    /// 设置 skills 目录（单目录）
    pub fn with_skills_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.search_dirs = vec![dir.into()];
        self
    }

    /// 添加搜索目录（多目录，按优先级排序）
    pub fn with_search_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.search_dirs = dirs;
        self
    }

    /// 添加一个搜索目录
    pub fn add_search_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.search_dirs.push(dir.into());
        self
    }

    /// 构建 SkillsManager
    pub fn build(&self) -> anyhow::Result<SkillsManager> {
        SkillsManager::new(&self.search_dirs[0])
    }

    /// 构建支持多目录的 SkillsManager
    pub fn build_multi(&self) -> anyhow::Result<SkillsManager> {
        if self.search_dirs.len() == 1 {
            SkillsManager::new(&self.search_dirs[0])
        } else {
            SkillsManager::with_search_dirs(self.search_dirs.clone())
        }
    }
}

/// 便捷函数：从目录创建 SkillsManager
pub fn from_dir(skills_dir: impl AsRef<std::path::Path>) -> anyhow::Result<SkillsManager> {
    SkillsManager::new(skills_dir)
}
