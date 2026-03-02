//! 配置加载器
//! Configuration loader
//!
//! 支持多种配置格式: YAML, TOML, JSON, INI, RON, JSON5
//! Supports multiple formats: YAML, TOML, JSON, INI, RON, JSON5
//!
//! 使用统一的 config crate 提供一致的 API 接口
//! Uses unified config crate to provide a consistent API interface

use super::schema::AgentConfig;
use config::FileFormat;
use mofa_kernel::config::{ConfigError, detect_format, from_str, load_config, load_merged};
use serde::{Deserialize, Serialize};

/// 配置格式
/// Configuration formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigFormat {
    /// YAML 格式
    /// YAML format
    Yaml,
    /// TOML 格式
    /// TOML format
    Toml,
    /// JSON 格式
    /// JSON format
    Json,
    /// INI 格式
    /// INI format
    Ini,
    /// RON 格式
    /// RON format
    Ron,
    /// JSON5 格式
    /// JSON5 format
    Json5,
}

/// 配置错误类型
/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum AgentConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parse error: {0}")]
    Parse(String),

    #[error("Config serialization error: {0}")]
    Serialization(String),

    #[error("Unsupported config format: {0}")]
    UnsupportedFormat(String),

    #[error("Config validation failed: {0}")]
    Validation(String),
}

/// 配置结果类型
/// Configuration result type
pub type AgentResult<T> = Result<T, AgentConfigError>;

impl ConfigFormat {
    /// 从文件扩展名推断格式
    /// Infer format from file extension
    pub fn from_extension(path: &str) -> Option<Self> {
        match detect_format(path) {
            Ok(FileFormat::Yaml) => Some(Self::Yaml),
            Ok(FileFormat::Toml) => Some(Self::Toml),
            Ok(FileFormat::Json) => Some(Self::Json),
            Ok(FileFormat::Ini) => Some(Self::Ini),
            Ok(FileFormat::Ron) => Some(Self::Ron),
            Ok(FileFormat::Json5) => Some(Self::Json5),
            _ => None,
        }
    }

    /// 转换为 config crate 的 FileFormat
    /// Convert to config crate's FileFormat
    pub fn to_file_format(self) -> FileFormat {
        match self {
            Self::Yaml => FileFormat::Yaml,
            Self::Toml => FileFormat::Toml,
            Self::Json => FileFormat::Json,
            Self::Ini => FileFormat::Ini,
            Self::Ron => FileFormat::Ron,
            Self::Json5 => FileFormat::Json5,
        }
    }

    /// 获取格式名称
    /// Get format name
    pub fn name(&self) -> &str {
        match self {
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Ini => "ini",
            Self::Ron => "ron",
            Self::Json5 => "json5",
        }
    }

    /// 获取默认文件扩展名
    /// Get default file extension
    pub fn default_extension(&self) -> &str {
        match self {
            Self::Yaml => "yml",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Ini => "ini",
            Self::Ron => "ron",
            Self::Json5 => "json5",
        }
    }
}

/// 配置加载器
/// Configuration loader
///
/// 支持从文件或字符串加载配置，支持多种格式
/// Support loading from file or string in various formats
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_runtime::agent::config::{ConfigLoader, ConfigFormat};
///
/// // 从 YAML 字符串加载
/// // Load from YAML string
/// let yaml = r#"
/// id: my-agent
/// name: My Agent
/// type: llm
/// llm:
///   model: gpt-4
/// "#;
/// let config = ConfigLoader::from_str(yaml, ConfigFormat::Yaml)?;
///
/// // 从文件加载 (自动检测格式)
/// // Load from file (auto-detect format)
/// let config = ConfigLoader::load_file("agent.yaml")?;
///
/// // 从 TOML 字符串加载
/// // Load from TOML string
/// let toml = r#"
/// id = "my-agent"
/// name = "My Agent"
/// type = "llm"
/// "#;
/// let config = ConfigLoader::from_toml(toml)?;
///
/// // 从 INI 文件加载
/// // Load from INI file
/// let config = ConfigLoader::load_ini("agent.ini")?;
/// ```
pub struct ConfigLoader;

