//! Prompt 管理模块
//!
//! 提供完整的 Prompt 模板管理功能，包括：
//!
//! - **模板定义**: 支持变量占位符的 Prompt 模板
//! - **模板注册**: 全局和局部 Prompt 注册中心
//! - **模板构建**: 链式 API 构建复杂 Prompt
//! - **模板加载**: 从 YAML 文件加载 Prompt 配置
//! - **预置模板**: 常用场景的 Prompt 模板库
//! - **数据库存储**: 支持 PostgreSQL、MySQL、SQLite 持久化
//! - **插件扩展**: 支持基于插件的动态模板管理

mod builder;
mod memory_store;
mod presets;
mod registry;
mod store;
mod template;
mod plugin;
mod hot_reload; // 新增插件模块

// SQL 存储模块 (条件编译)
#[cfg(any(
    feature = "persistence-postgres",
    feature = "persistence-mysql",
    feature = "persistence-sqlite"
))]
mod sql_store;

// 基础导出
pub use builder::*;
pub use hot_reload::*;
pub use memory_store::*;
pub use plugin::*;
pub use presets::*;
pub use registry::*;
pub use store::*;
pub use template::*;
// 导出插件模块

// 条件导出 SQL 存储
#[cfg(feature = "persistence-postgres")]
pub use sql_store::PostgresPromptStore;

#[cfg(feature = "persistence-mysql")]
pub use sql_store::MySqlPromptStore;

#[cfg(feature = "persistence-sqlite")]
pub use sql_store::SqlitePromptStore;

/// 便捷 prelude 模块
pub mod prelude {
    pub use super::builder::PromptBuilder;
    pub use super::hot_reload::*;
    pub use super::memory_store::InMemoryPromptStore;
    pub use super::plugin::*;
    pub use super::presets::*;
    pub use super::registry::PromptRegistry;
    pub use super::store::{DynPromptStore, PromptEntity, PromptFilter, PromptStore};
    pub use super::template::{PromptTemplate, PromptVariable, VariableType};
    // 导出插件模块到 prelude

    #[cfg(feature = "persistence-postgres")]
    pub use super::sql_store::PostgresPromptStore;

    #[cfg(feature = "persistence-mysql")]
    pub use super::sql_store::MySqlPromptStore;

    #[cfg(feature = "persistence-sqlite")]
    pub use super::sql_store::SqlitePromptStore;
}

