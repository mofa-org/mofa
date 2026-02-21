//! MoFA CLI - Command-line tool for building and managing AI agents

mod cli;
mod commands;
mod config;
mod output;
mod render;
mod state;
mod tui;
mod utils;
mod widgets;

use clap::Parser;
use cli::Cli;
use colored::Colorize;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    } else {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }

    // Create async runtime for all operations (TUI and CLI commands)
    let rt = tokio::runtime::Runtime::new()?;
    
    // Launch TUI if requested or no command provided
    if cli.tui || cli.command.is_none() {
        rt.block_on(tui::run())?;
        Ok(())
    } else {
        // Run CLI command in async context
        rt.block_on(run_command_async(cli))
    }
}

async fn run_command_async(cli: Cli) -> anyhow::Result<()> {
    use cli::Commands;

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

        Some(Commands::Agent(agent_cmd)) => match agent_cmd {
            cli::AgentCommands::Create {
                non_interactive,
                config,
            } => {
                commands::agent::create::run(non_interactive, config)?;
            }
            cli::AgentCommands::Start {
                agent_id,
                config,
                daemon,
            } => {
                commands::agent::start::run_async(&agent_id, config.as_deref(), daemon).await?;
            }
            cli::AgentCommands::Stop { agent_id } => {
                commands::agent::stop::run_async(&agent_id).await?;
            }
            cli::AgentCommands::Restart { agent_id, config } => {
                commands::agent::restart::run(&agent_id, config.as_deref())?;
            }
            cli::AgentCommands::Status { agent_id } => {
                commands::agent::status::run(agent_id.as_deref())?;
            }
            cli::AgentCommands::List { running, all } => {
                commands::agent::list::run_async(running, all).await?;
            }
        },

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

        Some(Commands::Plugin { action }) => match action {
            cli::PluginCommands::List {
                installed,
                available,
            } => {
                commands::plugin::list::run(installed, available)?;
            }
            cli::PluginCommands::Info { name } => {
                commands::plugin::info::run(&name)?;
            }
            cli::PluginCommands::Uninstall { name, force } => {
                commands::plugin::uninstall::run(&name, force)?;
            }
        },

        Some(Commands::Session { action }) => match action {
            cli::SessionCommands::List { agent, limit } => {
                commands::session::list::run(agent.as_deref(), limit)?;
            }
            cli::SessionCommands::Show { session_id, format } => {
                commands::session::show::run(
                    &session_id,
                    format.map(|f| f.to_string()).as_deref(),
                )?;
            }
            cli::SessionCommands::Delete { session_id, force } => {
                commands::session::delete::run(&session_id, force)?;
            }
            cli::SessionCommands::Export {
                session_id,
                output,
                format,
            } => {
                commands::session::export::run(&session_id, output, &format.to_string())?;
            }
        },

        Some(Commands::Tool { action }) => match action {
            cli::ToolCommands::List { available, enabled } => {
                commands::tool::list::run(available, enabled)?;
            }
            cli::ToolCommands::Info { name } => {
                commands::tool::info::run(&name)?;
            }
        },

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