impl ConfigLoader {
    /// 从字符串加载配置
    /// Load configuration from string
    pub fn from_str(content: &str, format: ConfigFormat) -> AgentResult<AgentConfig> {
        from_str(content, format.to_file_format()).map_err(|e| match e {
            ConfigError::Parse(e) => AgentConfigError::Parse(e.to_string()),
            ConfigError::Serialization(e) => AgentConfigError::Serialization(e),
            ConfigError::UnsupportedFormat(e) => AgentConfigError::UnsupportedFormat(e),
            _ => AgentConfigError::Parse(e.to_string()),
        })
    }

    /// 从 YAML 字符串加载
    /// Load from YAML string
    pub fn from_yaml(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Yaml)
    }

    /// 从 TOML 字符串加载
    /// Load from TOML string
    pub fn from_toml(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Toml)
    }

    /// 从 JSON 字符串加载
    /// Load from JSON string
    pub fn from_json(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Json)
    }

    /// 从 INI 字符串加载
    /// Load from INI string
    pub fn from_ini(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Ini)
    }

    /// 从 RON 字符串加载
    /// Load from RON string
    pub fn from_ron(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Ron)
    }

    /// 从 JSON5 字符串加载
    /// Load from JSON5 string
    pub fn from_json5(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Json5)
    }

    /// 从文件加载配置 (自动检测格式)
    /// Load from file (auto-detect format)
    pub fn load_file(path: &str) -> AgentResult<AgentConfig> {
        let config: AgentConfig = load_config(path).map_err(|e| match e {
            ConfigError::Io(e) => AgentConfigError::Io(e),
            ConfigError::Parse(e) => AgentConfigError::Parse(e),
            ConfigError::Serialization(e) => AgentConfigError::Serialization(e),
            ConfigError::UnsupportedFormat(e) => AgentConfigError::UnsupportedFormat(e),
            _ => AgentConfigError::Parse(e.to_string()),
        })?;

        // 验证配置
        // Validate configuration
        config
            .validate()
            .map_err(|errors| AgentConfigError::Validation(errors.join(", ")))?;

        Ok(config)
    }

    /// 从文件加载 YAML 配置
    /// Load YAML config from file
    pub fn load_yaml(path: &str) -> AgentResult<AgentConfig> {
        Self::load_file(path)
    }

    /// 从文件加载 TOML 配置
    /// Load TOML config from file
    pub fn load_toml(path: &str) -> AgentResult<AgentConfig> {
        Self::load_file(path)
    }

    /// 从文件加载 JSON 配置
    /// Load JSON config from file
    pub fn load_json(path: &str) -> AgentResult<AgentConfig> {
        Self::load_file(path)
    }

    /// 从文件加载 INI 配置
    /// Load INI config from file
    pub fn load_ini(path: &str) -> AgentResult<AgentConfig> {
        Self::load_file(path)
    }

    /// 从文件加载 RON 配置
    /// Load RON config from file
    pub fn load_ron(path: &str) -> AgentResult<AgentConfig> {
        Self::load_file(path)
    }

    /// 从文件加载 JSON5 配置
    /// Load JSON5 config from file
    pub fn load_json5(path: &str) -> AgentResult<AgentConfig> {
        Self::load_file(path)
    }

    /// 将配置序列化为字符串
    /// Serialize config to string
    pub fn to_string(config: &AgentConfig, format: ConfigFormat) -> AgentResult<String> {
        let content = match format {
            ConfigFormat::Yaml => serde_yaml::to_string(config).map_err(|e| {
                AgentConfigError::Serialization(format!("Failed to serialize to YAML: {}", e))
            })?,
            ConfigFormat::Toml => toml::to_string_pretty(config).map_err(|e| {
                AgentConfigError::Serialization(format!("Failed to serialize to TOML: {}", e))
            })?,
            ConfigFormat::Json => serde_json::to_string_pretty(config).map_err(|e| {
                AgentConfigError::Serialization(format!("Failed to serialize to JSON: {}", e))
            })?,
            ConfigFormat::Ini => {
                return Err(AgentConfigError::Serialization(
                    "INI serialization not directly supported. Use JSON, YAML, or TOML for saving."
                        .to_string(),
                ));
            }
            ConfigFormat::Ron => {
                return Err(AgentConfigError::Serialization(
                    "RON serialization not directly supported. Use JSON, YAML, or TOML for saving."
                        .to_string(),
                ));
            }
            ConfigFormat::Json5 => {
                // JSON5 is compatible with JSON for serialization purposes
                // JSON5 is compatible with JSON for serialization purposes
                serde_json::to_string_pretty(config).map_err(|e| {
                    AgentConfigError::Serialization(format!("Failed to serialize to JSON5: {}", e))
                })?
            }
        };

        Ok(content)
    }

    /// 将配置保存到文件
    /// Save configuration to file
    pub fn save_file(config: &AgentConfig, path: &str) -> AgentResult<()> {
        let format = ConfigFormat::from_extension(path).ok_or_else(|| {
            AgentConfigError::UnsupportedFormat(format!(
                "Unable to determine config format from file extension: {}",
                path
            ))
        })?;

        let content = Self::to_string(config, format)?;

        std::fs::write(path, content).map_err(AgentConfigError::Io)?;

        Ok(())
    }

    /// 加载多个配置文件
    /// Load multiple configuration files
    pub fn load_directory(dir_path: &str) -> AgentResult<Vec<AgentConfig>> {
        let mut configs = Vec::new();

        let entries = std::fs::read_dir(dir_path).map_err(AgentConfigError::Io)?;

        let supported_extensions = ["yaml", "yml", "toml", "json", "ini", "ron", "json5"];

        for entry in entries {
            let entry = entry.map_err(AgentConfigError::Io)?;

            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension().and_then(|e| e.to_str())
            {
                let ext_lower = ext.to_lowercase();
                if supported_extensions.contains(&ext_lower.as_str()) {
                    let path_str = path.to_string_lossy().to_string();
                    match Self::load_file(&path_str) {
                        Ok(config) => configs.push(config),
                        Err(e) => {
                            // 记录错误但继续加载其他文件
                            // Log error but continue loading other files
                            tracing::warn!("Failed to load config '{}': {}", path_str, e);
                        }
                    }
                }
            }
        }

        Ok(configs)
    }

    /// 合并多个配置 (后面的覆盖前面的)
    /// Merge configs (later ones override earlier ones)
    pub fn merge(base: AgentConfig, overlay: AgentConfig) -> AgentConfig {
        AgentConfig {
            id: if overlay.id.is_empty() {
                base.id
            } else {
                overlay.id
            },
            name: if overlay.name.is_empty() {
                base.name
            } else {
                overlay.name
            },
            description: overlay.description.or(base.description),
            agent_type: overlay.agent_type,
            components: ComponentsConfig {
                reasoner: overlay.components.reasoner.or(base.components.reasoner),
                memory: overlay.components.memory.or(base.components.memory),
                coordinator: overlay
                    .components
                    .coordinator
                    .or(base.components.coordinator),
            },
            capabilities: if overlay.capabilities.tags.is_empty() {
                base.capabilities
            } else {
                overlay.capabilities
            },
            custom: {
                let mut merged = base.custom;
                merged.extend(overlay.custom);
                merged
            },
            env_mappings: {
                let mut merged = base.env_mappings;
                merged.extend(overlay.env_mappings);
                merged
            },
            enabled: overlay.enabled,
            version: overlay.version.or(base.version),
        }
    }

    /// 从多个文件合并加载配置
    /// Load and merge config from multiple files
    pub fn load_merged_files(paths: &[&str]) -> AgentResult<AgentConfig> {
        load_merged(paths).map_err(|e| match e {
            ConfigError::Io(e) => AgentConfigError::Io(e),
            ConfigError::Parse(e) => AgentConfigError::Parse(e.to_string()),
            ConfigError::Serialization(e) => AgentConfigError::Serialization(e),
            ConfigError::UnsupportedFormat(e) => AgentConfigError::UnsupportedFormat(e),
            _ => AgentConfigError::Parse(e.to_string()),
        })
    }
}

