use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Plugin error: {0}")]
    PluginError(String),

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("State error: {0}")]
    StateError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Dialoguer error: {0}")]
    DialoguerError(#[from] dialoguer::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Parse integer error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("Parse float error: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),

    #[error("Mofa SDK error: {0}")]
    Sdk(String),

    #[error("Initialization error: {0}")]
    InitError(String),

    #[error("{0}")]
    Other(String),
}

// Ensure it can be easily created from strings
impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        CliError::Other(s.to_string())
    }
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError::Other(s)
    }
}
