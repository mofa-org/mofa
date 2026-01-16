//! 配置加载器
//!
//! 支持 YAML, TOML, JSON 格式的配置加载

use super::schema::AgentConfig;
use crate::agent::error::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 配置格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigFormat {
    /// YAML 格式
    Yaml,
    /// TOML 格式
    Toml,
    /// JSON 格式
    Json,
}

impl ConfigFormat {
    /// 从文件扩展名推断格式
    pub fn from_extension(path: &str) -> Option<Self> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("yaml") | Some("yml") => Some(Self::Yaml),
            Some("toml") => Some(Self::Toml),
            Some("json") => Some(Self::Json),
            _ => None,
        }
    }

    /// 获取格式名称
    pub fn name(&self) -> &str {
        match self {
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Json => "json",
        }
    }
}

/// 配置加载器
///
/// 支持从文件或字符串加载配置
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_runtime::agent::config::{ConfigLoader, ConfigFormat};
///
/// // 从 YAML 字符串加载
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
/// let config = ConfigLoader::load_file("agent.yaml")?;
/// ```
pub struct ConfigLoader;

impl ConfigLoader {
    /// 从字符串加载配置
    pub fn from_str(content: &str, format: ConfigFormat) -> AgentResult<AgentConfig> {
        let config = match format {
            ConfigFormat::Yaml => serde_yaml::from_str(content).map_err(|e| {
                AgentError::ConfigError(format!("Failed to parse YAML: {}", e))
            })?,
            ConfigFormat::Toml => toml::from_str(content).map_err(|e| {
                AgentError::ConfigError(format!("Failed to parse TOML: {}", e))
            })?,
            ConfigFormat::Json => serde_json::from_str(content).map_err(|e| {
                AgentError::ConfigError(format!("Failed to parse JSON: {}", e))
            })?,
        };

        Ok(config)
    }

