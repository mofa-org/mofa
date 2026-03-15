use crate::CliError;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

pub fn run(name: &str, dry_run: bool) -> Result<(), CliError> {
    let project_dir = PathBuf::from(name);

    if dry_run {
        println!("{} Dry run: scaffolding new MoFA agent: {}", "→".yellow(), name.cyan());
        println!("  Directory: {}", project_dir.display());
    } else {
        println!("{} Creating new MoFA agent: {}", "→".green(), name.cyan());
        println!("  Directory: {}", project_dir.display());
        fs::create_dir_all(&project_dir)?;
        fs::create_dir_all(project_dir.join("src"))?;
        fs::create_dir_all(project_dir.join("scripts"))?;
    }

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
mofa-sdk = "0.1"
tokio = {{ version = "1", features = ["full"] }}
anyhow = "1"
"#
    );

    let main_rs = r#"use anyhow::Result;
use mofa_sdk::runtime::{AgentBuilder, SimpleRuntime};

/// See https://docs.rs/mofa-sdk for more information.
#[tokio::main]
async fn main() -> Result<()> {
    println!("Agent starting up...");
    // let runtime = SimpleRuntime::new();
    // runtime.register_agent(...).await?;
    Ok(())
}
"#;

    let agent_rhai = r#"/// Sample Rhai script with on_message handler
/// See https://docs.rs/mofa-sdk for more information.

fn on_message(msg) {
    print("Received message: " + msg);
}
"#;

    write_file(&project_dir.join("Cargo.toml"), &cargo_toml, dry_run)?;
    write_file(&project_dir.join("src").join("main.rs"), main_rs, dry_run)?;
    write_file(&project_dir.join("scripts").join("agent.rhai"), agent_rhai, dry_run)?;

    if !dry_run {
        println!("{} Agent created successfully!", "✓".green());
    }

    Ok(())
}

fn write_file(path: &PathBuf, content: &str, dry_run: bool) -> Result<(), CliError> {
    if dry_run {
        println!("\n--- {} ---", path.display());
        println!("{}", content.trim_end());
    } else {
        fs::write(path, content)?;
    }
    Ok(())
}
