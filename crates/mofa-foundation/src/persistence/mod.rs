//! 数据持久化模块
//!
//! 基于微内核设计理念的可选数据持久化能力。
//!
//! # 设计理念
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Persistence Architecture                      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                   Core Traits (微内核)                     │   │
//! │  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐ │   │
//! │  │  │MessageStore │ │ ApiCallStore│ │ SessionStore        │ │   │
//! │  │  └─────────────┘ └─────────────┘ └─────────────────────┘ │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │                              │                                   │
//! │                              ▼                                   │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                   Backend Implementations                 │   │
//! │  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐ │   │
//! │  │  │ PostgreSQL  │ │   MySQL     │ │    SQLite           │ │   │
//! │  │  │  (sqlx-pg)  │ │ (sqlx-mysql)│ │  (sqlx-sqlite)      │ │   │
//! │  │  └─────────────┘ └─────────────┘ └─────────────────────┘ │   │
//! │  │                 ┌─────────────────────────────────────┐   │   │
//! │  │                 │           In-Memory (default)       │   │   │
//! │  │                 └─────────────────────────────────────┘   │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │                              │                                   │
//! │                              ▼                                   │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                   Plugin Integration                      │   │
//! │  │  ┌─────────────────────────────────────────────────────┐ │   │
//! │  │  │ PersistencePlugin - 自动记录 LLM 调用和消息          │ │   │
//! │  │  └─────────────────────────────────────────────────────┘ │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 特性
//!
//! - **可选依赖**: 通过 feature flags 控制后端实现
//! - **trait 抽象**: 核心功能通过 trait 定义，支持自定义实现
//! - **插件集成**: 提供 `PersistencePlugin` 无缝集成到 LLMAgent
//! - **异步设计**: 全异步 API，不阻塞主流程
//!
//! # 使用示例
//!
//! ## 内存存储 (默认)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{InMemoryStore, PersistencePlugin};
//!
//! let store = InMemoryStore::new();
//! let plugin = PersistencePlugin::new(store);
//!
//! let agent = LLMAgent::builder()
//!     .with_plugin(plugin)
//!     .build()?;
//! ```
//!
//! ## PostgreSQL 存储 (需要 `persistence-postgres` feature)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{PostgresStore, PersistenceHandler};
//!
//! let store = PostgresStore::connect("postgres://localhost/mofa").await?;
//! let handler = PersistenceHandler::new(store.into(), user_id, agent_id);
//! ```
//!
//! ## MySQL 存储 (需要 `persistence-mysql` feature)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{MySqlStore, PersistenceHandler};
//!
//! let store = MySqlStore::connect("mysql://localhost/mofa").await?;
//! let handler = PersistenceHandler::new(store.into(), user_id, agent_id);
//! ```
//!
//! ## SQLite 存储 (需要 `persistence-sqlite` feature)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{SqliteStore, PersistenceHandler};
//!
//! // 文件数据库
//! let store = SqliteStore::connect("sqlite:./data.db").await?;
//!
//! // 内存数据库 (适用于测试)
//! let store = SqliteStore::in_memory().await?;
//!
//! let handler = PersistenceHandler::new(store.into(), user_id, agent_id);
//! ```

mod entities;
mod memory;
mod plugin;
mod traits;

pub use entities::*;
pub use memory::*;
pub use plugin::*;
pub use traits::*;

#[cfg(feature = "persistence-postgres")]
mod postgres;
#[cfg(feature = "persistence-postgres")]
pub use postgres::*;

#[cfg(feature = "persistence-mysql")]
mod mysql;
#[cfg(feature = "persistence-mysql")]
pub use mysql::*;

#[cfg(feature = "persistence-sqlite")]
mod sqlite;
#[cfg(feature = "persistence-sqlite")]
pub use sqlite::*;
