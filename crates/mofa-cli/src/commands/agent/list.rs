//! `mofa agent list` command implementation

use crate::output::Table;
use crate::state::{self, AgentStatus, AgentStateStore};
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa agent list` command (async version)
pub async fn run_async(running_only: bool, show_all: bool) -> anyhow::Result<()> {
    println!("{} Listing agents", "→".green());

    if running_only {
        println!("  Showing running agents only");
    } else if show_all {
        println!("  Showing all agents");
    }

    println!();

    // Load agents from state store
    let store = state::get_agent_store().await?;
    let mut records = store.list().await?;

    // Filter based on status
    if running_only {
        records.retain(|r| r.status == AgentStatus::Running);
    }

    if records.is_empty() {
        println!("  No agents found.");
        return Ok(());
    }

    // Convert to display format
    let agents: Vec<AgentInfo> = records
        .into_iter()
        .map(|r| {
            let uptime = r.uptime();
            AgentInfo {
                id: r.id,
                name: r.name,
                status: r.status.to_string(),
                uptime,
                provider: r.provider,
                model: r.model,
            }
        })
        .collect();

    // Display as table
    let json = serde_json::to_value(&agents)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    Ok(())
}

/// Keep the sync version for backward compatibility
pub fn run(running_only: bool, show_all: bool) -> anyhow::Result<()> {
    println!("{} Listing agents (warning: using fallback sync version)", "→".yellow());
    
    if running_only {
        println!("  Showing running agents only");
    } else if show_all {
        println!("  Showing all agents");
    }

    println!();
    println!("  {} No agents found (sync mode fallback)", "ℹ".blue());
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct AgentInfo {
    id: String,
    name: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}