use super::schema::ComponentsConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(
            ConfigFormat::from_extension("config.yaml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.yml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.toml"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.json"),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.ini"),
            Some(ConfigFormat::Ini)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.ron"),
            Some(ConfigFormat::Ron)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.json5"),
            Some(ConfigFormat::Json5)
        );
        assert_eq!(ConfigFormat::from_extension("config.txt"), None);
    }

    #[test]
    fn test_format_to_file_format() {
        assert_eq!(ConfigFormat::Yaml.to_file_format(), FileFormat::Yaml);
        assert_eq!(ConfigFormat::Toml.to_file_format(), FileFormat::Toml);
        assert_eq!(ConfigFormat::Json.to_file_format(), FileFormat::Json);
        assert_eq!(ConfigFormat::Ini.to_file_format(), FileFormat::Ini);
        assert_eq!(ConfigFormat::Ron.to_file_format(), FileFormat::Ron);
        assert_eq!(ConfigFormat::Json5.to_file_format(), FileFormat::Json5);
    }

    #[test]
    fn test_load_yaml_string() {
        let yaml = r#"
id: test-agent
name: Test Agent
type: llm
model: gpt-4
temperature: 0.8
"#;

        let config = ConfigLoader::from_yaml(yaml).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_load_json_string() {
        let json = r#"{
            "id": "test-agent",
            "name": "Test Agent",
            "type": "llm",
            "model": "gpt-4"
        }"#;

        let config = ConfigLoader::from_json(json).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_load_toml_string() {
        let toml = r#"
id = "test-agent"
name = "Test Agent"
type = "llm"
model = "gpt-4"
"#;

        let config = ConfigLoader::from_toml(toml).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_load_ini_string() {
        // INI format requires sections or dot notation for nesting
        // Using dot notation to match AgentConfig's flat structure
        // INI format requires sections or dot notation for nesting
        // Using dot notation to match AgentConfig's flat structure
        let ini = r#"
id = "test-agent"
name = "Test Agent"
type = "llm"
model = "gpt-4"
"#;

        let config = ConfigLoader::from_ini(ini).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_load_ron_string() {
        let ron = r#"
(
    id: "test-agent",
    name: "Test Agent",
    type: "llm",
    model: "gpt-4",
)
"#;

        let config = ConfigLoader::from_ron(ron).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_load_json5_string() {
        let json5 = r#"{
    // JSON5 allows comments
    id: "test-agent",
    name: "Test Agent",
    type: "llm",
    model: "gpt-4",
}
"#;

        let config = ConfigLoader::from_json5(json5).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_serialize_config() {
        let config = AgentConfig::new("my-agent", "My Agent");

        let yaml = ConfigLoader::to_string(&config, ConfigFormat::Yaml).unwrap();
        assert!(yaml.contains("my-agent"));

        let json = ConfigLoader::to_string(&config, ConfigFormat::Json).unwrap();
        assert!(json.contains("my-agent"));

        let toml = ConfigLoader::to_string(&config, ConfigFormat::Toml).unwrap();
        assert!(toml.contains("my-agent"));
    }

    #[test]
    fn test_merge_configs() {
        let base =
            AgentConfig::new("base-agent", "Base Agent").with_description("Base description");

        let overlay = AgentConfig {
            id: String::new(), // Empty, should use base
            name: "Override Name".to_string(),
            description: Some("Override description".to_string()),
            ..Default::default()
        };

        let merged = ConfigLoader::merge(base, overlay);
        assert_eq!(merged.id, "base-agent"); // From base
        assert_eq!(merged.id, "base-agent"); // From base
        assert_eq!(merged.name, "Override Name"); // From overlay
        assert_eq!(merged.name, "Override Name"); // From overlay
        assert_eq!(merged.description, Some("Override description".to_string())); // From overlay
        assert_eq!(merged.description, Some("Override description".to_string())); // From overlay
    }

    #[test]
    fn test_format_names() {
        assert_eq!(ConfigFormat::Yaml.name(), "yaml");
        assert_eq!(ConfigFormat::Toml.name(), "toml");
        assert_eq!(ConfigFormat::Json.name(), "json");
        assert_eq!(ConfigFormat::Ini.name(), "ini");
        assert_eq!(ConfigFormat::Ron.name(), "ron");
        assert_eq!(ConfigFormat::Json5.name(), "json5");
    }

    #[test]
    fn test_default_extensions() {
        assert_eq!(ConfigFormat::Yaml.default_extension(), "yml");
        assert_eq!(ConfigFormat::Toml.default_extension(), "toml");
        assert_eq!(ConfigFormat::Json.default_extension(), "json");
        assert_eq!(ConfigFormat::Ini.default_extension(), "ini");
        assert_eq!(ConfigFormat::Ron.default_extension(), "ron");
        assert_eq!(ConfigFormat::Json5.default_extension(), "json5");
    }
}
