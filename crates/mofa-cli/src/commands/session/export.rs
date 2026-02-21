//! `mofa session export` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;
use std::path::PathBuf;

/// Execute the `mofa session export` command
pub fn run(session_id: &str, output: PathBuf, format: &str) -> anyhow::Result<()> {
    println!("{} Exporting session: {}", "→".green(), session_id.cyan());
    println!("  Format: {}", format.yellow());
    println!("  Output: {}", output.display().to_string().cyan());
    println!();

    let backend = CliBackend::discover()?;
    let session = backend.get_session(session_id)?;

    let output_str = match format {
        "json" => serde_json::to_string_pretty(&session)?,
        "yaml" => serde_yaml::to_string(&session)?,
        _ => anyhow::bail!("Unsupported export format: {}", format),
    };

    std::fs::write(&output, output_str)?;
    println!("{} Session exported to {}", "✓".green(), output.display());

    Ok(())
}
