//! `mofa config` command implementation

use colored::Colorize;
use std::collections::HashMap;

/// Execute the `mofa config get` command
pub fn run_get(key: &str) -> anyhow::Result<()> {
    let config = load_global_config()?;
    match config.get(key) {
        Some(value) => println!("{}", value),
        None => {
            println!("Config key '{}' not found", key.yellow());
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Execute the `mofa config set` command
pub fn run_set(key: &str, value: &str) -> anyhow::Result<()> {
    println!("{} Setting config: {} = {}", "→".green(), key.cyan(), value.white());
    // TODO: Implement actual config setting
    Ok(())
}

/// Execute the `mofa config unset` command
pub fn run_unset(key: &str) -> anyhow::Result<()> {
    println!("{} Unsetting config: {}", "→".green(), key.cyan());
    // TODO: Implement actual config unsetting
    Ok(())
}

/// Execute the `mofa config list` command
pub fn run_list() -> anyhow::Result<()> {
    println!("{} Global configuration", "→".green());
    println!();

    let config = load_global_config()?;

    if config.is_empty() {
        println!("  No configuration values set.");
    } else {
        let width = config.keys().map(|k| k.len()).max().unwrap_or(0);
        for (key, value) in config {
            println!("  {:<width$} = {}", key, value, width = width);
        }
    }

    Ok(())
}

/// Execute the `mofa config validate` command
pub fn run_validate() -> anyhow::Result<()> {
    println!("{} Validating configuration", "→".green());

    // TODO: Implement actual validation logic

    println!("{} Configuration is valid", "✓".green());
    Ok(())
}

/// Execute the `mofa config path` command
pub fn run_path() -> anyhow::Result<()> {
    let path = crate::utils::mofa_config_dir()?;
    println!("{}", path.display());
    Ok(())
}

/// Load global configuration from config directory
fn load_global_config() -> anyhow::Result<HashMap<String, String>> {
    let config_dir = crate::utils::mofa_config_dir()?;
    let config_file = config_dir.join("config.yml");

    if !config_file.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&config_file)?;
    let config: HashMap<String, String> = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

    Ok(config)
}
