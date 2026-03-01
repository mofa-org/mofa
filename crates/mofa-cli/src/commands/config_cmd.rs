//! `mofa config` command implementation

use crate::CliError;
use colored::Colorize;
use mofa_kernel::config::{load_config, substitute_env_vars};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Execute the `mofa config get` command
pub fn run_get(key: &str) -> Result<(), CliError> {
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
pub fn run_set(key: &str, value: &str) -> Result<(), CliError> {
    println!(
        "{} Setting config: {} = {}",
        "->".green(),
        key.cyan(),
        value.white()
    );

    let mut config = load_global_config()?;
    config.insert(key.to_string(), value.to_string());
    save_global_config(config)?;

    println!("{} Config saved successfully", "✓".green());
    Ok(())
}

/// Execute the `mofa config unset` command
pub fn run_unset(key: &str) -> Result<(), CliError> {
    println!("{} Unsetting config: {}", "->".green(), key.cyan());

    let mut config = load_global_config()?;
    if config.remove(key).is_none() {
        println!("{} Config key '{}' not found", "Warning".yellow(), key);
    } else {
        save_global_config(config)?;
        println!("{} Config unset successfully", "✓".green());
    }

    Ok(())
}

/// Execute the `mofa config list` command
pub fn run_list() -> Result<(), CliError> {
    println!("{} Global configuration", "->".green());
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
pub fn run_validate(config_path: Option<PathBuf>) -> Result<(), CliError> {
    println!("{} Validating configuration", "->".green());

    let config_path = if let Some(path) = config_path {
        path
    } else {
        // Try to find a config file
        find_config_file().ok_or_else(|| CliError::ConfigError("No config file found".into()))?
    };

    println!(
        "  Config file: {}",
        config_path.display().to_string().cyan()
    );

    // Load and validate the config
    match validate_config_file(&config_path) {
        Ok(_) => {
            println!("{} Configuration is valid", "✓".green());
            Ok(())
        }
        Err(e) => {
            println!("{} Configuration validation failed: {}", "✗".red(), e);
            std::process::exit(1);
        }
    }
}

/// Execute the `mofa config path` command
pub fn run_path() -> Result<(), CliError> {
    let path = crate::utils::mofa_config_dir()?;
    println!("{}", path.display());
    Ok(())
}

/// Load global configuration from config directory
fn load_global_config() -> Result<HashMap<String, String>, CliError> {
    let config_dir = crate::utils::mofa_config_dir()?;
    let config_file = config_dir.join("config.yml");

    if !config_file.exists() {
        return Ok(HashMap::new());
    }

    let _content = fs::read_to_string(&config_file)?;

    // Use the global config loading
    let config: Value = load_config(config_file.to_string_lossy().as_ref())
        .map_err(|e| CliError::ConfigError(format!("Failed to parse config: {}", e)))?;

    let mut result = HashMap::new();
    if let Some(obj) = config.as_object() {
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                result.insert(key.clone(), s.to_string());
            } else {
                result.insert(key.clone(), value.to_string());
            }
        }
    }

    Ok(result)
}

/// Save global configuration to config directory
fn save_global_config(config: HashMap<String, String>) -> Result<(), CliError> {
    let config_dir = crate::utils::mofa_config_dir()?;
    fs::create_dir_all(&config_dir)?;

    let config_file = config_dir.join("config.yml");

    // Convert to JSON/YAML structure
    let json_obj: Value = config.into_iter().map(|(k, v)| (k, json!(v))).collect();

    let content = serde_yaml::to_string(&json_obj)?;

    fs::write(&config_file, content)?;

    Ok(())
}

/// Find a config file in the current directory or parent directories
fn find_config_file() -> Option<PathBuf> {
    let supported_filenames = [
        "agent.yml",
        "agent.yaml",
        "agent.toml",
        "agent.json",
        "agent.ini",
        "agent.ron",
        "agent.json5",
    ];

    // Try current directory first
    for name in &supported_filenames {
        let path = PathBuf::from(name);
        if path.exists() {
            return Some(path);
        }
    }

    // Search upward
    let mut current = std::env::current_dir().ok()?;
    loop {
        for name in &supported_filenames {
            let target = current.join(name);
            if target.exists() {
                return Some(target);
            }
        }

        if !current.pop() {
            break;
        }
    }

    None
}

/// Validate a configuration file
fn validate_config_file(path: &PathBuf) -> Result<(), CliError> {
    let content = fs::read_to_string(path)?;

    // Check for environment variable substitution
    let substituted = substitute_env_vars(&content);

    // Try to parse based on file extension
    let result = match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            match ext.to_lowercase().as_str() {
                "yaml" | "yml" => serde_yaml::from_str::<Value>(&substituted)
                    .map_err(|e| CliError::ConfigError(format!("YAML parsing error: {}", e))),
                "toml" => toml::from_str::<Value>(&substituted)
                    .map_err(|e| CliError::ConfigError(format!("TOML parsing error: {}", e))),
                "json" => serde_json::from_str::<Value>(&substituted)
                    .map_err(|e| CliError::ConfigError(format!("JSON parsing error: {}", e))),
                "json5" => {
                    // Try to parse as JSON5 (fall back to JSON for validation)
                    serde_json::from_str::<Value>(&substituted)
                        .map_err(|e| CliError::ConfigError(format!("JSON5 parsing error: {}", e)))
                }
                "ini" => {
                    return Err(CliError::ConfigError(
                        "INI format validation is not yet supported. Please use YAML, TOML, or JSON format for validated configuration.".into()
                    ));
                }
                "ron" => {
                    return Err(CliError::ConfigError(
                        "RON format validation is not yet supported. Please use YAML, TOML, or JSON format for validated configuration.".into()
                    ));
                }
                _ => {
                    return Err(CliError::ConfigError(format!("Unsupported config format: {}", ext)));
                }
            }
        }
        None => {
            return Err(CliError::ConfigError("Cannot determine file format".into()));
        }
    };

    // Validate required fields
    if let Ok(config) = result
        && let Some(obj) = config.as_object()
    {
        // Check for agent section
        if !obj.contains_key("agent") {
            return Err(CliError::ConfigError("Missing required 'agent' section".into()));
        }

        // Check for required agent fields
        if let Some(agent) = obj.get("agent").and_then(|v| v.as_object()) {
            if !agent.contains_key("id") {
                return Err(CliError::ConfigError("Missing required 'agent.id' field".into()));
            }
            if !agent.contains_key("name") {
                return Err(CliError::ConfigError("Missing required 'agent.name' field".into()));
            }
        }
    }

    Ok(())
}
