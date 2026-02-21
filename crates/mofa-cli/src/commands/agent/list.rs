//! `mofa agent list` command implementation

use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use serde::Serialize;
use std::collections::BTreeMap;

/// Execute the `mofa agent list` command
pub async fn run(ctx: &CliContext, running_only: bool, _show_all: bool) -> anyhow::Result<()> {
    println!("{} Listing agents", "→".green());
    println!();

    let agents_metadata = ctx.agent_registry.list().await;
    let persisted_agents = ctx
        .agent_store
        .list()
        .map_err(|e| anyhow::anyhow!("Failed to list persisted agents: {}", e))?;

    let mut merged: BTreeMap<String, AgentInfo> = BTreeMap::new();
    for m in &agents_metadata {
        let status = format!("{:?}", m.state);
        merged.insert(
            m.id.clone(),
            AgentInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                status,
                description: m.description.clone(),
            },
        );
    }

    for (_, entry) in persisted_agents {
        merged.entry(entry.id.clone()).or_insert_with(|| {
            // Agents not in the in-memory registry are not currently running,
            // regardless of their last-persisted state.
            let mut status = entry.state;
            if status == "Running" || status == "Ready" {
                status = "Stopped".to_string();
            }
            AgentInfo {
                id: entry.id,
                name: entry.name,
                status,
                description: entry.description,
            }
        });
    }

    if merged.is_empty() {
        println!("  No agents registered.");
        println!();
        println!(
            "  Use {} to start an agent.",
            "mofa agent start <agent_id>".cyan()
        );
        return Ok(());
    }

    let agents: Vec<AgentInfo> = merged.into_values().collect();

    // Filter based on flags
    let filtered: Vec<_> = if running_only {
        agents
            .into_iter()
            .filter(|a| a.status == "Running" || a.status == "Ready")
            .collect()
    } else {
        agents
    };

    if filtered.is_empty() {
        println!("  No agents found matching criteria.");
        return Ok(());
    }

    // Display as table
    let json = serde_json::to_value(&filtered)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct AgentInfo {
    id: String,
    name: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}
