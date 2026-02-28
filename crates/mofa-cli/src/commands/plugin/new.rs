use crate::error::CliError;
use clap::ValueEnum;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use std::fs;
use std::path::{Path, PathBuf};
use serde::Serialize;
use tera::{Context, Tera};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PluginTemplate {
    SimpleTool,
    Middleware,
    LlmProvider,
    Custom,
}

impl std::fmt::Display for PluginTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginTemplate::SimpleTool => write!(f, "Simple Tool"),
            PluginTemplate::Middleware => write!(f, "Middleware"),
            PluginTemplate::LlmProvider => write!(f, "LLM Provider"),
            PluginTemplate::Custom => write!(f, "Custom (Empty)"),
        }
    }
}

pub async fn run(name: Option<&str>) -> Result<(), CliError> {
    let theme = ColorfulTheme::default();

    // 1. Prompt for Name
    let plugin_name = match name {
        Some(n) => n.to_string(),
        None => Input::with_theme(&theme)
            .with_prompt("Plugin name (e.g. my-awesome-plugin)")
            .interact_text()
            .map_err(|e| CliError::Other(format!("Prompt failed: {}", e)))?,
    };

    // Normalize hyphen/underscore for rust crate names
    let crate_name = plugin_name.replace("-", "_");
    
    // 2. Prompt for Description
    let description: String = Input::with_theme(&theme)
        .with_prompt("Short description")
        .default("A MoFA plugin".into())
        .interact_text()
        .map_err(|e| CliError::Other(format!("Prompt failed: {}", e)))?;

    // 3. Prompt for Author
    let author: String = Input::with_theme(&theme)
        .with_prompt("Author")
        .default("Anonymous".into())
        .interact_text()
        .map_err(|e| CliError::Other(format!("Prompt failed: {}", e)))?;

    // 4. Prompt for Template Type
    let templates = &[
        PluginTemplate::SimpleTool,
        PluginTemplate::Middleware,
        PluginTemplate::LlmProvider,
        PluginTemplate::Custom,
    ];
    let selection = Select::with_theme(&theme)
        .with_prompt("Select a starting template")
        .default(0)
        .items(&templates[..])
        .interact()
        .map_err(|e| CliError::Other(format!("Prompt failed: {}", e)))?;

    let selected_template = templates[selection];

    println!(
        "\n🚀 Scaffolding new plugin {} ({})...",
        plugin_name.cyan().bold(),
        selected_template.to_string().yellow()
    );

    let target_dir = std::env::current_dir()?.join(&plugin_name);

    if target_dir.exists() {
        return Err(CliError::Other(format!(
            "Directory '{}' already exists. Aborting.",
            target_dir.display()
        )));
    }

    generate_scaffold(&target_dir, &plugin_name, &crate_name, &description, &author, selected_template)?;

    println!(
        "✅ Successfully created plugin in {}!",
        target_dir.display().to_string().green()
    );
    
    // Attempt auto-adding to workspace
    let added_to_workspace = add_to_workspace_if_present(&target_dir)?;
    if added_to_workspace {
        println!("ℹ️  Added `{}` to the adjacent workspace Cargo.toml", plugin_name.cyan());
    }

    println!("\nNext steps:");
    println!("  cd {}", plugin_name);
    println!("  cargo check");

    Ok(())
}

