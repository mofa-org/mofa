//! `mofa agent start` command implementation

use crate::state::{self, AgentRecord, AgentStatus, AgentStateStore};
use colored::Colorize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Execute the `mofa agent start` command (async version)
pub async fn run_async(agent_id: &str, _config: Option<&std::path::Path>, daemon: bool) -> anyhow::Result<()> {
    println!("{} Starting agent: {}", "→".green(), agent_id.cyan());

    if daemon {
        println!("  Mode: {}", "daemon".yellow());
    }

    // Load state store
    let store = state::get_agent_store().await?;

    // Check if agent exists
    let mut record = match store.get(agent_id).await? {
        Some(r) => {
            if r.status == AgentStatus::Running {
                println!("{} Agent '{}' is already running", "!".yellow(), agent_id);
                return Ok(());
            }
            r
        }
        None => {
            // Create new agent record with default values
            AgentRecord::new(agent_id, agent_id)
        }
    };

    // Mark as running with current Unix timestamp
    record.status = AgentStatus::Running;
    record.started_at = Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
    );

    // Save to store
    store.update(record).await?;

    println!("{} Agent '{}' started", "✓".green(), agent_id);

    Ok(())
}

/// Keep the sync version for backward compatibility
pub fn run(agent_id: &str, _config: Option<&std::path::Path>, daemon: bool) -> anyhow::Result<()> {
    println!("{} Starting agent: {} (warning: using fallback sync version)", "→".yellow(), agent_id.cyan());

    if daemon {
        println!("  Mode: {}", "daemon".yellow());
    }

    println!("{} Agent '{}' started (sync mode)", "✓".green(), agent_id);

    Ok(())
}
