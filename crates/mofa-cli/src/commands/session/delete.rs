//! `mofa session delete` command implementation

use crate::context::CliContext;
use colored::Colorize;
use dialoguer::Confirm;

/// Execute the `mofa session delete` command
pub async fn run(ctx: &CliContext, session_id: &str, force: bool) -> anyhow::Result<()> {
    if !force {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Delete session '{}'? This cannot be undone",
                session_id
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{} Cancelled", "→".yellow());
            return Ok(());
        }
    }

    println!("{} Deleting session: {}", "→".green(), session_id.cyan());

    let deleted = ctx
        .session_manager
        .delete(session_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete session: {}", e))?;

    if deleted {
        println!("{} Session '{}' deleted", "✓".green(), session_id);
    } else {
        println!("{} Session '{}' not found", "!".yellow(), session_id);
    }

    Ok(())
}
