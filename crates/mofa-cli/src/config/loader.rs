//! Configuration file discovery and loading

use super::AgentConfig;
use anyhow::Context;
use std::path::{Path, PathBuf};

/// Configuration file types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Json,
}

impl ConfigFormat {
    /// Detect format from file extension
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext.to_lowercase().as_str() {
                "yml" | "yaml" => Some(Self::Yaml),
                "json" => Some(Self::Json),
                _ => None,
            })
    }

    /// Get default filename for this format
    pub fn default_filename(&self) -> &str {
        match self {
            Self::Yaml => "agent.yml",
            Self::Json => "agent.json",
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
    pub fn read<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let format = ConfigFormat::from_path(path)
            .ok_or_else(|| anyhow::anyhow!("Unsupported config format: {}", path.display()))?;

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        Ok(Self {
            path: path.to_path_buf(),
            format,
            content,
        })
    }

    /// Parse the configuration file
    pub fn parse(&self) -> anyhow::Result<AgentConfig> {
        match self.format {
            ConfigFormat::Yaml => {
                serde_yaml::from_str(&self.content)
                    .context("Failed to parse YAML configuration")
            }
            ConfigFormat::Json => {
                serde_json::from_str(&self.content)
                    .context("Failed to parse JSON configuration")
            }
        }
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
        // Try current directory first
        for name in &["agent.yml", "agent.yaml", "agent.json"] {
            let path = PathBuf::from(name);
            if path.exists() {
                return Some(path);
            }
        }

        // Search upward
        let mut current = std::env::current_dir().ok()?;
        loop {
            for name in &["agent.yml", "agent.yaml", "agent.json"] {
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
    pub fn load<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<AgentConfig> {
        let path = path.as_ref();

        // If path is a directory, look for config files
        let config_path = if path.is_dir() {
            path.join("agent.yml")
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
    ) -> anyhow::Result<AgentConfig> {
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
            ConfigFormat::from_path(Path::new("agent.json")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("agent.txt")),
            None
        );
    }

    #[test]
    fn test_default_filename() {
        assert_eq!(ConfigFormat::Yaml.default_filename(), "agent.yml");
        assert_eq!(ConfigFormat::Json.default_filename(), "agent.json");
    }
}
