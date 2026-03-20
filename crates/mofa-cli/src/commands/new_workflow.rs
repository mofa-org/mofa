use crate::CliError;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

pub fn run(name: &str, dry_run: bool) -> Result<(), CliError> {
    let file_path = PathBuf::from(format!("{name}.yaml"));

    if dry_run {
        println!("{} Dry run: scaffolding new MoFA workflow: {}", "→".yellow(), file_path.display().to_string().cyan());
    } else {
        println!("{} Creating new MoFA workflow: {}", "→".green(), file_path.display().to_string().cyan());
    }

    let yaml_content = format!(
        r#"name: {name}
version: "1.0"
description: "A 3-node workflow: input -> process -> output"

nodes:
  - id: input
    type: source
    description: "Input source"
  
  - id: process
    type: processor
    description: "Data processor"

  - id: output
    type: sink
    description: "Output sink"

edges:
  - from: input
    to: process
  
  - from: process
    to: output
"#
    );

    if dry_run {
        println!("\n--- {} ---", file_path.display());
        println!("{}", yaml_content.trim_end());
    } else {
        fs::write(&file_path, yaml_content)?;
        println!("{} Workflow created successfully!", "✓".green());
    }

    Ok(())
}
