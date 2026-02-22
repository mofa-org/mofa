//! `mofa agent stop` command implementation

use crate::context::CliContext;
use colored::Colorize;
use tracing::info;

/// Execute the `mofa agent stop` command
pub async fn run(ctx: &CliContext, agent_id: &str) -> anyhow::Result<()> {
    println!("{} Stopping agent: {}", "→".green(), agent_id.cyan());

    // Check if agent exists
    if !ctx.persistent_agents.exists(agent_id).await {
        anyhow::bail!("Agent '{}' not found", agent_id);
    }

    // Get agent metadata
    let mut agent_metadata = ctx
        .persistent_agents
        .get(agent_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to retrieve agent metadata"))?;

    println!("  Name:   {}", agent_metadata.name.cyan());
    println!(
        "  Status: {}",
        agent_metadata.last_state.to_string().yellow()
    );

    // If running, stop the process
    if agent_metadata.last_state == crate::state::AgentProcessState::Running {
        if let Some(pid) = agent_metadata.process_id {
            println!("  PID:    {}", pid.to_string().cyan());
            println!("  {} Sending termination signal...", "→".green());

            // Try graceful shutdown first, then force if needed
            match ctx.process_manager.stop_agent_by_pid(pid, false).await {
                Ok(_) => {
                    println!("  {} Process terminated gracefully", "✓".green());

                    // Give process time to clean up
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                    // Check if still running, force if necessary
                    if ctx.process_manager.is_running(pid) {
                        println!(
                            "  {} Process still running, forcing termination...",
                            "!".yellow()
                        );
                        if let Err(e) = ctx.process_manager.stop_agent_by_pid(pid, true).await {
                            eprintln!("  {} Failed to force termination: {}", "✗".red(), e);
                        } else {
                            println!("  {} Process force-terminated", "✓".green());
                        }
                    }
                }
                Err(e) => {
                    println!("  {} Failed to terminate process: {}", "✗".red(), e);
                    println!(
                        "  {} The agent may still be running. Try 'tasklist' or 'ps' to verify.",
                        "!".yellow()
                    );
                }
            }
        }
    } else {
        println!("  {} Agent is not running", "!".yellow());
    }

    // Update agent state to stopped
    agent_metadata.mark_stopped();
    ctx.persistent_agents.update(agent_metadata).await?;

    // Untrack the process
    ctx.persistent_agents.untrack_process(agent_id).await;

    println!("{} Agent '{}' stopped", "✓".green(), agent_id);

    info!("Agent '{}' stopped", agent_id);

    Ok(())
}
