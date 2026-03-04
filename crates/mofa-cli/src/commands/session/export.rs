//! `mofa session export` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;
use std::path::PathBuf;

/// Execute the `mofa session export` command
pub async fn run(
    ctx: &CliContext,
    session_id: &str,
    output: PathBuf,
    format: &str,
) -> Result<(), CliError> {
    println!("{} Exporting session: {}", "→".green(), session_id.cyan());
    println!("  Format: {}", format.yellow());
    println!("  Output: {}", output.display().to_string().cyan());
    println!();

    let session = ctx
        .session_manager
        .get(session_id)
        .await
        .map_err(|e| CliError::SessionError(format!("Failed to load session: {}", e)))?
        .ok_or_else(|| CliError::SessionError(format!("Session '{}' not found", session_id)))?;

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
        _ => return Err(CliError::SessionError(format!("Unsupported export format: {}", format))),
    };

    std::fs::write(&output, output_str)?;
    println!("{} Session exported to {}", "✓".green(), output.display());

    Ok(())
}
