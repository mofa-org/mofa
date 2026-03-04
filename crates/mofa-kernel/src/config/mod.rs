//! Global Configuration System
//!
//! This module provides a global configuration loading abstraction that supports
//! multiple configuration formats: YAML, TOML, JSON, INI, RON, JSON5.
//!
//! ## Features
//!
//! - Auto-detection of format from file extension
//! - Environment variable substitution (`${VAR}` and `$VAR` syntax)
//! - Configuration merging from multiple sources
//! - Support for all major configuration formats

use config::{Config as Cfg, Environment, File, FileFormat};
use regex::Regex;
use serde::de::DeserializeOwned;
use std::path::Path;

/// Configuration format detection error
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parsing error: {0}")]
    Parse(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Result type for config operations
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Detect configuration format from file extension
///
/// # Supported Extensions
///
/// - YAML: `.yaml`, `.yml`
/// - TOML: `.toml`
/// - JSON: `.json`
/// - INI: `.ini`
/// - RON: `.ron`
/// - JSON5: `.json5`
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::config::detect_format;
///
/// let format = detect_format("config.toml").unwrap();
/// assert_eq!(format, FileFormat::Toml);
/// ```
pub fn detect_format(path: &str) -> ConfigResult<FileFormat> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| ConfigError::UnsupportedFormat("No file extension found".to_string()))?;

    match ext.to_lowercase().as_str() {
        "yaml" | "yml" => Ok(FileFormat::Yaml),
        "toml" => Ok(FileFormat::Toml),
        "json" => Ok(FileFormat::Json),
        "ini" => Ok(FileFormat::Ini),
        "ron" => Ok(FileFormat::Ron),
        "json5" => Ok(FileFormat::Json5),
        _ => Err(ConfigError::UnsupportedFormat(ext.to_string())),
    }
}

/// Substitute environment variables in a string
///
/// Supports both `${VAR_NAME}` and `$VAR_NAME` syntax. Uses regex to find and
/// replace all environment variable references with their values.
///
/// # Syntax
///
/// - `${VAR_NAME}` - Environment variable in braces (preferred)
/// - `$VAR_NAME` - Environment variable without braces
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::config::substitute_env_vars;
///
/// std::env::set_var("DATABASE_URL", "postgres://localhost/mydb");
/// let result = substitute_env_vars("db_url: ${DATABASE_URL}");
/// assert_eq!(result, "db_url: postgres://localhost/mydb");
/// ```
pub fn substitute_env_vars(content: &str) -> String {
    let mut result = content.to_string();

    // Match ${VAR_NAME} pattern (braced syntax - higher priority)
    let re_braced = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();
    result = re_braced
        .replace_all(&result, |caps: &regex::Captures| {
            let var_name = &caps[1];
            std::env::var(var_name).unwrap_or_else(|_| caps[0].to_string())
        })
        .to_string();

    // Match $VAR_NAME pattern (non-braced, but only if not already substituted)
    // This regex matches $ followed by a valid identifier name
    let re_simple = Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)\b").unwrap();
    result = re_simple
        .replace_all(&result, |caps: &regex::Captures| {
            let var_name = &caps[1];
            std::env::var(var_name).unwrap_or_else(|_| caps[0].to_string())
        })
        .to_string();

    result
}

/// Load configuration from a file
///
/// Automatically detects the format from the file extension and performs
/// environment variable substitution on the loaded content.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::config::load_config;
///
/// #[derive(serde::Deserialize)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// let config: MyConfig = load_config("config.toml")?;
/// ```
pub fn load_config<T>(path: &str) -> ConfigResult<T>
where
    T: DeserializeOwned,
{
    let format = detect_format(path)?;
    let content = std::fs::read_to_string(path)?;

    // Substitute environment variables first
    let substituted_content = substitute_env_vars(&content);

    // Parse using the config crate
    let config = Cfg::builder()
        .add_source(File::from_str(&substituted_content, format))
        .build()
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    // Deserialize to target type
    config
        .try_deserialize()
        .map_err(|e| ConfigError::Serialization(e.to_string()))
}

/// Load configuration from a string with explicit format
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::config::{from_str, FileFormat};
///
/// let toml = r#"
/// name = "test"
/// port = 8080
/// "#;
///
/// #[derive(serde::Deserialize)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// let config: MyConfig = from_str(toml, FileFormat::Toml)?;
/// ```
pub fn from_str<T>(content: &str, format: FileFormat) -> ConfigResult<T>
where
    T: DeserializeOwned,
{
    let substituted_content = substitute_env_vars(content);

    let config = Cfg::builder()
        .add_source(File::from_str(&substituted_content, format))
        .build()
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    config
        .try_deserialize()
        .map_err(|e| ConfigError::Serialization(e.to_string()))
}

/// Merge multiple configuration sources
///
/// Later sources override earlier ones. This is useful for layering
/// configurations (e.g., defaults -> file -> environment).
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::config::merge_configs;
///
/// let base = r#"{ "name": "base", "port": 8080 }"#;
/// let override = r#"{ "port": 9090 }"#;
///
/// #[derive(serde::Deserialize, Debug)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// let config: MyConfig = merge_configs(&[base, override])?;
/// // config.name == "base", config.port == 9090
/// ```
pub fn merge_configs<T>(sources: &[(&str, FileFormat)]) -> ConfigResult<T>
where
    T: DeserializeOwned,
{
    let mut builder = Cfg::builder();

    for (content, format) in sources {
        let substituted = substitute_env_vars(content);
        builder = builder.add_source(File::from_str(&substituted, *format));
    }

    let config = builder
        .build()
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    config
        .try_deserialize()
        .map_err(|e| ConfigError::Serialization(e.to_string()))
}

