//! 统一配置管理模块
//!
//! 提供框架级的配置加载、解析和访问接口，支持：
//! - 多种配置格式 (YAML, TOML, JSON, INI, RON, JSON5)
//! - 数据库配置
//! - 缓存配置
//! - 消息队列配置
//! - 多环境支持
//! - 配置热加载

use mofa_kernel::config::{detect_format, from_str, load_config};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 配置错误类型
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
pub struct ConfigLoader {
    config: FrameworkConfig,
}

impl ConfigLoader {
    /// 从文件加载配置 (自动检测格式)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let config = load_config(&path_str).map_err(|e| match e {
            mofa_kernel::config::ConfigError::Io(e) => ConfigError::Io(e),
            mofa_kernel::config::ConfigError::Parse(e) => ConfigError::Parse(e.to_string()),
            mofa_kernel::config::ConfigError::Serialization(e) => ConfigError::Parse(e),
            mofa_kernel::config::ConfigError::UnsupportedFormat(e) => ConfigError::UnsupportedFormat(e),
        })?;

        Ok(Self { config })
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self, ConfigError> {
        // 从环境变量构建配置
        // 这里实现简化版本，实际可以使用 envy 等库
        Ok(Self {
            config: FrameworkConfig::default(),
        })
    }

    /// 获取完整配置
    pub fn config(&self) -> &FrameworkConfig {
        &self.config
    }

    /// 获取数据库配置
    pub fn database_config(&self) -> &DatabaseConfig {
        &self.config.database
    }

    /// 获取缓存配置
    pub fn cache_config(&self) -> &CacheConfig {
        &self.config.cache
    }

    /// 获取消息队列配置
    pub fn message_queue_config(&self) -> &MessageQueueConfig {
        &self.config.message_queue
    }
}

/// 数据库配置
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DatabaseConfig {
    /// 数据库类型
    pub r#type: String,

    /// 数据库连接URL
    pub url: String,

    /// 最大连接数
    pub max_connections: Option<u32>,

    /// 连接超时时间（秒）
    pub connection_timeout: Option<u32>,

    /// 空闲连接超时时间（秒）
    pub idle_timeout: Option<u32>,

    /// 额外配置参数
    pub extra: Option<HashMap<String, String>>,
}

/// 缓存配置
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CacheConfig {
    /// 缓存类型
    pub r#type: String,

    /// 缓存服务器地址
    pub servers: Vec<String>,

    /// 缓存前缀
    pub prefix: Option<String>,

    /// 默认过期时间（秒）
    pub default_ttl: Option<u32>,

    /// 最大容量
    pub max_size: Option<usize>,

    /// 额外配置参数
    pub extra: Option<HashMap<String, String>>,
}

/// 消息队列配置
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MessageQueueConfig {
    /// 消息队列类型
    pub r#type: String,

    /// 消息队列服务器地址
    pub brokers: Vec<String>,

    /// 消息队列主题
    pub topic: Option<String>,

    /// 消费组
    pub group_id: Option<String>,

    /// 额外配置参数
    pub extra: Option<HashMap<String, String>>,
}

/// 框架核心配置
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FrameworkConfig {
    /// 数据库配置
    pub database: DatabaseConfig,

    /// 缓存配置
    pub cache: CacheConfig,

    /// 消息队列配置
    pub message_queue: MessageQueueConfig,

    /// 框架名称
    pub framework_name: Option<String>,

    /// 框架版本
    pub framework_version: Option<String>,

    /// 环境名称
    pub environment: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_yaml_config() {
        let yaml = r#"
database:
  type: postgresql
  url: postgresql://localhost/mydb
  max_connections: 10

cache:
  type: redis
  servers:
    - localhost:6379
  default_ttl: 3600

message_queue:
  type: kafka
  brokers:
    - localhost:9092
  topic: mofa-events
"#;

        let config: FrameworkConfig = from_str(yaml, config::FileFormat::Yaml).unwrap();
        assert_eq!(config.database.r#type, "postgresql");
        assert_eq!(config.cache.r#type, "redis");
        assert_eq!(config.message_queue.r#type, "kafka");
    }

    #[test]
    fn test_load_json_config() {
        let json = r#"{
    "database": {
        "type": "postgresql",
        "url": "postgresql://localhost/mydb"
    },
    "cache": {
        "type": "redis",
        "servers": ["localhost:6379"]
    }
}"#;

        let config: FrameworkConfig = from_str(json, config::FileFormat::Json).unwrap();
        assert_eq!(config.database.r#type, "postgresql");
        assert_eq!(config.cache.r#type, "redis");
    }

    #[test]
    fn test_load_toml_config() {
        let toml = r#"
[database]
type = "postgresql"
url = "postgresql://localhost/mydb"

[cache]
type = "redis"
servers = ["localhost:6379"]
"#;

        let config: FrameworkConfig = from_str(toml, config::FileFormat::Toml).unwrap();
        assert_eq!(config.database.r#type, "postgresql");
        assert_eq!(config.cache.r#type, "redis");
    }

    #[test]
    fn test_load_ini_config() {
        let ini = r#"
[database.type]
value = "postgresql"

[database.url]
value = "postgresql://localhost/mydb"

[cache.type]
value = "redis"

[cache.servers]
value = "localhost:6379"
"#;

        let config: FrameworkConfig = from_str(ini, config::FileFormat::Ini).unwrap();
        assert_eq!(config.database.r#type, "postgresql");
        assert_eq!(config.cache.r#type, "redis");
    }

    #[test]
    fn test_load_ron_config() {
        let ron = r#"
(
    database: (
        type: "postgresql",
        url: "postgresql://localhost/mydb",
    ),
    cache: (
        type: "redis",
        servers: ["localhost:6379"],
    ),
    message_queue: (
        type: "kafka",
        brokers: ["localhost:9092"],
    ),
)
"#;

        let config: FrameworkConfig = from_str(ron, config::FileFormat::Ron).unwrap();
        assert_eq!(config.database.r#type, "postgresql");
        assert_eq!(config.cache.r#type, "redis");
        assert_eq!(config.message_queue.r#type, "kafka");
    }
}
