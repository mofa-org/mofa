//! Output formatting module
//!
//! Provides various output formats for CLI commands including JSON, tables, and progress indicators.

use crate::CliError;

mod json;
mod progress;
mod table;

pub use json::JsonOutput;
pub use table::Table;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output
    #[default]
    Text,
    /// JSON output for automation
    Json,
    /// Table-formatted output
    Table,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Json => write!(f, "json"),
            Self::Table => write!(f, "table"),
        }
    }
}

/// Result wrapper for CLI commands that supports multiple output formats
pub enum CommandResult {
    /// No output (success only)
    Empty,
    /// Simple message
    Message(String),
    /// Data that can be formatted as JSON or table
    Data(Box<dyn JsonOutput>),
}

impl CommandResult {
    /// Create an empty result
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Create a message result
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    /// Create a data result
    pub fn data<T: JsonOutput + 'static>(data: T) -> Self {
        Self::Data(Box::new(data))
    }
}

/// Format result for display
pub fn format_result<T: JsonOutput>(result: &T, format: OutputFormat) -> Result<String, CliError> {
    match format {
        OutputFormat::Text => Ok(format_text(result)),
        OutputFormat::Json => Ok(format_json(result)),
        OutputFormat::Table => Ok(format_table(result)),
    }
}

fn format_text<T: JsonOutput>(result: &T) -> String {
    // Default text representation
    result.to_json().to_string()
}

fn format_json<T: JsonOutput>(result: &T) -> String {
    result.to_json().to_string()
}

fn format_table<T: JsonOutput>(result: &T) -> String {
    // Try to convert to table format
    let json = result.to_json();
    if let Some(arr) = json.as_array()
        && !arr.is_empty()
    {
        return Table::from_json_array(arr).to_string();
    }
    // Fallback to JSON
    json.to_string()
}
