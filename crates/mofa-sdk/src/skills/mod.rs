//! Agent Skills 管理 API
//! Agent Skills Management API
//!
//! 提供 Skills 的统一管理接口，支持：
//! Provides a unified management interface for Skills, supporting:
//! - 渐进式披露（Progressive Disclosure）
//! - Progressive Disclosure
//! - 热更新支持
//! - Hot update support
//! - 搜索和加载
//! - Search and loading

pub mod manager;

pub use manager::{SkillInfo, SkillsManager};

// 重新导出 skill 相关类型
// Re-export skill related types
pub use mofa_plugins::skill::{
    DisclosureController, Requirement, RequirementCheck, SkillMetadata, SkillParser,
    SkillRequirements, SkillState, SkillVersion,
};

use mofa_kernel::agent::types::error::GlobalResult;
use std::path::PathBuf;

/// Skills 管理器构建器
/// Skills Manager Builder
#[derive(Debug, Clone)]
pub struct SkillsManagerBuilder {
    search_dirs: Vec<PathBuf>,
}

impl SkillsManagerBuilder {
    /// 创建新的构建器
    /// Create a new builder
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            search_dirs: vec![skills_dir.into()],
        }
    }

    /// 设置 skills 目录（单目录）
    /// Set skills directory (single directory)
    pub fn with_skills_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.search_dirs = vec![dir.into()];
        self
    }

    /// 添加搜索目录（多目录，按优先级排序）
    /// Add search directories (multiple directories, sorted by priority)
    pub fn with_search_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.search_dirs = dirs;
        self
    }

    /// 添加一个搜索目录
    /// Add a single search directory
    pub fn add_search_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.search_dirs.push(dir.into());
        self
    }

    /// 构建 SkillsManager
    /// Build SkillsManager
    pub fn build(&self) -> GlobalResult<SkillsManager> {
        SkillsManager::new(&self.search_dirs[0])
    }

    /// 构建支持多目录的 SkillsManager
    /// Build SkillsManager with multi-directory support
    pub fn build_multi(&self) -> GlobalResult<SkillsManager> {
        if self.search_dirs.len() == 1 {
            SkillsManager::new(&self.search_dirs[0])
        } else {
            SkillsManager::with_search_dirs(self.search_dirs.clone())
        }
    }
}

/// 便捷函数：从目录创建 SkillsManager
/// Convenience function: Create SkillsManager from a directory
pub fn from_dir(skills_dir: impl AsRef<std::path::Path>) -> GlobalResult<SkillsManager> {
    SkillsManager::new(skills_dir)
}
