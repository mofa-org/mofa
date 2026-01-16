//! Agent Skills 模块
//!
//! 实现渐进式披露的 Agent Skills 系统，支持 SKILL.md 格式的技能定义。

pub mod metadata;
pub mod parser;
pub mod disclosure;

pub use disclosure::DisclosureController;
pub use metadata::{CodeFile, SkillMetadata, SkillState, SkillVersion};
pub use parser::SkillParser;
