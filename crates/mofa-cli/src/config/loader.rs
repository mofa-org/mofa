//! Configuration file discovery and loading
//!
//! Supports multiple configuration formats: YAML, TOML, JSON, INI, RON, JSON5

use crate::CliError;
use super::AgentConfig;
use mofa_kernel::config::{detect_format, from_str};
use std::path::{Path, PathBuf};

/// Configuration file types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Toml,
    Json,
    Ini,
    Ron,
    Json5,
}

impl ConfigFormat {
    /// Detect format from file extension
    pub fn from_path(path: &Path) -> Option<Self> {
        match detect_format(path.to_str().unwrap_or("")) {
            Ok(config::FileFormat::Yaml) => Some(Self::Yaml),
            Ok(config::FileFormat::Toml) => Some(Self::Toml),
            Ok(config::FileFormat::Json) => Some(Self::Json),
            Ok(config::FileFormat::Ini) => Some(Self::Ini),
            Ok(config::FileFormat::Ron) => Some(Self::Ron),
            Ok(config::FileFormat::Json5) => Some(Self::Json5),
            _ => None,
        }
    }

    /// Get default filename for this format
    pub fn default_filename(&self) -> &str {
        match self {
            Self::Yaml => "agent.yml",
            Self::Toml => "agent.toml",
            Self::Json => "agent.json",
            Self::Ini => "agent.ini",
            Self::Ron => "agent.ron",
            Self::Json5 => "agent.json5",
        }
    }

    /// Get file extensions for this format
    pub fn extensions(&self) -> &[&str] {
        match self {
            Self::Yaml => &["yml", "yaml"],
            Self::Toml => &["toml"],
            Self::Json => &["json"],
            Self::Ini => &["ini"],
            Self::Ron => &["ron"],
            Self::Json5 => &["json5"],
        }
    }
}

/// Configuration file wrapper
#[derive(Debug, Clone)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub format: ConfigFormat,
    pub content: String,
}

impl ConfigFile {
    /// Read a configuration file
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, CliError> {
        let path = path.as_ref();
        let format = ConfigFormat::from_path(path).ok_or_else(|| {
            CliError::ConfigError(format!("Unsupported config format: {}", path.display()))
        })?;

        let content = std::fs::read_to_string(path).map_err(|e| {
            CliError::ConfigError(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        Ok(Self {
            path: path.to_path_buf(),
            format,
            content,
        })
    }

    /// Parse the configuration file
    pub fn parse(&self) -> Result<AgentConfig, CliError> {
        let file_format = match self.format {
            ConfigFormat::Yaml => config::FileFormat::Yaml,
            ConfigFormat::Toml => config::FileFormat::Toml,
            ConfigFormat::Json => config::FileFormat::Json,
            ConfigFormat::Ini => config::FileFormat::Ini,
            ConfigFormat::Ron => config::FileFormat::Ron,
            ConfigFormat::Json5 => config::FileFormat::Json5,
        };

        from_str(&self.content, file_format).map_err(|e| {
            CliError::ConfigError(format!("Failed to parse configuration: {}", e))
        })
    }
}

/// Configuration loader with discovery support
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    /// Additional search paths
    search_paths: Vec<PathBuf>,
}

impl ConfigLoader {
    /// Create a new configuration loader
    pub fn new() -> Self {
        Self {
            search_paths: Vec::new(),
        }
    }

    /// Add a search path
    pub fn add_search_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.search_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Find a configuration file by searching upward from current directory
    pub fn find_config(&self) -> Option<PathBuf> {
        let supported_filenames = [
            "agent.yml",
            "agent.yaml",
            "agent.toml",
            "agent.json",
            "agent.ini",
            "agent.ron",
            "agent.json5",
        ];

        // Try current directory first
        for name in &supported_filenames {
            let path = PathBuf::from(name);
            if path.exists() {
                return Some(path);
            }
        }

        // Search upward
        let mut current = std::env::current_dir().ok()?;
        loop {
            for name in &supported_filenames {
                let target = current.join(name);
                if target.exists() {
                    return Some(target);
                }
            }

            if !current.pop() {
                break;
            }
        }

        None
    }

    /// Load configuration from a specific path
    pub fn load<P: AsRef<Path>>(&self, path: P) -> Result<AgentConfig, CliError> {
        let path = path.as_ref();

        // If path is a directory, look for config files
        let config_path = if path.is_dir() {
            // Try to find a config file in the directory
            let supported_filenames = [
                "agent.yml",
                "agent.yaml",
                "agent.toml",
                "agent.json",
                "agent.ini",
                "agent.ron",
                "agent.json5",
            ];

            let mut found = None;
            for name in &supported_filenames {
                let target = path.join(name);
                if target.exists() {
                    found = Some(target);
                    break;
                }
            }

            found.ok_or_else(|| {
                CliError::ConfigError(format!(
                    "No config file found in directory: {}",
                    path.display()
                ))
            })?
        } else {
            path.to_path_buf()
        };

        let file = ConfigFile::read(&config_path)?;
        file.parse()
    }

    /// Load configuration with merge from file, environment, and overrides
    pub fn load_merged(
        &self,
        file_path: Option<PathBuf>,
        _env_prefix: Option<&str>,
    ) -> Result<AgentConfig, CliError> {
        let config = if let Some(path) = file_path {
            self.load(&path)?
        } else if let Some(found) = self.find_config() {
            self.load(&found)?
        } else {
            AgentConfig::default()
        };

        Ok(config)
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_format_detection() {
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.yml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.yaml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.toml")),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.json")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.ini")),
            Some(ConfigFormat::Ini)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.ron")),
            Some(ConfigFormat::Ron)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.json5")),
            Some(ConfigFormat::Json5)
        );
        assert_eq!(ConfigFormat::from_path(Path::new("agent.txt")), None);
    }

    #[test]
    fn test_default_filename() {
        assert_eq!(ConfigFormat::Yaml.default_filename(), "agent.yml");
        assert_eq!(ConfigFormat::Toml.default_filename(), "agent.toml");
        assert_eq!(ConfigFormat::Json.default_filename(), "agent.json");
        assert_eq!(ConfigFormat::Ini.default_filename(), "agent.ini");
        assert_eq!(ConfigFormat::Ron.default_filename(), "agent.ron");
        assert_eq!(ConfigFormat::Json5.default_filename(), "agent.json5");
    }

    #[test]
    fn test_extensions() {
        assert_eq!(ConfigFormat::Yaml.extensions(), &["yml", "yaml"]);
        assert_eq!(ConfigFormat::Toml.extensions(), &["toml"]);
        assert_eq!(ConfigFormat::Json.extensions(), &["json"]);
        assert_eq!(ConfigFormat::Ini.extensions(), &["ini"]);
        assert_eq!(ConfigFormat::Ron.extensions(), &["ron"]);
        assert_eq!(ConfigFormat::Json5.extensions(), &["json5"]);
    }
}
