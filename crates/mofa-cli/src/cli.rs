//! CLI command definitions using clap

use crate::output::OutputFormat;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// MoFA CLI - Build and manage AI agents
#[derive(Parser)]
#[command(name = "mofa")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Launch TUI (Terminal User Interface) mode
    #[arg(short, long, global = false)]
    pub tui: bool,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format (text, json, table)
    #[arg(short = 'o', long, global = true)]
    pub output: Option<OutputFormat>,

    /// Configuration file path
    #[arg(short = 'c', long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Create a new MoFA agent project
    New {
        /// Project name
        name: String,

        /// Project template
        #[arg(short, long, default_value = "basic")]
        template: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Initialize MoFA in an existing project
    Init {
        /// Project directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Build the agent project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,

        /// Target features
        #[arg(short, long)]
        features: Option<String>,
    },

    /// Run the agent
    Run {
        /// Agent configuration file
        #[arg(short, long, default_value = "agent.yml")]
        config: PathBuf,

        /// Enable dora runtime
        #[arg(long)]
        dora: bool,
    },

    /// Run a dora dataflow
    #[cfg(feature = "dora")]
    Dataflow {
        /// Dataflow YAML file
        file: PathBuf,

        /// Use uv for Python nodes
        #[arg(long)]
        uv: bool,
    },

    /// Generate project files
    Generate {
        #[command(subcommand)]
        what: GenerateCommands,
    },

    /// Show information about MoFA
    Info,

    /// Database management commands
    Db {
        #[command(subcommand)]
        action: DbCommands,
    },

    /// Agent management commands
    #[command(subcommand)]
    Agent(AgentCommands),

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },

    /// Plugin management
    Plugin {
        #[command(subcommand)]
        action: PluginCommands,
    },

    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionCommands,
    },

    /// Tool management
    Tool {
        #[command(subcommand)]
        action: ToolCommands,
    },

    /// Gateway management (OpenAI-compatible API)
    Gateway {
        #[command(subcommand)]
        action: GatewayCommands,
    },
}

/// Generate subcommands
#[derive(Subcommand)]
pub enum GenerateCommands {
    /// Generate agent configuration
    Config {
        /// Output file
        #[arg(short, long, default_value = "agent.yml")]
        output: PathBuf,
    },

    /// Generate dataflow configuration
    Dataflow {
        /// Output file
        #[arg(short, long, default_value = "dataflow.yml")]
        output: PathBuf,
    },
}

/// Database management subcommands
#[derive(Subcommand)]
pub enum DbCommands {
    /// Initialize persistence database tables
    Init {
        /// Database type
        #[arg(short = 't', long, value_enum)]
        db_type: DatabaseType,

        /// Output SQL to file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Database connection URL (executes SQL directly)
        #[arg(short = 'u', long)]
        database_url: Option<String>,
    },

    /// Show migration SQL for a database type
    Schema {
        /// Database type
        #[arg(short = 't', long, value_enum)]
        db_type: DatabaseType,
    },
}

/// Database type
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DatabaseType {
    /// PostgreSQL database
    Postgres,
    /// MySQL/MariaDB database
    Mysql,
    /// SQLite database
    Sqlite,
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseType::Postgres => write!(f, "postgres"),
            DatabaseType::Mysql => write!(f, "mysql"),
            DatabaseType::Sqlite => write!(f, "sqlite"),
        }
    }
}

/// Agent management subcommands
#[derive(Subcommand)]
pub enum AgentCommands {
    /// Create a new agent (interactive wizard)
    Create {
        /// Run in non-interactive mode
        #[arg(long)]
        non_interactive: bool,

        /// Configuration file path
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Start an agent
    Start {
        /// Agent ID
        agent_id: String,

        /// Configuration file path
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Agent factory type (use `mofa agent status` to inspect available factories)
        #[arg(long = "type")]
        factory_type: Option<String>,

        /// Run as daemon
        #[arg(long)]
        daemon: bool,
    },

    /// Stop a running agent
    Stop {
        /// Agent ID
        agent_id: String,
    },

    /// Restart an agent
    Restart {
        /// Agent ID
        agent_id: String,

        /// Configuration file path
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Show agent status
    Status {
        /// Agent ID (omit to list all)
        agent_id: Option<String>,
    },

    /// List all agents
    List {
        /// Show only running agents
        #[arg(long)]
        running: bool,

        /// Show all agents
        #[arg(long)]
        all: bool,
    },
}

/// Configuration management subcommands
#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Get or set a configuration value
    #[command(subcommand)]
    Value(ConfigValueCommands),

    /// List all configuration values
    List,

    /// Validate configuration
    Validate,

    /// Show configuration file path
    Path,
}

/// Configuration value subcommands
#[derive(Subcommand)]
pub enum ConfigValueCommands {
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Configuration value
        value: String,
    },

