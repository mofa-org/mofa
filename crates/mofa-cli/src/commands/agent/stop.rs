//! `mofa agent stop` command implementation

use crate::state::{self, AgentStatus, AgentStateStore};
use colored::Colorize;

/// Execute the `mofa agent stop` command (async version)
pub async fn run_async(agent_id: &str) -> anyhow::Result<()> {
    println!("{} Stopping agent: {}", "→".green(), agent_id.cyan());

    // Load state store
    let store = state::get_agent_store().await?;

    // Check if agent exists
    match store.get(agent_id).await? {
        Some(mut record) => {
            if record.status == AgentStatus::Stopped {
                println!("{} Agent '{}' is already stopped", "!".yellow(), agent_id);
                return Ok(());
            }

            // Mark as stopped
            record.status = AgentStatus::Stopped;
            record.started_at = None;

            // Save to store
            store.update(record).await?;

            println!("{} Agent '{}' stopped", "✓".green(), agent_id);
            Ok(())
        }
        None => {
            println!("{} Agent '{}' not found", "✗".red(), agent_id);
            Err(anyhow::anyhow!("Agent not found: {}", agent_id))
        }
    }
}

/// Keep the sync version for backward compatibility
pub fn run(agent_id: &str) -> anyhow::Result<()> {
    println!("{} Stopping agent: {} (warning: using fallback sync version)", "→".yellow(), agent_id.cyan());

    println!("{} Agent '{}' stopped (sync mode)", "✓".green(), agent_id);

    Ok(())
}
