//! Agent Skills Management API
//!
//! Provides a unified management interface for Skills, supporting:
//! - Progressive Disclosure
//! - Hot update support
//! - Search and loading

pub mod manager;

pub use manager::{SkillInfo, SkillsManager};

// Re-export skill related types
pub use mofa_plugins::skill::{
    DisclosureController, Requirement, RequirementCheck, SkillMetadata, SkillParser,
    SkillRequirements, SkillState, SkillVersion,
};

use mofa_kernel::agent::types::error::GlobalResult;
use std::path::PathBuf;

/// Skills Manager Builder
#[derive(Debug, Clone)]
pub struct SkillsManagerBuilder {
    search_dirs: Vec<PathBuf>,
}

impl SkillsManagerBuilder {
    /// Create a new builder
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            search_dirs: vec![skills_dir.into()],
        }
    }

    /// Set skills directory (single directory)
    pub fn with_skills_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.search_dirs = vec![dir.into()];
        self
    }

    /// Add search directories (multiple directories, sorted by priority)
    pub fn with_search_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.search_dirs = dirs;
        self
    }

    /// Add a single search directory
    pub fn add_search_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.search_dirs.push(dir.into());
        self
    }

    /// Build SkillsManager
    pub fn build(&self) -> GlobalResult<SkillsManager> {
        SkillsManager::new(&self.search_dirs[0])
    }

    /// Build SkillsManager with multi-directory support
    pub fn build_multi(&self) -> GlobalResult<SkillsManager> {
        if self.search_dirs.len() == 1 {
            SkillsManager::new(&self.search_dirs[0])
        } else {
            SkillsManager::with_search_dirs(self.search_dirs.clone())
        }
    }
}

/// Convenience function: Create SkillsManager from a directory
pub fn from_dir(skills_dir: impl AsRef<std::path::Path>) -> GlobalResult<SkillsManager> {
    SkillsManager::new(skills_dir)
}