fn generate_scaffold(
    target: &Path,
    plugin_name: &str,
    crate_name: &str,
    description: &str,
    author: &str,
    template: PluginTemplate,
) -> Result<(), CliError> {
    fs::create_dir_all(target)?;
    fs::create_dir_all(target.join("src"))?;
    fs::create_dir_all(target.join("tests"))?;

    let mut tera = Tera::default();
    let mut ctx = Context::new();
    ctx.insert("plugin_name", plugin_name);
    ctx.insert("crate_name", crate_name);
    ctx.insert("description", description);
    ctx.insert("author", author);
    ctx.insert("template_type", &format!("{:?}", template));

    // Define all templates
    tera.add_raw_template("Cargo.toml", include_str!("../../templates/Cargo.toml.tera"))
        .map_err(|e| CliError::Other(e.to_string()))?;
    tera.add_raw_template("lib.rs", include_str!("../../templates/lib.rs.tera"))
        .map_err(|e| CliError::Other(e.to_string()))?;
    tera.add_raw_template("config.rs", include_str!("../../templates/config.rs.tera"))
        .map_err(|e| CliError::Other(e.to_string()))?;
    tera.add_raw_template("handler.rs", include_str!("../../templates/handler.rs.tera"))
        .map_err(|e| CliError::Other(e.to_string()))?;
    tera.add_raw_template("integration.rs", include_str!("../../templates/integration.rs.tera"))
        .map_err(|e| CliError::Other(e.to_string()))?;
    tera.add_raw_template("README.md", include_str!("../../templates/README.md.tera"))
        .map_err(|e| CliError::Other(e.to_string()))?;

    // Render & write
    let cargo_toml = tera.render("Cargo.toml", &ctx).map_err(|e| CliError::Other(e.to_string()))?;
    fs::write(target.join("Cargo.toml"), cargo_toml)?;

    let lib_rs = tera.render("lib.rs", &ctx).map_err(|e| CliError::Other(e.to_string()))?;
    fs::write(target.join("src/lib.rs"), lib_rs)?;

    let config_rs = tera.render("config.rs", &ctx).map_err(|e| CliError::Other(e.to_string()))?;
    fs::write(target.join("src/config.rs"), config_rs)?;

    let handler_rs = tera.render("handler.rs", &ctx).map_err(|e| CliError::Other(e.to_string()))?;
    fs::write(target.join("src/handler.rs"), handler_rs)?;

    let integration_rs = tera.render("integration.rs", &ctx).map_err(|e| CliError::Other(e.to_string()))?;
    fs::write(target.join("tests/integration.rs"), integration_rs)?;

    let readme = tera.render("README.md", &ctx).map_err(|e| CliError::Other(e.to_string()))?;
    fs::write(target.join("README.md"), readme)?;

    Ok(())
}

fn add_to_workspace_if_present(target_dir: &Path) -> Result<bool, CliError> {
    // Try to find a workspace Cargo.toml at the parent level
    let mut current = target_dir.parent();
    
    // Typical heuristics: We usually execute from root workspace or nested once
    // For safety, just check exactly one parent.
    if let Some(parent) = current {
        let parent_cargo = parent.join("Cargo.toml");
        if parent_cargo.exists() {
            let content = fs::read_to_string(&parent_cargo)?;
            // Rudimentary check for workspace members array
            if content.contains("[workspace]") && content.contains("members = [") {
                // If it isn't already there...
                let plugin_name = target_dir.file_name().unwrap_or_default().to_string_lossy();
                if !content.contains(&format!("\"{}\"", plugin_name)) && !content.contains(&format!("'{}'", plugin_name)) {
                    // Try to inject it at the end of members
                    if let Some(members_start) = content.find("members = [") {
                        let members_end = content[members_start..].find("]").map(|m| m + members_start);
                        if let Some(end_idx) = members_end {
                            // Extract inner array, add ours, reform
                            let inner = &content[members_start + 11..end_idx];
                            let new_content = if inner.trim().is_empty() {
                                format!("{}members = [\n    \"{}\"\n]{}", &content[..members_start], plugin_name, &content[end_idx+1..])
                            } else {
                                // Find last entry
                                let last_quote = inner.rfind('"').or_else(|| inner.rfind('\''));
                                if let Some(q) = last_quote {
                                    let absolute_q = members_start + 11 + q;
                                    format!("{}\",\n    \"{}\"{}", &content[..absolute_q], plugin_name, &content[absolute_q+1..])
                                } else {
                                    content.clone() // fallback
                                }
                            };
                            
                            // To perfectly preserve user formatting, let's do a naive replace
                            fs::write(parent_cargo, new_content)?;
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }
    
    Ok(false)
}
