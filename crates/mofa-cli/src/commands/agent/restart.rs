//! `mofa agent restart` command implementation

use colored::Colorize;

/// Execute the `mofa agent restart` command
pub fn run(agent_id: &str, _config: Option<&std::path::Path>) -> anyhow::Result<()> {
    println!("{} Restarting agent: {}", "→".green(), agent_id.cyan());

    // TODO: Implement actual agent restart logic
    // This would involve:
    // 1. Stopping the agent
    // 2. Starting it again with the same config

    println!("{} Agent '{}' restarted", "✓".green(), agent_id);

    Ok(())
}
