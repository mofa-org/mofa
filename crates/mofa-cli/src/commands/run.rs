//! `mofa run` command implementation

use crate::CliError;
use colored::Colorize;

/// Execute the `mofa run` command
pub fn run(config: &std::path::Path) -> Result<(), CliError> {
    println!(
        "{} Running agent with config: {}",
        "→".green(),
        config.display()
    );

    let status = std::process::Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(config)
        .status()?;

    if !status.success() {
        println!("{} Agent exited with error", "✗".red());
        std::process::exit(1);
    }

    Ok(())
}