    /// 从 YAML 字符串加载
    pub fn from_yaml(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Yaml)
    }

    /// 从 TOML 字符串加载
    pub fn from_toml(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Toml)
    }

    /// 从 JSON 字符串加载
    pub fn from_json(content: &str) -> AgentResult<AgentConfig> {
        Self::from_str(content, ConfigFormat::Json)
    }

    /// 从文件加载配置 (自动检测格式)
    pub fn load_file(path: &str) -> AgentResult<AgentConfig> {
        let format = ConfigFormat::from_extension(path).ok_or_else(|| {
            AgentError::ConfigError(format!(
                "Unable to determine config format from file extension: {}",
                path
            ))
        })?;

        let content = std::fs::read_to_string(path).map_err(|e| {
            AgentError::ConfigError(format!("Failed to read config file '{}': {}", path, e))
        })?;

        let config = Self::from_str(&content, format)?;

        // 验证配置
        config.validate().map_err(|errors| {
            AgentError::ConfigError(format!("Config validation failed: {}", errors.join(", ")))
        })?;

        Ok(config)
    }

    /// 从文件加载 YAML 配置
    pub fn load_yaml(path: &str) -> AgentResult<AgentConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AgentError::ConfigError(format!("Failed to read YAML file '{}': {}", path, e))
        })?;
        Self::from_yaml(&content)
    }

    /// 从文件加载 TOML 配置
    pub fn load_toml(path: &str) -> AgentResult<AgentConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AgentError::ConfigError(format!("Failed to read TOML file '{}': {}", path, e))
        })?;
        Self::from_toml(&content)
    }

    /// 从文件加载 JSON 配置
    pub fn load_json(path: &str) -> AgentResult<AgentConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AgentError::ConfigError(format!("Failed to read JSON file '{}': {}", path, e))
        })?;
        Self::from_json(&content)
    }

    /// 将配置序列化为字符串
    pub fn to_string(config: &AgentConfig, format: ConfigFormat) -> AgentResult<String> {
        let content = match format {
            ConfigFormat::Yaml => serde_yaml::to_string(config).map_err(|e| {
                AgentError::ConfigError(format!("Failed to serialize to YAML: {}", e))
            })?,
            ConfigFormat::Toml => toml::to_string_pretty(config).map_err(|e| {
                AgentError::ConfigError(format!("Failed to serialize to TOML: {}", e))
            })?,
            ConfigFormat::Json => serde_json::to_string_pretty(config).map_err(|e| {
                AgentError::ConfigError(format!("Failed to serialize to JSON: {}", e))
            })?,
        };

        Ok(content)
    }

    /// 将配置保存到文件
    pub fn save_file(config: &AgentConfig, path: &str) -> AgentResult<()> {
        let format = ConfigFormat::from_extension(path).ok_or_else(|| {
            AgentError::ConfigError(format!(
                "Unable to determine config format from file extension: {}",
                path
            ))
        })?;

        let content = Self::to_string(config, format)?;

        std::fs::write(path, content).map_err(|e| {
            AgentError::ConfigError(format!("Failed to write config file '{}': {}", path, e))
        })?;

        Ok(())
    }

    /// 加载多个配置文件
    pub fn load_directory(dir_path: &str) -> AgentResult<Vec<AgentConfig>> {
        let mut configs = Vec::new();

        let entries = std::fs::read_dir(dir_path).map_err(|e| {
            AgentError::ConfigError(format!("Failed to read directory '{}': {}", dir_path, e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                AgentError::ConfigError(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "yaml" || ext_lower == "yml" || ext_lower == "toml" || ext_lower == "json" {
                        let path_str = path.to_string_lossy().to_string();
                        match Self::load_file(&path_str) {
                            Ok(config) => configs.push(config),
                            Err(e) => {
                                // 记录错误但继续加载其他文件
                                tracing::warn!("Failed to load config '{}': {}", path_str, e);
                            }
                        }
                    }
                }
        }

        Ok(configs)
    }

    /// 合并多个配置 (后面的覆盖前面的)
    pub fn merge(base: AgentConfig, overlay: AgentConfig) -> AgentConfig {
        AgentConfig {
            id: if overlay.id.is_empty() { base.id } else { overlay.id },
            name: if overlay.name.is_empty() { base.name } else { overlay.name },
            description: overlay.description.or(base.description),
            agent_type: overlay.agent_type,
            components: ComponentsConfig {
                reasoner: overlay.components.reasoner.or(base.components.reasoner),
                memory: overlay.components.memory.or(base.components.memory),
                coordinator: overlay.components.coordinator.or(base.components.coordinator),
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
}

use super::schema::ComponentsConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(ConfigFormat::from_extension("config.yaml"), Some(ConfigFormat::Yaml));
        assert_eq!(ConfigFormat::from_extension("config.yml"), Some(ConfigFormat::Yaml));
        assert_eq!(ConfigFormat::from_extension("config.toml"), Some(ConfigFormat::Toml));
        assert_eq!(ConfigFormat::from_extension("config.json"), Some(ConfigFormat::Json));
        assert_eq!(ConfigFormat::from_extension("config.txt"), None);
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
    fn test_serialize_config() {
        let config = AgentConfig::new("my-agent", "My Agent");

        let mut yaml = ConfigLoader::to_string(&config, ConfigFormat::Yaml).unwrap();
        assert!(yaml.contains("my-agent"));

        let mut json = ConfigLoader::to_string(&config, ConfigFormat::Json).unwrap();
        assert!(json.contains("my-agent"));
    }

    #[test]
    fn test_merge_configs() {
        let base = AgentConfig::new("base-agent", "Base Agent")
            .with_description("Base description");

        let overlay = AgentConfig {
            id: String::new(), // Empty, should use base
            name: "Override Name".to_string(),
            description: Some("Override description".to_string()),
            ..Default::default()
        };

        let merged = ConfigLoader::merge(base, overlay);
        assert_eq!(merged.id, "base-agent"); // From base
        assert_eq!(merged.name, "Override Name"); // From overlay
        assert_eq!(merged.description, Some("Override description".to_string())); // From overlay
    }
}
