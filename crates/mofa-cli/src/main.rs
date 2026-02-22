//! MoFA CLI - Command-line tool for building and managing AI agents

mod cli;
mod commands;
mod config;
mod context;
mod output;
mod render;
mod store;
mod tui;
mod utils;
mod widgets;

use clap::Parser;
use cli::Cli;
use colored::Colorize;
use context::CliContext;

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().collect();
    normalize_legacy_output_flags(&mut args);
    let cli = Cli::parse_from(args);

    if cli.output_legacy.is_some() {
        eprintln!("Warning: '--output' is deprecated. Use '--output-format' instead.");
    }

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    } else {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }

    let rt = tokio::runtime::Runtime::new()?;

    // Launch TUI if requested or no command provided
    if cli.tui || cli.command.is_none() {
        // Run TUI mode
        rt.block_on(tui::run())?;
        Ok(())
    } else {
        // Run CLI command
        rt.block_on(run_command(cli))
    }
}

async fn run_command(cli: Cli) -> anyhow::Result<()> {
    use cli::Commands;

    // Initialize context for commands that need backend services
    let needs_context = matches!(
        &cli.command,
        Some(
            Commands::Agent(_)
                | Commands::Plugin { .. }
                | Commands::Session { .. }
                | Commands::Tool { .. }
        )
    );

    let ctx = if needs_context {
        Some(CliContext::new().await?)
    } else {
        None
    };

    match cli.command {
        Some(Commands::New {
            name,
            template,
            output,
        }) => {
            commands::new::run(&name, &template, output.as_deref())?;
        }

        Some(Commands::Init { path }) => {
            commands::init::run(&path)?;
        }

        Some(Commands::Build { release, features }) => {
            commands::build::run(release, features.as_deref())?;
        }

        Some(Commands::Run { config, dora }) => {
            commands::run::run(&config, dora)?;
        }

        #[cfg(feature = "dora")]
        Some(Commands::Dataflow { file, uv }) => {
            commands::run::run_dataflow(&file, uv)?;
        }

        Some(Commands::Generate { what }) => match what {
            cli::GenerateCommands::Config { output } => {
                commands::generate::run_config(&output)?;
            }
            cli::GenerateCommands::Dataflow { output } => {
                commands::generate::run_dataflow(&output)?;
            }
        },

        Some(Commands::Info) => {
            commands::generate::run_info();
        }

        Some(Commands::Db { action }) => match action {
            cli::DbCommands::Init {
                db_type,
                output,
                database_url,
            } => {
                commands::db::run_init(db_type, output, database_url)?;
            }
            cli::DbCommands::Schema { db_type } => {
                commands::db::run_schema(db_type)?;
            }
        },

        Some(Commands::Agent(agent_cmd)) => {
            let ctx = ctx.as_ref().unwrap();
            match agent_cmd {
                cli::AgentCommands::Create {
                    non_interactive,
                    config,
                } => {
                    commands::agent::create::run(non_interactive, config)?;
                }
                cli::AgentCommands::Start {
                    agent_id,
                    config,
                    factory_type,
                    daemon,
                } => {
                    commands::agent::start::run(
                        ctx,
                        &agent_id,
                        config.as_deref(),
                        factory_type.as_deref(),
                        daemon,
                    )
                    .await?;
                }
                cli::AgentCommands::Stop {
                    agent_id,
                    force_persisted_stop,
                } => {
                    commands::agent::stop::run(ctx, &agent_id, force_persisted_stop).await?;
                }
                cli::AgentCommands::Restart { agent_id, config } => {
                    commands::agent::restart::run(ctx, &agent_id, config.as_deref()).await?;
                }
                cli::AgentCommands::Status { agent_id } => {
                    commands::agent::status::run(ctx, agent_id.as_deref()).await?;
                }
                cli::AgentCommands::List { running, all } => {
                    commands::agent::list::run(ctx, running, all).await?;
                }
            }
        }

        Some(Commands::Config { action }) => match action {
            cli::ConfigCommands::Value(value_cmd) => match value_cmd {
                cli::ConfigValueCommands::Get { key } => {
                    commands::config_cmd::run_get(&key)?;
                }
                cli::ConfigValueCommands::Set { key, value } => {
                    commands::config_cmd::run_set(&key, &value)?;
                }
                cli::ConfigValueCommands::Unset { key } => {
                    commands::config_cmd::run_unset(&key)?;
                }
            },
            cli::ConfigCommands::List => {
                commands::config_cmd::run_list()?;
            }
            cli::ConfigCommands::Validate => {
                commands::config_cmd::run_validate(None)?;
            }
            cli::ConfigCommands::Path => {
                commands::config_cmd::run_path()?;
            }
        },

        Some(Commands::Plugin { action }) => {
            let ctx = ctx.as_ref().unwrap();
            match action {
                cli::PluginCommands::List {
                    installed,
                    available,
                } => {
                    commands::plugin::list::run(ctx, installed, available).await?;
                }
                cli::PluginCommands::Info { name } => {
                    commands::plugin::info::run(ctx, &name).await?;
                }
                cli::PluginCommands::Uninstall { name, force } => {
                    commands::plugin::uninstall::run(ctx, &name, force).await?;
                }
            }
        }

        Some(Commands::Session { action }) => {
            let ctx = ctx.as_ref().unwrap();
            match action {
                cli::SessionCommands::List { agent, limit } => {
                    commands::session::list::run(ctx, agent.as_deref(), limit).await?;
                }
                cli::SessionCommands::Show { session_id, format } => {
                    let show_format = format.map(|f| f.to_string());
                    commands::session::show::run(ctx, &session_id, show_format.as_deref()).await?;
                }
                cli::SessionCommands::Delete { session_id, force } => {
                    commands::session::delete::run(ctx, &session_id, force).await?;
                }
                cli::SessionCommands::Export {
                    session_id,
                    output_path,
                    format,
                } => {
                    commands::session::export::run(
                        ctx,
                        &session_id,
                        output_path,
                        &format.to_string(),
                    )
                    .await?;
                }
            }
        }

        Some(Commands::Tool { action }) => {
            let ctx = ctx.as_ref().unwrap();
            match action {
                cli::ToolCommands::List { available, enabled } => {
                    commands::tool::list::run(ctx, available, enabled).await?;
                }
                cli::ToolCommands::Info { name } => {
                    commands::tool::info::run(ctx, &name).await?;
                }
            }
        }

        None => {
            // Should have been handled by TUI check above
            // If we get here, show help
            println!(
                "{}",
                "No command specified. Use --help for usage information.".yellow()
            );
            println!("Run with --tui flag or without arguments to launch the TUI.");
        }
    }

    Ok(())
}