    /// Unset a configuration value
    Unset {
        /// Configuration key
        key: String,
    },
}

/// Plugin management subcommands
#[derive(Subcommand)]
pub enum PluginCommands {
    /// List plugins
    List {
        /// Show installed plugins only
        #[arg(long)]
        installed: bool,

        /// Show available plugins
        #[arg(long)]
        available: bool,
    },

    /// Show plugin information
    Info {
        /// Plugin name
        name: String,
    },

    /// Uninstall a plugin
    Uninstall {
        /// Plugin name
        name: String,

        /// Force removal without confirmation
        #[arg(long)]
        force: bool,
    },
}

/// Session management subcommands
#[derive(Subcommand)]
pub enum SessionCommands {
    /// List sessions
    List {
        /// Filter by agent ID
        #[arg(short, long)]
        agent: Option<String>,

        /// Limit number of results
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },

    /// Show session details
    Show {
        /// Session ID
        session_id: String,

        /// Output format
        #[arg(short = 'o', long)]
        format: Option<SessionFormat>,
    },

    /// Delete a session
    Delete {
        /// Session ID
        session_id: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Export session data
    Export {
        /// Session ID
        session_id: String,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,

        /// Export format
        #[arg(short, long)]
        format: ExportFormat,
    },
}

/// Session output format
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SessionFormat {
    /// JSON format
    Json,
    /// Table format
    Table,
    /// YAML format
    Yaml,
}

/// Export format
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ExportFormat {
    /// JSON format
    Json,
    /// YAML format
    Yaml,
}

impl std::fmt::Display for SessionFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionFormat::Json => write!(f, "json"),
            SessionFormat::Table => write!(f, "table"),
            SessionFormat::Yaml => write!(f, "yaml"),
        }
    }
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportFormat::Json => write!(f, "json"),
            ExportFormat::Yaml => write!(f, "yaml"),
        }
    }
}

/// Tool management subcommands
#[derive(Subcommand)]
pub enum ToolCommands {
    /// List tools
    List {
        /// Show available tools
        #[arg(long)]
        available: bool,

        /// Show enabled tools
        #[arg(long)]
        enabled: bool,
    },

    /// Show tool information
    Info {
        /// Tool name
        name: String,
    },
}

/// Gateway management subcommands
#[derive(Subcommand)]
pub enum GatewayCommands {
    /// Serve OpenAI-compatible gateway endpoint
    Serve {
        /// Gateway host address
        #[arg(long, default_value = "0.0.0.0")]
        host: String,

        /// Gateway port
        #[arg(long, default_value_t = 8000)]
        port: u16,

        /// Backend spec (repeatable): `url`, `url@weight`, or `name=url@weight`
        #[arg(long = "backend", required = true)]
        backends: Vec<String>,

        /// Global requests-per-minute limit
        #[arg(long, default_value_t = 120)]
        rpm: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_legacy_output_flag_parses_for_backwards_compatibility() {
        let parsed = Cli::try_parse_from(["mofa", "--output", "json", "info"]);
        assert!(parsed.is_ok(), "legacy --output flag should parse");
    }

    #[test]
    fn test_agent_stop_force_persisted_stop_flag_parses() {
        let parsed =
            Cli::try_parse_from(["mofa", "agent", "stop", "agent-1", "--force-persisted-stop"]);
        assert!(
            parsed.is_ok(),
            "agent stop should accept --force-persisted-stop"
        );
    }

    #[test]
    fn test_session_show_format_json_parses() {
        let parsed = Cli::try_parse_from(["mofa", "session", "show", "s1", "--format", "json"]);
        assert!(parsed.is_ok(), "session show --format json should parse");
    }

    #[test]
    fn test_session_show_legacy_short_output_flag_still_parses() {
        let parsed = Cli::try_parse_from(["mofa", "session", "show", "s1", "-o", "json"]);
        assert!(parsed.is_ok(), "session show -o json should still parse");
    }

    #[test]
    fn test_session_export_output_and_format_parse_together() {
        let parsed = Cli::try_parse_from([
            "mofa",
            "session",
            "export",
            "s1",
            "--output",
            "/tmp/s1.json",
            "--format",
            "json",
        ]);
        assert!(
            parsed.is_ok(),
            "session export --output ... --format ... should parse"
        );
    }

    #[test]
    fn test_session_export_legacy_short_output_flag_still_parses() {
        let parsed = Cli::try_parse_from([
            "mofa",
            "session",
            "export",
            "s1",
            "-o",
            "/tmp/s1.json",
            "--format",
            "json",
        ]);
        assert!(parsed.is_ok(), "session export -o ... should still parse");
    }
}
