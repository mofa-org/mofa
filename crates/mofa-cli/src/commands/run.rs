//! `mofa run` command implementation

use crate::CliError;
use colored::Colorize;

/// Execute the `mofa run` command
pub fn run(config: &std::path::Path, _dora: bool) -> Result<(), CliError> {
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

/// Execute the `mofa dataflow` command (requires dora feature)
#[cfg(feature = "dora")]
pub fn run_dataflow(file: &std::path::Path, uv: bool) -> Result<(), CliError> {
    use mofa_sdk::dora::{DoraRuntime, RuntimeConfig};

    println!("{} Running dataflow: {}", "→".green(), file.display());

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let config = RuntimeConfig::embedded(file).with_uv(uv);
        let mut runtime = DoraRuntime::new(config);
        match runtime.run().await {
            Ok(result) => {
                println!("{} Dataflow {} completed", "✓".green(), result.uuid);
                Ok(())
            }
            Err(e) => {
                return Err(CliError::Other(format!("Dataflow failed: {}", e)))
            }
        }
    })
}
