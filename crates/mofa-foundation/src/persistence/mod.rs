//! 数据持久化模块
//! Data Persistence Module
//!
//! 基于微内核设计理念的可选数据持久化能力。
//! Optional data persistence capabilities based on microkernel design principles.
//!
//! # 设计理念
//! # Design Philosophy
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Persistence Architecture                     │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                   Core Traits (微内核)                    │   │
//! │  │                   Core Traits (Microkernel)              │   │
//! │  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐ │   │
//! │  │  │MessageStore │ │ ApiCallStore│ │ SessionStore         │ │   │
//! │  │  └─────────────┘ └─────────────┘ └─────────────────────┘ │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │                               │                                 │
//! │                               ▼                                 │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                   Backend Implementations                │   │
//! │  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐ │   │
//! │  │  │ PostgreSQL  │ │   MySQL     │ │     SQLite          │ │   │
//! │  │  │  (sqlx-pg)  │ │ (sqlx-mysql)│ │  (sqlx-sqlite)      │ │   │
//! │  │  └─────────────┘ └─────────────┘ └─────────────────────┘ │   │
//! │  │                 ┌─────────────────────────────────────┐   │   │
//! │  │                 │         In-Memory (default)         │   │   │
//! │  │                 └─────────────────────────────────────┘   │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │                               │                                 │
//! │                               ▼                                 │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                   Plugin Integration                     │   │
//! │  │  ┌─────────────────────────────────────────────────────┐ │   │
//! │  │  │ PersistencePlugin - 自动记录 LLM 调用和消息           │ │   │
//! │  │  │ PersistencePlugin - Auto-log LLM calls and messages │ │   │
//! │  │  └─────────────────────────────────────────────────────┘ │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 特性
//! # Features
//!
//! - **可选依赖**: 通过 feature flags 控制后端实现
//! - **Optional Dependencies**: Control backend implementations via feature flags
//! - **trait 抽象**: 核心功能通过 trait 定义，支持自定义实现
//! - **Trait Abstraction**: Core functions defined via traits, supporting custom implementations
//! - **插件集成**: 提供 `PersistencePlugin` 无缝集成到 LLMAgent
//! - **Plugin Integration**: Provides `PersistencePlugin` for seamless LLMAgent integration
//! - **异步设计**: 全异步 API，不阻塞主流程
//! - **Async Design**: Fully asynchronous APIs, no blocking of the main process
//!
//! # 使用示例
//! # Usage Examples
//!
//! ## 内存存储 (默认)
//! ## In-Memory Storage (Default)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{InMemoryStore, PersistencePlugin};
//! use uuid::Uuid;
//!
//! let store = InMemoryStore::new();
//! let plugin = PersistencePlugin::from_store(
//!     "persistence-plugin",
//!     store,
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//! );
//! ```
//!
//! ## PostgreSQL 存储 (需要 `persistence-postgres` feature)
//! ## PostgreSQL Storage (Requires `persistence-postgres` feature)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{PostgresStore, PersistencePlugin};
//! use uuid::Uuid;
//!
//! let store = PostgresStore::connect("postgres://localhost/mofa").await?;
//! let plugin = PersistencePlugin::from_store(
//!     "persistence-plugin",
//!     store,
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//! );
//! ```
//!
//! ## MySQL 存储 (需要 `persistence-mysql` feature)
//! ## MySQL Storage (Requires `persistence-mysql` feature)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{MySqlStore, PersistencePlugin};
//! use uuid::Uuid;
//!
//! let store = MySqlStore::connect("mysql://localhost/mofa").await?;
//! let plugin = PersistencePlugin::from_store(
//!     "persistence-plugin",
//!      store,
//!      Uuid::now_v7(),
//!      Uuid::now_v7(),
//!      Uuid::now_v7(),
//!      Uuid::now_v7(),
//! );
//! ```
//!
//! ## SQLite 存储 (需要 `persistence-sqlite` feature)
//! ## SQLite Storage (Requires `persistence-sqlite` feature)
//!
//! ```rust,ignore
//! use mofa_foundation::persistence::{SqliteStore, PersistencePlugin};
//! use uuid::Uuid;
//!
//! // 文件数据库
//! // File-based database
//! let store = SqliteStore::connect("sqlite:./data.db").await?;
//!
//! // 内存数据库 (适用于测试)
//! // In-memory database (suitable for testing)
//! let store = SqliteStore::in_memory().await?;
//!
//! let plugin = PersistencePlugin::from_store(
//!     "persistence-plugin",
//!     store,
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//!     Uuid::now_v7(),
//! );
//! ```

mod entities;
mod memory;
mod metrics_source;
mod plugin;
mod traits;

pub use entities::*;
pub use memory::*;
pub use metrics_source::*;
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
