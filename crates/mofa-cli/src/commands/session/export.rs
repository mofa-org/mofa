//! `mofa session export` command implementation

use crate::context::CliContext;
use colored::Colorize;
use std::path::PathBuf;

/// Execute the `mofa session export` command
pub async fn run(
    ctx: &CliContext,
    session_id: &str,
    output: PathBuf,
    format: &str,
) -> anyhow::Result<()> {
    println!("{} Exporting session: {}", "→".green(), session_id.cyan());
    println!("  Format: {}", format.yellow());
    println!("  Output: {}", output.display().to_string().cyan());
    println!();

    let session = ctx.session_manager.get_or_create(session_id).await;

    if session.is_empty() {
        println!(
            "{} Session '{}' has no messages to export",
            "!".yellow(),
            session_id
        );
        return Ok(());
    }

    let session_data = serde_json::json!({
        "session_id": session.key,
        "created_at": session.created_at.to_rfc3339(),
        "updated_at": session.updated_at.to_rfc3339(),
        "metadata": session.metadata,
        "messages": session.messages.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content,
                "timestamp": m.timestamp.to_rfc3339(),
            })
        }).collect::<Vec<_>>(),
    });

    let output_str = match format {
        "json" => serde_json::to_string_pretty(&session_data)?,
        "yaml" => serde_yaml::to_string(&session_data)?,
        _ => anyhow::bail!("Unsupported export format: {}", format),
    };

    std::fs::write(&output, output_str)?;
    println!("{} Session exported to {}", "✓".green(), output.display());

    Ok(())
}
