//! 配置管理示例
//! Configuration Management Example
//!
//! 展示如何使用框架的统一配置管理系统
//! Demonstrating the use of the framework's unified configuration management system

use mofa_sdk::runtime::FrameworkConfig;
use tracing::{info, Level};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    // 1. 从JSON文件加载配置
    // 1. Load configuration from JSON file
    info!("=== 1. 从JSON字符串加载配置 ===");
    info!("=== 1. Load configuration from JSON string ===");
    let json_config = r#"
    {
        "database": {
            "type": "mysql",
            "url": "mysql://root:password@localhost:3306/mofa",
            "max_connections": 20,
            "connection_timeout": 30
        },
        "cache": {
            "type": "redis",
            "servers": ["redis://localhost:6379", "redis://localhost:6380"],
            "prefix": "mofa:",
            "default_ttl": 3600
        },
        "message_queue": {
            "type": "kafka",
            "brokers": ["localhost:9092"],
            "topic": "agent_events",
            "group_id": "aimo_consumer_group"
        },
        "environment": "development"
    }
    "#;

    let framework_config: FrameworkConfig = serde_json::from_str(json_config)?;

    info!("数据库类型: {}", framework_config.database.r#type);
    info!("Database type: {}", framework_config.database.r#type);
    info!("数据库URL: {}", framework_config.database.url);
    info!("Database URL: {}", framework_config.database.url);
    info!("Redis服务器: {:?}", framework_config.cache.servers);
    info!("Redis servers: {:?}", framework_config.cache.servers);
    info!("Kafka主题: {:?}", framework_config.message_queue.topic);
    info!("Kafka topic: {:?}", framework_config.message_queue.topic);
    info!("运行环境: {:?}", framework_config.environment);
    info!("Runtime environment: {:?}", framework_config.environment);

    info!("\n=== 2. 配置加载器示例 ===");
    info!("\n=== 2. Configuration loader example ===");
    info!("配置加载器支持从JSON/YAML文件和环境变量加载配置");
    info!("Loader supports config from JSON/YAML files and environment variables");
    info!("例如: ConfigLoader::from_file(\"config.yml\")?;");
    info!("Example: ConfigLoader::from_file(\"config.yml\")?;");
    info!("例如: ConfigLoader::from_env()?;");
    info!("Example: ConfigLoader::from_env()?;");

    // 3. 配置访问接口
    // 3. Configuration access interface
    info!("\n=== 3. 配置访问接口 ===");
    info!("\n=== 3. Configuration access interface ===");
    // 演示直接访问配置字段
    // Demonstrating direct access to configuration fields
    info!("直接访问数据库配置: {:?}", framework_config.database);
    info!("Direct access to database config: {:?}", framework_config.database);
    info!("直接访问缓存配置: {:?}", framework_config.cache);
    info!("Direct access to cache config: {:?}", framework_config.cache);
    info!("直接访问消息队列配置: {:?}", framework_config.message_queue);
    info!("Direct access to message queue config: {:?}", framework_config.message_queue);

    info!("\n配置管理系统演示完成!");
    info!("\nConfiguration management system demo completed!");

    Ok(())
}
