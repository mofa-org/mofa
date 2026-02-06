//! Agent Skills 模块
//!
//! 实现渐进式披露的 Agent Skills 系统，支持 SKILL.md 格式的技能定义。

pub mod disclosure;
pub mod metadata;
pub mod parser;

pub use disclosure::DisclosureController;
pub use metadata::{
    CodeFile, Requirement, RequirementCheck, SkillMetadata, SkillRequirements, SkillState,
    SkillVersion,
};
pub use parser::SkillParser;
