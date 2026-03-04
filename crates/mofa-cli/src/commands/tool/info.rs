//! `mofa tool info` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;
use mofa_kernel::agent::components::tool::ToolRegistry;

/// Execute the `mofa tool info` command
pub async fn run(ctx: &CliContext, name: &str) -> Result<(), CliError> {
    println!("{} Tool information: {}", "→".green(), name.cyan());
    println!();

    match ctx.tool_registry.get(name) {
        Some(tool) => {
            let metadata = tool.metadata();
            println!("  Name:           {}", tool.name().cyan());
            println!("  Description:    {}", tool.description().white());
            if let Some(category) = &metadata.category {
                println!("  Category:       {}", category.white());
            }
            if !metadata.tags.is_empty() {
                println!("  Tags:           {}", metadata.tags.join(", ").white());
            }
            println!(
                "  Dangerous:      {}",
                if metadata.is_dangerous {
                    "Yes".red()
                } else {
                    "No".green()
                }
            );
            println!(
                "  Needs network:  {}",
                if metadata.requires_network {
                    "Yes".yellow()
                } else {
                    "No".white()
                }
            );
            println!(
                "  Needs FS:       {}",
                if metadata.requires_filesystem {
                    "Yes".yellow()
                } else {
                    "No".white()
                }
            );
            println!(
                "  Confirmation:   {}",
                if tool.requires_confirmation() {
                    "Required".yellow()
                } else {
                    "Not required".white()
                }
            );

            // Show parameter schema
            let schema = tool.parameters_schema();
            if !schema.is_null() {
                println!();
                println!("  Parameters:");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&schema)?
                        .lines()
                        .map(|l| format!("    {}", l))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }

            // Show source
            if let Some(source) = ctx.tool_registry.get_source(name) {
                println!();
                println!("  Source:          {:?}", source);
            }
        }
        None => {
            println!("  Tool '{}' not found in registry", name);
            println!();
            println!("  Use {} to see available tools.", "mofa tool list".cyan());
        }
    }

    println!();
    Ok(())
}
