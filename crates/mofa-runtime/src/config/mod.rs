//! 统一配置管理模块
//! Unified configuration management module
//!
//! 提供框架级的配置加载、解析和访问接口，支持：
//! Provides framework-level configuration loading, parsing, and access interfaces, supporting:
//! - 多种配置格式 (YAML, TOML, JSON, INI, RON, JSON5)
//! - Multiple configuration formats (YAML, TOML, JSON, INI, RON, JSON5)
//! - 数据库配置
//! - Database configurations
//! - 缓存配置
//! - Cache configurations
//! - 消息队列配置
//! - Message queue configurations
//! - 多环境支持
//! - Multi-environment support
//! - 配置热加载
//! - Configuration hot-reloading

use mofa_kernel::config::load_config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// 配置错误类型
/// Configuration error types
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parse error: {0}")]
    Parse(String),

    #[error("Config field missing: {0}")]
    FieldMissing(&'static str),

    #[error("Invalid config value: {0}")]
    InvalidValue(&'static str),

    #[error("Unsupported config format: {0}")]
    UnsupportedFormat(String),
}

/// 配置加载器
/// Configuration loader
pub struct ConfigLoader {
    config: FrameworkConfig,
}

impl ConfigLoader {
    /// 从文件加载配置 (自动检测格式)
    /// Load configuration from file (auto-detect format)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let config = load_config(&path_str).map_err(|e| match e {
            mofa_kernel::config::ConfigError::Io(e) => ConfigError::Io(e),
            mofa_kernel::config::ConfigError::Parse(e) => ConfigError::Parse(e.to_string()),
            mofa_kernel::config::ConfigError::Serialization(e) => ConfigError::Parse(e),
            mofa_kernel::config::ConfigError::UnsupportedFormat(e) => {
                ConfigError::UnsupportedFormat(e)
            }
            _ => ConfigError::Parse(e.to_string()),
        })?;

        Ok(Self { config })
    }

    /// 从环境变量加载配置
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        // 从环境变量构建配置
        // Build configuration from environment variables
        // 这里实现简化版本，实际可以使用 envy 等库
        // Simplified implementation here; libraries like envy can be used in practice
        Ok(Self {
            config: FrameworkConfig::default(),
        })
    }

    /// 获取完整配置
    /// Get the complete configuration
    pub fn config(&self) -> &FrameworkConfig {
        &self.config
    }

    /// 获取数据库配置
    /// Get database configuration
    pub fn database_config(&self) -> &DatabaseConfig {
        &self.config.database
    }

    /// 获取缓存配置
    /// Get cache configuration
    pub fn cache_config(&self) -> &CacheConfig {
        &self.config.cache
    }

    /// 获取消息队列配置
    /// Get message queue configuration
    pub fn message_queue_config(&self) -> &MessageQueueConfig {
        &self.config.message_queue
    }
}

/// 数据库配置
/// Database configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DatabaseConfig {
    /// 数据库类型
    /// Database type
    pub r#type: String,

    /// 数据库连接URL
    /// Database connection URL
    pub url: String,

    /// 最大连接数
    /// Maximum connection pool size
    pub max_connections: Option<u32>,

    /// 连接超时时间（秒）
    /// Connection timeout (seconds)
    pub connection_timeout: Option<u32>,

    /// 空闲连接超时时间（秒）
    /// Idle connection timeout (seconds)
    pub idle_timeout: Option<u32>,

    /// 额外配置参数
    /// Additional configuration parameters
    pub extra: Option<HashMap<String, String>>,
}

/// 缓存配置
/// Cache configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CacheConfig {
    /// 缓存类型
    /// Cache type
    pub r#type: String,

    /// 缓存服务器地址
    /// Cache server addresses
    pub servers: Vec<String>,

    /// 缓存前缀
    /// Cache key prefix
    pub prefix: Option<String>,

    /// 默认过期时间（秒）
    /// Default expiration time (seconds)
    pub default_ttl: Option<u32>,

    /// 最大容量
    /// Maximum capacity size
    pub max_size: Option<usize>,

    /// 额外配置参数
    /// Additional configuration parameters
    pub extra: Option<HashMap<String, String>>,
}

/// 消息队列配置
/// Message queue configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MessageQueueConfig {
    /// 消息队列类型
    /// Message queue type
    pub r#type: String,

    /// 消息队列服务器地址
    /// Message queue broker addresses
    pub brokers: Vec<String>,

    /// 消息队列主题
    /// Message queue topic
    pub topic: Option<String>,

    /// 消费组
    /// Consumer group ID
    pub group_id: Option<String>,

    /// 额外配置参数
    /// Additional configuration parameters
    pub extra: Option<HashMap<String, String>>,
}

/// 框架核心配置
/// Framework core configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FrameworkConfig {
    /// 数据库配置
    /// Database configuration
    pub database: DatabaseConfig,

    /// 缓存配置
    /// Cache configuration
    pub cache: CacheConfig,

    /// 消息队列配置
    /// Message queue configuration
    pub message_queue: MessageQueueConfig,

    /// 框架名称
    /// Framework name
    pub framework_name: Option<String>,

    /// 框架版本
    /// Framework version
    pub framework_version: Option<String>,

    /// 环境名称
    /// Environment name
    pub environment: Option<String>,
}
