//! `mofa agent list` command implementation

use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa agent list` command
pub async fn run(ctx: &CliContext, running_only: bool, _show_all: bool) -> anyhow::Result<()> {
    println!("{} Listing agents", "→".green());
    println!();

    let agents_metadata = ctx.agent_registry.list().await;

    if agents_metadata.is_empty() {
        println!("  No agents registered.");
        println!();
        println!(
            "  Use {} to start an agent.",
            "mofa agent start <agent_id>".cyan()
        );
        return Ok(());
    }

    let agents: Vec<AgentInfo> = agents_metadata
        .iter()
        .map(|m| {
            let status = format!("{:?}", m.state);
            AgentInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                status,
                description: m.description.clone(),
            }
        })
        .collect();

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
