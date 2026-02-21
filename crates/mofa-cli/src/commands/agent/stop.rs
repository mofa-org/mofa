//! `mofa agent stop` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa agent stop` command
pub fn run(agent_id: &str) -> anyhow::Result<()> {
    println!("{} Stopping agent: {}", "→".green(), agent_id.cyan());
    let backend = CliBackend::discover()?;
    let stopped = backend.stop_agent(agent_id)?;
    println!("{} Agent '{}' stopped", "✓".green(), stopped.id);

    Ok(())
}