/// Load configuration from multiple files with later files overriding earlier ones
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::config::load_merged;
///
/// #[derive(serde::Deserialize, Debug)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// let config: MyConfig = load_merged(&["defaults.toml", "local.toml"])?;
/// ```
pub fn load_merged<T>(paths: &[&str]) -> ConfigResult<T>
where
    T: DeserializeOwned,
{
    let mut builder = Cfg::builder();

    for path in paths {
        let format = detect_format(path)?;
        let content = std::fs::read_to_string(path)?;
        let substituted = substitute_env_vars(&content);
        builder = builder.add_source(File::from_str(&substituted, format));
    }

    let config = builder
        .build()
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    config
        .try_deserialize()
        .map_err(|e| ConfigError::Serialization(e.to_string()))
}

/// Load configuration with environment variable overrides
///
/// Environment variables should be prefixed with the given prefix and use
/// double underscores `__` to represent nesting.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::config::load_with_env;
///
/// // For config struct with field "database.url", env var would be "APP_DATABASE__URL"
/// let config: MyConfig = load_with_env("config.toml", "APP")?;
/// ```
pub fn load_with_env<T>(path: &str, env_prefix: &str) -> ConfigResult<T>
where
    T: DeserializeOwned,
{
    let format = detect_format(path)?;
    let content = std::fs::read_to_string(path)?;

    let substituted = substitute_env_vars(&content);

    let config = Cfg::builder()
        .add_source(File::from_str(&substituted, format))
        .add_source(Environment::with_prefix(env_prefix).separator("__"))
        .build()
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    config
        .try_deserialize()
        .map_err(|e| ConfigError::Serialization(e.to_string()))
}

#[cfg(all(test, feature = "config"))]
mod unit_tests {
    use super::*;

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format("config.yaml").unwrap(), FileFormat::Yaml);
        assert_eq!(detect_format("config.yml").unwrap(), FileFormat::Yaml);
        assert_eq!(detect_format("config.toml").unwrap(), FileFormat::Toml);
        assert_eq!(detect_format("config.json").unwrap(), FileFormat::Json);
        assert_eq!(detect_format("config.ini").unwrap(), FileFormat::Ini);
        assert_eq!(detect_format("config.ron").unwrap(), FileFormat::Ron);
        assert_eq!(detect_format("config.json5").unwrap(), FileFormat::Json5);
        assert!(detect_format("config.txt").is_err());
    }

    #[test]
    fn test_from_str_toml() {
        let toml = r#"
id = "test-agent"
name = "Test Agent"
model = "gpt-4"
"#;

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct TestConfig {
            id: String,
            name: String,
            model: String,
        }

        let config: TestConfig = from_str(toml, FileFormat::Toml).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
        assert_eq!(config.model, "gpt-4");
    }

    #[test]
    fn test_from_str_json() {
        let json = r#"
{
    "id": "test-agent",
    "name": "Test Agent"
}
"#;

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct TestConfig {
            id: String,
            name: String,
        }

        let config: TestConfig = from_str(json, FileFormat::Json).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_from_str_yaml() {
        let yaml = r#"
id: test-agent
name: Test Agent
"#;

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct TestConfig {
            id: String,
            name: String,
        }

        let config: TestConfig = from_str(yaml, FileFormat::Yaml).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_from_str_ini() {
        let ini = r#"
default.id = "test-agent"
default.name = "Test Agent"
"#;

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct IniConfig {
            default: IniSection,
        }

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct IniSection {
            id: String,
            name: String,
        }

        let config: IniConfig = from_str(ini, FileFormat::Ini).unwrap();
        assert_eq!(config.default.id, "test-agent");
        assert_eq!(config.default.name, "Test Agent");
    }

    #[test]
    fn test_from_str_ron() {
        let ron = r#"
(
    id: "test-agent",
    name: "Test Agent",
)
"#;

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct TestConfig {
            id: String,
            name: String,
        }

        let config: TestConfig = from_str(ron, FileFormat::Ron).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_from_str_json5() {
        let json5 = r#"
{
    // JSON5 comment
    id: "test-agent",
    name: "Test Agent",
}
"#;

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct TestConfig {
            id: String,
            name: String,
        }

        let config: TestConfig = from_str(json5, FileFormat::Json5).unwrap();
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.name, "Test Agent");
    }

    #[test]
    fn test_merge_configs() {
        let base = r#"
{
    "id": "base-agent",
    "name": "Base Name",
    "model": "gpt-3.5"
}
"#;

        let override_config = r#"
{
    "model": "gpt-4"
}
"#;

        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
        struct TestConfig {
            id: String,
            name: String,
            model: String,
        }

        let config: TestConfig = merge_configs(&[
            (base, FileFormat::Json),
            (override_config, FileFormat::Json),
        ])
        .unwrap();
        assert_eq!(config.id, "base-agent");
        assert_eq!(config.name, "Base Name");
        assert_eq!(config.model, "gpt-4");
    }
}

// Include integration tests
#[cfg(test)]
mod tests;
