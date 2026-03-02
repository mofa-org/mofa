//! `mofa build` command implementation

use crate::CliError;
use colored::Colorize;

/// Execute the `mofa build` command
pub fn run(release: bool, features: Option<&str>) -> Result<(), CliError> {
    println!("{} Building agent...", "→".green());

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");

    if release {
        cmd.arg("--release");
    }

    if let Some(f) = features {
        cmd.arg("--features").arg(f);
    }

    let status = cmd.status()?;

    if status.success() {
        println!("{} Build successful!", "✓".green());
    } else {
        println!("{} Build failed!", "✗".red());
        std::process::exit(1);
    }

    Ok(())
}
