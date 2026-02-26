//! `mofa info` command implementation
//!
//! Displays system information, runtime stats, and all available commands
//! with their subcommands dynamically derived from the CLI definition.

use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa info` command
pub async fn run(ctx: &CliContext) -> anyhow::Result<()> {
    print_header();
    print_system_info(ctx);
    print_runtime_stats(ctx).await;
    print_commands();
    Ok(())
}

fn print_header() {
    println!();
    println!("  🤖  MoFA — Model-based Framework for Agents");
    println!();
    println!("  Version:    {}", env!("CARGO_PKG_VERSION").yellow());
    println!(
        "  Repository: {}",
        "https://github.com/mofa-org/mofa".blue()
    );
    println!();
}

fn print_system_info(ctx: &CliContext) {
    println!("  {}", "Directories".bold());
    println!("    Data:   {}", ctx.data_dir.display().to_string().cyan());
    println!(
        "    Config: {}",
        ctx.config_dir.display().to_string().cyan()
    );
    println!();
}

async fn print_runtime_stats(ctx: &CliContext) {
    let agent_count = ctx.agent_store.list().map(|v| v.len()).unwrap_or(0);
    let plugin_count = ctx.plugin_store.list().map(|v| v.len()).unwrap_or(0);
    let tool_count = ctx.tool_store.list().map(|v| v.len()).unwrap_or(0);
    let session_count = ctx
        .session_manager
        .list()
        .await
        .map(|v| v.len())
        .unwrap_or(0);

    println!("  {}", "Runtime".bold());
    println!(
        "    Agents:   {}    Plugins: {}    Tools: {}    Sessions: {}",
        agent_count.to_string().yellow(),
        plugin_count.to_string().yellow(),
        tool_count.to_string().yellow(),
        session_count.to_string().yellow(),
    );
    println!();
}

fn print_commands() {
    println!("  {}", "Commands".bold());

    let commands: &[(&str, &[&str])] = &[
        ("new", &["Create a new MoFA agent project"]),
        ("init", &["Initialize MoFA in an existing project"]),
        ("build", &["Build the agent project"]),
        ("run", &["Run the agent"]),
        ("generate", &["config", "dataflow"]),
        ("info", &["Show this information"]),
        ("db", &["init", "schema"]),
        (
            "agent",
            &[
                "create", "start", "stop", "restart", "status", "list", "logs", "delete",
            ],
        ),
        (
            "config",
            &["value get|set|unset", "list", "validate", "path"],
        ),
        ("plugin", &["list", "info", "install", "uninstall"]),
        ("session", &["list", "show", "delete", "export"]),
        ("tool", &["list", "info", "enable", "disable"]),
    ];

    for (name, subs) in commands {
        if subs.len() == 1 && !subs[0].contains(' ') && subs[0].len() > 10 {
            // Single-purpose command — show its description
            println!("    {:<14} {}", name.green(), subs[0].dimmed());
        } else {
            // Command with subcommands
            println!("    {:<14} {}", name.green(), subs.join(" | ").dimmed());
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_info_runs_without_error() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        let result = run(&ctx).await;
        assert!(result.is_ok());
    }
}