fn normalize_legacy_output_flags(args: &mut [String]) {
    const TOP_LEVEL_COMMANDS: &[&str] = &[
        "new", "init", "build", "run", "dataflow", "generate", "info", "db", "agent", "config",
        "plugin", "session", "tool",
    ];

    let top_command_index = args
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, arg)| TOP_LEVEL_COMMANDS.contains(&arg.as_str()))
        .map(|(idx, _)| idx);

    let top_command = top_command_index.and_then(|idx| args.get(idx).map(String::as_str));
    let allows_global_after_command = matches!(
        top_command,
        Some("info")
            | Some("agent")
            | Some("plugin")
            | Some("tool")
            | Some("config")
            | Some("build")
            | Some("run")
            | Some("init")
    );

    let mut i = 1;
    while i + 1 < args.len() {
        let is_legacy_output_flag = args[i] == "-o" || args[i] == "--output";
        let looks_like_output_format = matches!(args[i + 1].as_str(), "text" | "json" | "table");

        if is_legacy_output_flag && looks_like_output_format {
            let before_command = match top_command_index {
                Some(cmd_idx) => i < cmd_idx,
                None => true,
            };

            if before_command || allows_global_after_command {
                args[i] = "--output-format".to_string();
            }
        }

        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_legacy_output_flags;

    #[test]
    fn test_normalize_legacy_output_before_command() {
        let mut args = vec![
            "mofa".to_string(),
            "--output".to_string(),
            "json".to_string(),
            "info".to_string(),
        ];
        normalize_legacy_output_flags(&mut args);
        assert_eq!(args[1], "--output-format");
    }

    #[test]
    fn test_normalize_legacy_output_before_command_with_option_value_prefix() {
        let mut args = vec![
            "mofa".to_string(),
            "--config".to_string(),
            "/tmp/mofa.toml".to_string(),
            "--output".to_string(),
            "json".to_string(),
            "info".to_string(),
        ];
        normalize_legacy_output_flags(&mut args);
        assert_eq!(args[3], "--output-format");
    }

    #[test]
    fn test_normalize_legacy_output_after_agent_command() {
        let mut args = vec![
            "mofa".to_string(),
            "agent".to_string(),
            "list".to_string(),
            "-o".to_string(),
            "table".to_string(),
        ];
        normalize_legacy_output_flags(&mut args);
        assert_eq!(args[3], "--output-format");
    }

    #[test]
    fn test_do_not_normalize_session_export_output_path() {
        let mut args = vec![
            "mofa".to_string(),
            "session".to_string(),
            "export".to_string(),
            "s1".to_string(),
            "-o".to_string(),
            "/tmp/s1.json".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        normalize_legacy_output_flags(&mut args);
        assert_eq!(args[4], "-o");
    }

    #[test]
    fn test_do_not_normalize_session_show_local_output_alias() {
        let mut args = vec![
            "mofa".to_string(),
            "session".to_string(),
            "show".to_string(),
            "s1".to_string(),
            "-o".to_string(),
            "json".to_string(),
        ];
        normalize_legacy_output_flags(&mut args);
        assert_eq!(args[4], "-o");
    }
}
