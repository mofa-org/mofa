//! `mofa agent start` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa agent start` command
pub fn run(agent_id: &str, config: Option<&std::path::Path>, daemon: bool) -> anyhow::Result<()> {
    println!("{} Starting agent: {}", "→".green(), agent_id.cyan());

    let backend = CliBackend::discover()?;
    let started = backend.start_agent(agent_id, config, daemon)?;

    if started.daemon {
        println!("  Mode: {}", "daemon".yellow());
    }
    if let Some(path) = started.config_path {
        println!("  Config: {}", path.cyan());
    }
    println!("{} Agent '{}' started", "✓".green(), started.id);

    Ok(())
}
